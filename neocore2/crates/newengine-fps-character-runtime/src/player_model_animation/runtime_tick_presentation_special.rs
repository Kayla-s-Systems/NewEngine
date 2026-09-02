/// Full-body authored overrides that suppress underlying locomotion/equipment presentation.
/// This phase owns fall, landing and NoClip pose state and their event suppression semantics.
fn apply_special_full_body_overrides(
    player: newengine_ecs::EntityId,
    binding: &mut PlayerAnimationRuntimeBinding,
    dt: f32,
    frame: &PlayerAnimationFrameInput,
    clip_ref: &mut String,
    timeline_events: &mut Vec<newengine_animation_api::AnimationTimelineEventV1>,
    event_occurrences: &mut Vec<AnimationEventOccurrence>,
) {
    let semantic = frame.semantic;
    let fall_presentation_requested = frame.fall_presentation_requested;
    let noclip_enabled = semantic.noclip_enabled;
    let requested_fall_band = if fall_presentation_requested {
        select_fall_presentation_band(
            semantic.fall_distance,
            binding.fall_low_pose.is_some(),
            binding.fall_medium_pose.is_some(),
            binding.fall_high_pose.is_some(),
            binding.fall_medium_min_distance,
            binding.fall_high_min_distance,
        )
    } else {
        None
    };
    if requested_fall_band != binding.fall_active_band {
        binding.fall_active_band = requested_fall_band;
        binding.fall_time_seconds = 0.0;
        let selected = match requested_fall_band {
            Some(FallPresentationBand::Low) => binding.fall_low_pose.as_mut(),
            Some(FallPresentationBand::Medium) => binding.fall_medium_pose.as_mut(),
            Some(FallPresentationBand::High) => binding.fall_high_pose.as_mut(),
            None => None,
        };
        if let Some(clip) = selected {
            clip.event_cursor.restart();
            newengine_ulog_api::ulog::info!(
                "fps-character: fall presentation selected player={} band={:?} distance_m={:.3} clip='{}'",
                player.stable_u64(),
                requested_fall_band.expect("selected fall band"),
                semantic.fall_distance,
                clip.clip_ref,
            );
        }
    } else if requested_fall_band.is_some() {
        binding.fall_time_seconds += dt;
    }

    if let Some(band) = requested_fall_band {
        let animation_runtime = &binding.animation_runtime;
        let clip = match band {
            FallPresentationBand::Low => binding.fall_low_pose.as_mut(),
            FallPresentationBand::Medium => binding.fall_medium_pose.as_mut(),
            FallPresentationBand::High => binding.fall_high_pose.as_mut(),
        };
        if let Some(clip) = clip {
            let _ = binding
                .pose_continuity
                .restore_last_visible_pose(&mut binding.sampled_target_locals);
            if let Err(error) = clip.clip.sample_local_pose_bound_preserve_untracked(
                binding.fall_time_seconds,
                animation_runtime,
                &clip.binding,
                &mut binding.sampled_target_locals,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "fps-character: height-aware fall pose sampling failed player={} band={:?} distance_m={:.3} clip='{}': {}",
                    player.stable_u64(),
                    band,
                    semantic.fall_distance,
                    clip.clip_ref,
                    error,
                );
            } else {
                *clip_ref = clip.clip_ref.clone();
            }
        }
        // Height-aware fall presentation is a full-body override. Locomotion/equipment
        // timeline events from the underlying graph must not leak through this frame.
        timeline_events.clear();
        event_occurrences.clear();
    } else if binding.fall_active_band.is_some() {
        binding.fall_active_band = None;
        binding.fall_time_seconds = 0.0;
    }

    // Landing is a non-retained semantic pulse. A hot-reloaded animation subscriber never
    // replays historical impacts, and presentation never polls PlayerLandingState directly.
    if let Some(landing) = semantic.landing {
        let band = select_fall_presentation_band(
            landing.distance,
            binding.landing_soft_pose.is_some(),
            binding.landing_medium_pose.is_some(),
            binding.landing_hard_pose.is_some() || binding.landing_hard_run_pose.is_some(),
            binding.fall_medium_min_distance,
            binding.fall_high_min_distance,
        );
        binding.landing_active_band = band;
        binding.landing_active_run = matches!(band, Some(FallPresentationBand::High))
            && binding.landing_hard_run_pose.is_some()
            && landing.horizontal_speed > 1.5;
        binding.landing_time_seconds = 0.0;
        binding.landing_active_distance = landing.distance;
        binding.landing_active_downward_speed = landing.downward_speed;
        binding.landing_active_horizontal_speed = landing.horizontal_speed;
        let selected = match (band, binding.landing_active_run) {
            (Some(FallPresentationBand::Low), _) => binding.landing_soft_pose.as_mut(),
            (Some(FallPresentationBand::Medium), _) => binding.landing_medium_pose.as_mut(),
            (Some(FallPresentationBand::High), true) => binding.landing_hard_run_pose.as_mut(),
            (Some(FallPresentationBand::High), false) => binding.landing_hard_pose.as_mut(),
            (None, _) => None,
        };
        if let Some(clip) = selected {
            clip.event_cursor.restart();
            newengine_ulog_api::ulog::info!(
                "fps-character: landing presentation selected player={} band={:?} distance_m={:.3} downward_speed={:.3} horizontal_speed={:.3} clip='{}' source=animation-semantic-pulse",
                player.stable_u64(),
                band.expect("selected landing band"),
                landing.distance,
                landing.downward_speed,
                landing.horizontal_speed,
                clip.clip_ref,
            );
        }
    }
    if fall_presentation_requested || noclip_enabled {
        binding.landing_active_band = None;
        binding.landing_time_seconds = 0.0;
        binding.landing_active_run = false;
    } else if let Some(band) = binding.landing_active_band {
        let time = binding.landing_time_seconds;
        let run_variant = binding.landing_active_run;
        let finished;
        {
            let clip = match (band, run_variant) {
                (FallPresentationBand::Low, _) => binding.landing_soft_pose.as_mut(),
                (FallPresentationBand::Medium, _) => binding.landing_medium_pose.as_mut(),
                (FallPresentationBand::High, true) => binding.landing_hard_run_pose.as_mut(),
                (FallPresentationBand::High, false) => binding.landing_hard_pose.as_mut(),
            };
            if let Some(clip) = clip {
                let _ = binding
                    .pose_continuity
                    .restore_last_visible_pose(&mut binding.sampled_target_locals);
                let duration = clip.clip.duration_seconds.max(1.0 / 30.0);
                let sample_time = time.min(duration);
                if let Err(error) = clip.clip.sample_local_pose_bound_preserve_untracked(
                    sample_time,
                    &binding.animation_runtime,
                    &clip.binding,
                    &mut binding.sampled_target_locals,
                ) {
                    newengine_ulog_api::ulog::warn!(
                        "fps-character: landing pose sampling failed player={} band={:?} distance_m={:.3} clip='{}': {}",
                        player.stable_u64(),
                        band,
                        binding.landing_active_distance,
                        clip.clip_ref,
                        error,
                    );
                    finished = true;
                } else {
                    *clip_ref = clip.clip_ref.clone();
                    finished = time + dt >= duration;
                }
            } else {
                finished = true;
            }
        }
        timeline_events.clear();
        event_occurrences.clear();
        binding.landing_time_seconds += dt;
        if finished {
            binding.landing_active_band = None;
            binding.landing_time_seconds = 0.0;
            binding.landing_active_run = false;
        }
    }

    if noclip_enabled {
        if !binding.noclip_active {
            binding.noclip_time_seconds = 0.0;
            binding.noclip_active = true;
            if let Some(noclip) = binding.noclip_pose.as_mut() {
                noclip.event_cursor.restart();
            }
            newengine_ulog_api::ulog::info!(
                "fps-character: NoClip presentation entered player={} clip='{}' overlays=off foot_contact=off",
                player.stable_u64(),
                binding
                    .noclip_pose
                    .as_ref()
                    .map(|clip| clip.clip_ref.as_str())
                    .unwrap_or("none")
            );
        } else {
            binding.noclip_time_seconds += dt;
        }
        if let Some(noclip) = binding.noclip_pose.as_mut() {
            let _ = binding
                .pose_continuity
                .restore_last_visible_pose(&mut binding.sampled_target_locals);
            let duration = noclip.clip.duration_seconds.max(1.0 / 30.0);
            let sample_time = binding.noclip_time_seconds.rem_euclid(duration);
            if let Err(error) = noclip.clip.sample_local_pose_bound_preserve_untracked(
                sample_time,
                &binding.animation_runtime,
                &noclip.binding,
                &mut binding.sampled_target_locals,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "fps-character: NoClip full-body pose sampling failed player={} clip='{}': {}",
                    player.stable_u64(),
                    noclip.clip_ref,
                    error
                );
            } else {
                *clip_ref = noclip.clip_ref.clone();
            }
        }
        timeline_events.clear();
        event_occurrences.clear();
    } else if binding.noclip_active {
        binding.noclip_active = false;
        binding.noclip_time_seconds = 0.0;
        newengine_ulog_api::ulog::info!(
            "fps-character: NoClip presentation exited player={} locomotion_restored=true",
            player.stable_u64()
        );
    }
}
