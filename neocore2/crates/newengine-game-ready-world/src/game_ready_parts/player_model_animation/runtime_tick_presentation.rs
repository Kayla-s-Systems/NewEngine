/// Phase 2: evaluate locomotion and authored presentation layers. This phase owns animation
/// cursors/state machines, but delegates final look/IK/continuity/palette construction to phase 3.
fn evaluate_player_animation_presentation(
    player: newengine_ecs::EntityId,
    binding: &mut PlayerAnimationRuntimeBinding,
    dt: f32,
    frame: &PlayerAnimationFrameInput,
) -> Option<PlayerAnimationFrameOutput> {
    let presentation_started = std::time::Instant::now();
    let semantic = frame.semantic;
    let animation_state = semantic.animation_state;
    let look_context = semantic.look_context;
    let noclip_enabled = semantic.noclip_enabled;
    let fall_presentation_requested = frame.fall_presentation_requested;
    let unarmed_active = frame.unarmed_active;
    let unarmed_attack_sequence = frame.unarmed_attack_sequence;
    let rifle_aim_alpha = semantic.aim_alpha;
    let equipment_stance = semantic.equipment_stance;
    let equipment_pose_family = frame.equipment_pose_family.as_deref();
    let equipment_presentation_active = frame.equipment_presentation_active;
    let equipment_trace_changed = binding.equipment_trace_active != equipment_presentation_active
        || binding.equipment_trace_stance != equipment_stance
        || binding.equipment_trace_family.as_deref() != equipment_pose_family;
    if equipment_trace_changed {
        let selected_clip = equipment_presentation_active
            .then(|| {
                select_equipment_pose_set(
                    &binding.equipment_default_pose_set,
                    &binding.equipment_pose_sets,
                    equipment_pose_family,
                )
                .and_then(|set| match equipment_stance {
                    EquipmentPresentationStance::Ready => set.ready.as_ref(),
                    EquipmentPresentationStance::Aim => set.aim.as_ref(),
                    EquipmentPresentationStance::Reload => set.reload.as_ref(),
                    EquipmentPresentationStance::None => None,
                })
                .map(|clip| clip.clip_ref.clone())
            })
            .flatten();
        let stance = match equipment_stance {
            EquipmentPresentationStance::None => "none",
            EquipmentPresentationStance::Ready => "ready",
            EquipmentPresentationStance::Aim => "aim",
            EquipmentPresentationStance::Reload => "reload",
        };
        newengine_ulog_api::ulog::info!(
            "game-ready: equipment pose selected player={} active={} family='{}' stance='{}' clip='{}' policy='weapon class selects character-owned authored capability; no cross-family fallback'",
            player.stable_u64(),
            equipment_presentation_active,
            equipment_pose_family.unwrap_or("<generic>"),
            stance,
            selected_clip.as_deref().unwrap_or("<none>"),
        );
        binding.equipment_trace_active = equipment_presentation_active;
        binding.equipment_trace_family = equipment_pose_family.map(str::to_owned);
        binding.equipment_trace_stance = equipment_stance;
    }
    let rifle_reload_progress = semantic.reload_progress;
    let body_yaw = frame.body_yaw;
    let view_body_yaw_delta = frame.view_body_yaw_delta;
    let view_pitch = frame.view_pitch;
    let native_turn_allowed = frame.native_turn_allowed;
    let mut timeline_events = Vec::new();
    let mut turn_step_request = None;
    if semantic.max_pulse_sequence > binding.consumed_pulse_sequence {
        binding.consumed_pulse_sequence = semantic.max_pulse_sequence;
        binding
            .semantic_input
            .discard_pulses_through(binding.consumed_pulse_sequence);
    }
    let mut event_occurrences = Vec::new();
    binding.equipment_time_seconds += dt;
    binding.equipment_ik_residual_diag_cooldown =
        (binding.equipment_ik_residual_diag_cooldown - dt).max(0.0);
    binding.equipment_resolved_weapon_root = None;
    if unarmed_active {
        if binding.unarmed_attack_sequence != unarmed_attack_sequence {
            binding.unarmed_attack_sequence = unarmed_attack_sequence;
            binding.unarmed_attack_time_seconds = 0.0;
            if let Some(attack) = binding.unarmed_attack_pose.as_mut() {
                attack.event_cursor.restart();
            }
        } else if unarmed_attack_sequence > 0 {
            binding.unarmed_attack_time_seconds += dt;
        }
    } else {
        binding.unarmed_attack_sequence = 0;
        binding.unarmed_attack_time_seconds = 0.0;
    }

    let reload_active = equipment_stance == EquipmentPresentationStance::Reload;
    if reload_active && !binding.equipment_reload_active {
        if let Some(reload) = select_equipment_pose_set_mut(
            &mut binding.equipment_default_pose_set,
            &mut binding.equipment_pose_sets,
            equipment_pose_family,
        )
        .and_then(|set| set.reload.as_mut())
        {
            reload.event_cursor.restart();
        }
    }
    binding.equipment_reload_active = reload_active;

    let requested_state = animation_state.locomotion;
    let requested_slot = binding.resolve_slot(requested_state);
    let (effective_state, desired_slot) = match requested_slot {
        Some(slot) => (requested_state, slot),
        None => (binding.active_state, binding.active_slot),
    };
    let state_changed = binding.active_state != effective_state;
    let slot_changed = binding.active_slot != desired_slot;
    let transitioned = state_changed || slot_changed;
    let target_graph_state = locomotion_state_for_slot(desired_slot);

    if slot_changed {
        if let Err(error) = blend_locomotion_graph_to_state(
            player,
            &binding.locomotion_graph,
            &mut binding.locomotion_graph_instance,
            target_graph_state,
        ) {
            newengine_ulog_api::ulog::warn!(
                "game-ready: locomotion graph BlendToState failed player={} state='{}' graph_state='{}': {}",
                player.stable_u64(),
                animation_state.locomotion.clip_hint(),
                target_graph_state,
                error,
            );
            return None;
        }
        binding.active_slot = desired_slot;
    }
    if state_changed {
        binding.active_state = effective_state;
    }

    if let Err(error) = apply_locomotion_graph_parameters(
        player,
        &binding.locomotion_graph,
        &mut binding.locomotion_graph_instance,
        animation_state.normalized_speed,
    ) {
        newengine_ulog_api::ulog::warn!(
            "game-ready: locomotion graph SetParameter failed player={} graph='{}': {}",
            player.stable_u64(),
            binding.locomotion_graph.name(),
            error,
        );
        return None;
    }

    // Zero-gap transition contract: both source and destination advance on the transition
    // frame and blending starts immediately. There is no synthetic frozen t=0 frame.
    let graph_dt = dt * locomotion_playback_rate(animation_state);
    if let Err(error) = binding.locomotion_graph_instance.evaluate(
        &binding.locomotion_graph,
        &binding.animation_runtime,
        graph_dt,
        &mut binding.locomotion_graph_evaluation,
    ) {
        newengine_ulog_api::ulog::warn!(
            "game-ready: locomotion graph evaluation failed player={} state='{}' graph_state='{}': {}",
            player.stable_u64(),
            animation_state.locomotion.clip_hint(),
            target_graph_state,
            error,
        );
        return None;
    }
    binding
        .sampled_target_locals
        .clone_from(&binding.locomotion_graph_evaluation.local_pose);

    if let Err(error) = collect_locomotion_graph_events(
        player,
        &binding.locomotion_graph,
        &binding.locomotion_graph_evaluation,
        &binding.clips,
        &mut timeline_events,
    ) {
        newengine_ulog_api::ulog::warn!(
            "game-ready: locomotion graph timeline event evaluation failed player={} graph='{}': {}",
            player.stable_u64(),
            binding.locomotion_graph.name(),
            error,
        );
    }

    let active_slot = binding.active_slot;
    let active_state = binding.active_state;
    let mut clip_ref = binding.clips[active_slot]
        .as_ref()
        .expect("resolved player animation slot must contain a clip")
        .clip_ref
        .clone();
    if transitioned {
        let duration = binding.clips[active_slot]
            .as_ref()
            .map(|clip| clip.clip.duration_seconds)
            .unwrap_or_default();
        newengine_ulog_api::ulog::info!(
            "game-ready: player locomotion graph transition player={} state='{}' graph_state='{}' clip='{}' duration={:.3}s normalized_speed={:.3}",
            player.stable_u64(),
            active_state.clip_hint(),
            target_graph_state,
            clip_ref,
            duration,
            animation_state.normalized_speed
        );
    }
    // Native turn-in-place follows the *live* free-look residual. Camera/view yaw is never
    // captured by an active body step: head/eyes keep consuming the current view every frame,
    // while the body starts only after authored look space is exhausted. If the user crosses
    // the opposite authored limit during a step, re-plan through pose continuity instead of
    // forcing the stale turn direction to finish. Returning inside the limit does not cancel
    // an already planted foot step, which avoids foot popping.
    if !native_turn_allowed && binding.turn_in_place.is_some() {
        binding.turn_in_place = None;
    }
    let look_state = resolve_authored_look_state(active_state, equipment_stance, look_context);
    let look_allowed =
        unarmed_attack_sequence == 0 && equipment_stance != EquipmentPresentationStance::Reload;
    let (live_turn_yaw_delta, live_turn_hysteresis) = if native_turn_allowed && look_allowed {
        if let Some(projection) =
            binding
                .authored_look
                .projection(look_state, view_body_yaw_delta, view_pitch)
        {
            debug_assert!(
                (view_body_yaw_delta
                    - projection.body_projected[0]
                    - projection.eye_projected[0]
                    - projection.residual[0])
                    .abs()
                    <= 1.0e-3,
                "authored look yaw projection must conserve view intent"
            );
            debug_assert!(
                (view_pitch
                    - projection.body_projected[1]
                    - projection.eye_projected[1]
                    - projection.residual[1])
                    .abs()
                    <= 1.0e-3,
                "authored look pitch projection must conserve view intent"
            );
            let residual = if projection.residual[0].abs() > projection.turn_hysteresis_radians {
                projection.residual[0]
            } else {
                0.0
            };
            (residual, projection.turn_hysteresis_radians)
        } else if look_state.contextual() {
            // An explicit contextual state without its authored range stays fail-closed.
            (0.0, f32::INFINITY)
        } else {
            let hysteresis = binding
                .minimum_turn_step_radians()
                .map(|angle| angle * 0.5)
                .unwrap_or(f32::INFINITY);
            let residual = if view_body_yaw_delta.abs() > hysteresis {
                view_body_yaw_delta
            } else {
                0.0
            };
            (residual, hysteresis)
        }
    } else {
        (0.0, f32::INFINITY)
    };

    if let Some(active) = binding.turn_in_place {
        if live_view_residual_requires_turn_replan(
            active.slot,
            live_turn_yaw_delta,
            live_turn_hysteresis,
        ) {
            newengine_ulog_api::ulog::info!(
                "game-ready: native turn-in-place replanned player={} old_side={} live_residual_deg={:.1} policy=free-look-never-captured-opposite-limit-replans",
                player.stable_u64(),
                if active.slot.signed_yaw_radians() > 0.0 { "left" } else { "right" },
                live_turn_yaw_delta.to_degrees(),
            );
            binding.turn_in_place = None;
        }
    }

    if native_turn_allowed && binding.turn_in_place.is_none() {
        if let Some(slot) = nearest_turn_in_place_slot(live_turn_yaw_delta, |candidate| {
            binding.turn_clip(candidate).is_some()
        }) {
            if let Some(clip) = binding.turn_clip_mut(slot) {
                clip.event_cursor.restart();
            }
            binding.turn_in_place = Some(TurnInPlaceRuntimeState {
                slot,
                elapsed_seconds: 0.0,
                start_body_yaw: body_yaw,
                last_body_yaw: body_yaw,
                applied_yaw_radians: 0.0,
            });
            binding.turn_sequence = binding.turn_sequence.wrapping_add(1).max(1);
            let selected_turn_ref = binding
                .turn_clip(slot)
                .map(|clip| clip.clip_ref.as_str())
                .unwrap_or("<missing>");
            newengine_ulog_api::ulog::info!(
                "game-ready: native turn-in-place started player={} side={} angle_deg={:.0} view_delta_deg={:.1} residual_deg={:.1} clip={} policy=free-look-live-residual-authored-stepping",
                player.stable_u64(),
                if slot.signed_yaw_radians() > 0.0 { "left" } else { "right" },
                slot.angle_degrees(),
                view_body_yaw_delta.to_degrees(),
                live_turn_yaw_delta.to_degrees(),
                selected_turn_ref,
            );
        }
    }
    let active_turn = binding.turn_in_place;
    if let Some(mut turn) = active_turn {
        // Accumulate accepted simulation yaw without wrapping the whole 180-degree turn.
        turn.applied_yaw_radians =
            accumulate_turn_in_place_yaw(turn.applied_yaw_radians, turn.last_body_yaw, body_yaw);
        turn.last_body_yaw = body_yaw;

        let clip_data = binding.turn_clip(turn.slot).map(|clip| {
            (
                clip.clip_ref.clone(),
                clip.clip.clone(),
                clip.binding.clone(),
                clip.clip.duration_seconds.max(1.0 / 30.0),
            )
        });
        if let Some((turn_ref, turn_clip, turn_binding, duration)) = clip_data {
            let _ = binding
                .pose_continuity
                .restore_last_visible_pose(&mut binding.sampled_target_locals);
            turn.elapsed_seconds = (turn.elapsed_seconds + dt).min(duration);
            if let Err(error) = turn_clip.sample_local_pose_bound_preserve_untracked(
                turn.elapsed_seconds,
                &binding.animation_runtime,
                &turn_binding,
                &mut binding.sampled_target_locals,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: native turn-in-place sampling failed player={} clip={}: {}",
                    player.stable_u64(),
                    turn_ref,
                    error,
                );
                binding.turn_in_place = None;
            } else {
                clip_ref = turn_ref.clone();

                // Remove accepted physical yaw from the sampled root. Authored feet/pelvis
                // stepping remains visually intact and cannot double-spin.
                compensate_turn_root_yaw(
                    &mut binding.sampled_target_locals,
                    binding.turn_root_joint,
                    turn.applied_yaw_radians,
                );

                if let Some(clip) = binding.turn_clip_mut(turn.slot) {
                    let _ = crate::animation_events::collect_timeline_events(
                        player,
                        &clip.clip_ref,
                        "character.turn_in_place",
                        &clip.clip,
                        &mut clip.event_cursor,
                        turn.elapsed_seconds,
                        &mut event_occurrences,
                        &mut timeline_events,
                    );
                }

                let desired_applied_yaw =
                    turn_in_place_target_yaw(turn.slot, turn.elapsed_seconds, duration);
                let yaw_error = desired_applied_yaw - turn.applied_yaw_radians;
                let step_yaw = bounded_turn_in_place_step(yaw_error);
                if step_yaw.abs() > 1.0e-6 {
                    turn_step_request = Some(step_yaw);
                }

                let final_error = turn.slot.signed_yaw_radians() - turn.applied_yaw_radians;
                let clip_finished = turn.elapsed_seconds + 1.0e-5 >= duration;
                if clip_finished && final_error.abs() <= TURN_IN_PLACE_FINISH_EPSILON_RADIANS {
                    newengine_ulog_api::ulog::info!(
                        "game-ready: native turn-in-place completed player={} side={} angle_deg={:.0} start_yaw_deg={:.2} applied_yaw_deg={:.3} residual_deg={:.3} max_step_deg={:.1} policy=no-snap-no-teleport",
                        player.stable_u64(),
                        if turn.slot.signed_yaw_radians() > 0.0 { "left" } else { "right" },
                        turn.slot.angle_degrees(),
                        turn.start_body_yaw.to_degrees(),
                        turn.applied_yaw_radians.to_degrees(),
                        final_error.to_degrees(),
                        TURN_IN_PLACE_MAX_STEP_RADIANS.to_degrees(),
                    );
                    binding.turn_in_place = None;
                } else {
                    // Hold final authored stance while the remainder drains through the
                    // same bounded path. There is never a special final facing commit.
                    binding.turn_in_place = Some(turn);
                }
            }
        } else {
            binding.turn_in_place = None;
        }
    }

    if unarmed_active {
        let attack_phase = binding.unarmed_attack_pose.as_ref().and_then(|clip| {
            if unarmed_attack_sequence == 0 {
                return None;
            }
            let duration = clip.clip.duration_seconds.max(1.0 / 30.0);
            (binding.unarmed_attack_time_seconds <= duration)
                .then(|| (binding.unarmed_attack_time_seconds / duration).clamp(0.0, 1.0))
        });
        let (overlay, phase, label) = if let Some(phase) = attack_phase {
            (binding.unarmed_attack_pose.as_ref(), phase, "attack")
        } else if matches!(
            animation_state.locomotion,
            newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Idle
                | newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::CrouchIdle
        ) {
            let phase = binding
                .unarmed_ready_pose
                .as_ref()
                .map(|clip| {
                    let duration = clip.clip.duration_seconds.max(1.0 / 30.0);
                    (binding.equipment_time_seconds.rem_euclid(duration) / duration).clamp(0.0, 1.0)
                })
                .unwrap_or(0.0);
            (binding.unarmed_ready_pose.as_ref(), phase, "ready")
        } else {
            (None, 0.0, "locomotion")
        };
        if let Err(error) = apply_character_rotation_overlay(
            overlay,
            &binding.skeleton,
            &binding.animation_runtime,
            &mut binding.equipment_overlay_locals,
            &mut binding.sampled_target_locals,
            phase,
        ) {
            newengine_ulog_api::ulog::warn!(
                "game-ready: character-owned unarmed overlay failed player={} state='{}' phase={:.3}: {}",
                player.stable_u64(),
                label,
                phase,
                error,
            );
        }
        let event_result = match label {
            "attack" => binding.unarmed_attack_pose.as_mut().map(|clip| {
                crate::animation_events::collect_timeline_events(
                    player,
                    &clip.clip_ref,
                    "character.unarmed.attack",
                    &clip.clip,
                    &mut clip.event_cursor,
                    binding.unarmed_attack_time_seconds,
                    &mut event_occurrences,
                    &mut timeline_events,
                )
            }),
            "ready" => binding.unarmed_ready_pose.as_mut().map(|clip| {
                crate::animation_events::collect_timeline_events(
                    player,
                    &clip.clip_ref,
                    "character.unarmed.ready",
                    &clip.clip,
                    &mut clip.event_cursor,
                    binding.equipment_time_seconds,
                    &mut event_occurrences,
                    &mut timeline_events,
                )
            }),
            _ => None,
        };
        if let Some(Err(error)) = event_result {
            newengine_ulog_api::ulog::warn!(
                "game-ready: unarmed animation timeline event evaluation failed player={} state='{}': {}",
                player.stable_u64(),
                label,
                error,
            );
        }
        if label != "ready" {
            if let Some(ready) = binding.unarmed_ready_pose.as_mut() {
                let _ = ready.event_cursor.seek(binding.equipment_time_seconds);
            }
        }
    } else if let Some(ready) = binding.unarmed_ready_pose.as_mut() {
        let _ = ready.event_cursor.seek(binding.equipment_time_seconds);
    }

    let aim_timeline_active = equipment_presentation_active
        && equipment_stance == EquipmentPresentationStance::Aim
        && rifle_aim_alpha > 0.001;
    if equipment_presentation_active {
        if equipment_stance == EquipmentPresentationStance::Reload {
            let progress = rifle_reload_progress.unwrap_or(0.0);
            let overlay_ref = select_equipment_pose_set(
                &binding.equipment_default_pose_set,
                &binding.equipment_pose_sets,
                equipment_pose_family,
            )
            .and_then(|set| set.reload.as_ref())
            .map(|clip| clip.clip_ref.clone())
            .unwrap_or_else(|| "<none>".to_owned());
            if let Err(error) = apply_equipment_rotation_overlay(
                select_equipment_pose_set(
                    &binding.equipment_default_pose_set,
                    &binding.equipment_pose_sets,
                    equipment_pose_family,
                )
                .and_then(|set| set.reload.as_ref()),
                &binding.animation_runtime,
                &mut binding.equipment_overlay_locals,
                &mut binding.sampled_target_locals,
                progress,
                binding.equipment_reload_rotation_weights.as_slice(),
                1.0,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: authored equipment reload overlay failed player={} ref='{}' phase={:.3}: {}",
                    player.stable_u64(),
                    overlay_ref,
                    progress,
                    error,
                );
            }
            if let Some(reload) = select_equipment_pose_set_mut(
                &mut binding.equipment_default_pose_set,
                &mut binding.equipment_pose_sets,
                equipment_pose_family,
            )
            .and_then(|set| set.reload.as_mut())
            {
                let playback_time = reload.clip.duration_seconds * progress;
                if let Err(error) = crate::animation_events::collect_timeline_events(
                    player,
                    &reload.clip_ref,
                    "character.equipment.reload",
                    &reload.clip,
                    &mut reload.event_cursor,
                    playback_time,
                    &mut event_occurrences,
                    &mut timeline_events,
                ) {
                    newengine_ulog_api::ulog::warn!(
                        "game-ready: equipment reload timeline event evaluation failed player={} clip='{}': {}",
                        player.stable_u64(),
                        reload.clip_ref,
                        error,
                    );
                }
            }
        } else {
            let ready_pose_set = select_equipment_pose_set(
                &binding.equipment_default_pose_set,
                &binding.equipment_pose_sets,
                equipment_pose_family,
            );
            let ready_sample_phase = equipment_ready_sample_phase_for_pose_set(
                ready_pose_set,
                binding.equipment_ready_sample_phase,
            );
            if let Err(error) = apply_equipment_rotation_overlay(
                ready_pose_set.and_then(|set| set.ready.as_ref()),
                &binding.animation_runtime,
                &mut binding.equipment_overlay_locals,
                &mut binding.sampled_target_locals,
                ready_sample_phase,
                binding.equipment_ready_rotation_weights.as_slice(),
                1.0,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: authored equipment ready overlay failed player={}: {}",
                    player.stable_u64(),
                    error,
                );
            }
            if aim_timeline_active {
                let aim_phase = select_equipment_pose_set(
                    &binding.equipment_default_pose_set,
                    &binding.equipment_pose_sets,
                    equipment_pose_family,
                )
                .and_then(|set| set.aim.as_ref())
                .map(|clip| {
                    let duration = clip.clip.duration_seconds.max(1.0 / 30.0);
                    (binding.equipment_time_seconds.rem_euclid(duration) / duration).clamp(0.0, 1.0)
                })
                .unwrap_or(0.0);
                if let Err(error) = apply_equipment_rotation_overlay(
                    select_equipment_pose_set(
                        &binding.equipment_default_pose_set,
                        &binding.equipment_pose_sets,
                        equipment_pose_family,
                    )
                    .and_then(|set| set.aim.as_ref()),
                    &binding.animation_runtime,
                    &mut binding.equipment_overlay_locals,
                    &mut binding.sampled_target_locals,
                    aim_phase,
                    binding.equipment_aim_rotation_weights.as_slice(),
                    rifle_aim_alpha,
                ) {
                    newengine_ulog_api::ulog::warn!(
                        "game-ready: authored equipment aim overlay failed player={} phase={:.3} alpha={:.3}: {}",
                        player.stable_u64(),
                        aim_phase,
                        rifle_aim_alpha,
                        error,
                    );
                }
                if let Some(aim) = select_equipment_pose_set_mut(
                    &mut binding.equipment_default_pose_set,
                    &mut binding.equipment_pose_sets,
                    equipment_pose_family,
                )
                .and_then(|set| set.aim.as_mut())
                {
                    if let Err(error) = crate::animation_events::collect_timeline_events(
                        player,
                        &aim.clip_ref,
                        "character.equipment.aim",
                        &aim.clip,
                        &mut aim.event_cursor,
                        binding.equipment_time_seconds,
                        &mut event_occurrences,
                        &mut timeline_events,
                    ) {
                        newengine_ulog_api::ulog::warn!(
                            "game-ready: equipment aim timeline event evaluation failed player={} clip='{}': {}",
                            player.stable_u64(),
                            aim.clip_ref,
                            error,
                        );
                    }
                }
            }
        }
    }
    if !aim_timeline_active {
        if let Some(aim) = select_equipment_pose_set_mut(
            &mut binding.equipment_default_pose_set,
            &mut binding.equipment_pose_sets,
            equipment_pose_family,
        )
        .and_then(|set| set.aim.as_mut())
        {
            let _ = aim.event_cursor.seek(binding.equipment_time_seconds);
        }
    }

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
                "game-ready: fall presentation selected player={} band={:?} distance_m={:.3} clip='{}'",
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
                    "game-ready: height-aware fall pose sampling failed player={} band={:?} distance_m={:.3} clip='{}': {}",
                    player.stable_u64(),
                    band,
                    semantic.fall_distance,
                    clip.clip_ref,
                    error,
                );
            } else {
                clip_ref = clip.clip_ref.clone();
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
                "game-ready: landing presentation selected player={} band={:?} distance_m={:.3} downward_speed={:.3} horizontal_speed={:.3} clip='{}' source=animation-semantic-pulse",
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
                        "game-ready: landing pose sampling failed player={} band={:?} distance_m={:.3} clip='{}': {}",
                        player.stable_u64(),
                        band,
                        binding.landing_active_distance,
                        clip.clip_ref,
                        error,
                    );
                    finished = true;
                } else {
                    clip_ref = clip.clip_ref.clone();
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
                "game-ready: NoClip presentation entered player={} clip='{}' overlays=off foot_contact=off",
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
                    "game-ready: NoClip full-body pose sampling failed player={} clip='{}': {}",
                    player.stable_u64(),
                    noclip.clip_ref,
                    error
                );
            } else {
                clip_ref = noclip.clip_ref.clone();
            }
        }
        timeline_events.clear();
        event_occurrences.clear();
    } else if binding.noclip_active {
        binding.noclip_active = false;
        binding.noclip_time_seconds = 0.0;
        newengine_ulog_api::ulog::info!(
            "game-ready: NoClip presentation exited player={} locomotion_restored=true",
            player.stable_u64()
        );
    }

    let presentation_core_ms = presentation_started.elapsed().as_secs_f32() * 1000.0;
    let finalize_started = std::time::Instant::now();
    let (palette, foot_pose, finalize_timing) = finalize_player_pose_and_palette(
        player,
        binding,
        dt,
        frame,
        &clip_ref,
        active_state,
        unarmed_attack_sequence,
        equipment_stance,
        transitioned,
    )?;
    let finalize_ms = finalize_started.elapsed().as_secs_f32() * 1000.0;
    Some(PlayerAnimationFrameOutput {
        palette,
        clip_ref,
        active_state,
        foot_pose,
        turn_step_request,
        model_to_world: frame.model_to_world,
        timeline_events,
        presentation_core_ms,
        finalize_ms,
        finalize_timing,
    })
}
