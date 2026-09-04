fn evaluate_locomotion_presentation_layer(
    player: newengine_ecs::EntityId,
    binding: &mut PlayerAnimationRuntimeBinding,
    dt: f32,
    frame: &PlayerAnimationFrameInput,
    timeline_events: &mut Vec<newengine_animation_api::AnimationTimelineEventV1>,
) -> Option<(
    newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
    bool,
    String,
)> {
    let animation_state = frame.semantic.animation_state;
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
            "fps-character: locomotion graph BlendToState failed player={} state='{}' graph_state='{}': {}",
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
            "fps-character: locomotion graph SetParameter failed player={} graph='{}': {}",
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
        "fps-character: locomotion graph evaluation failed player={} state='{}' graph_state='{}': {}",
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
        timeline_events,
    ) {
        newengine_ulog_api::ulog::warn!(
        "fps-character: locomotion graph timeline event evaluation failed player={} graph='{}': {}",
        player.stable_u64(),
        binding.locomotion_graph.name(),
        error,
    );
    }

    let active_slot = binding.active_slot;
    let active_state = binding.active_state;
    let clip_ref = binding.clips[active_slot]
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
        "fps-character: player locomotion graph transition player={} state='{}' graph_state='{}' clip='{}' duration={:.3}s normalized_speed={:.3}",
        player.stable_u64(),
        active_state.clip_hint(),
        target_graph_state,
        clip_ref,
        duration,
        animation_state.normalized_speed
    );
    }
    Some((active_state, transitioned, clip_ref))
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn weapon_aim_body_turn_request(
    view_body_yaw_delta: f32,
    minimum_turn_step: Option<f32>,
    first_person_active: bool,
) -> (f32, f32) {
    if first_person_active {
        return (0.0, 0.0);
    }
    const BODY_FOLLOW_HYSTERESIS: f32 = 5.0_f32.to_radians();
    let beyond_sector = view_body_yaw_delta.is_finite()
        && view_body_yaw_delta.abs()
            > WeaponAimControllerState::THIRD_PERSON_YAW_LIMIT_RADIANS + BODY_FOLLOW_HYSTERESIS;
    let residual = if beyond_sector {
        minimum_turn_step
            .filter(|step| step.is_finite() && *step > 1.0e-5)
            .map(|step| step.copysign(view_body_yaw_delta))
            .unwrap_or(0.0)
    } else {
        0.0
    };
    (residual, BODY_FOLLOW_HYSTERESIS)
}

#[allow(clippy::too_many_arguments)]
fn evaluate_native_turn_presentation_layer(
    player: newengine_ecs::EntityId,
    binding: &mut PlayerAnimationRuntimeBinding,
    dt: f32,
    frame: &PlayerAnimationFrameInput,
    active_state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
    clip_ref: &mut String,
    event_occurrences: &mut Vec<AnimationEventOccurrence>,
    timeline_events: &mut Vec<newengine_animation_api::AnimationTimelineEventV1>,
) -> Option<f32> {
    let look_context = frame.semantic.look_context;
    let equipment_stance = frame.semantic.equipment_stance;
    let unarmed_attack_sequence = frame.unarmed_attack_sequence;
    let view_body_yaw_delta = frame.view_body_yaw_delta;
    let view_pitch = frame.view_pitch;
    let native_turn_allowed = frame.native_turn_allowed;
    let body_yaw = frame.body_yaw;
    let mut turn_step_request = None;
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
    let weapon_aim_authority = equipment_stance == EquipmentPresentationStance::Aim;
    let look_allowed =
        unarmed_attack_sequence == 0 && equipment_allows_authored_head_look(equipment_stance);
    // Weapon aim owns a real free-aim cone around the torso. TLOU/GTA-style body follow only
    // begins after the camera leaves that sector; the body then consumes one authored turn step
    // instead of continuously stealing mouse yaw from the arms. Cyberpunk-style rubber-band damping
    // remains inside WeaponAimControllerState; FPP bypasses this body handoff entirely.
    let minimum_turn_step = binding.minimum_turn_step_radians();
    let (live_turn_yaw_delta, live_turn_hysteresis) = if native_turn_allowed && weapon_aim_authority {
        weapon_aim_body_turn_request(
            view_body_yaw_delta,
            minimum_turn_step,
            frame.first_person_active,
        )
    } else if native_turn_allowed && look_allowed {
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
            "fps-character: native turn-in-place replanned player={} old_side={} live_residual_deg={:.1} policy=free-look-never-captured-opposite-limit-replans",
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
            "fps-character: native turn-in-place started player={} side={} angle_deg={:.0} view_delta_deg={:.1} residual_deg={:.1} clip={} policy=free-look-live-residual-authored-stepping",
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
                    "fps-character: native turn-in-place sampling failed player={} clip={}: {}",
                    player.stable_u64(),
                    turn_ref,
                    error,
                );
                binding.turn_in_place = None;
            } else {
                *clip_ref = turn_ref.clone();

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
                        event_occurrences,
                        timeline_events,
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
                    "fps-character: native turn-in-place completed player={} side={} angle_deg={:.0} start_yaw_deg={:.2} applied_yaw_deg={:.3} residual_deg={:.3} max_step_deg={:.1} policy=no-snap-no-teleport",
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

    turn_step_request
}

fn apply_unarmed_presentation_layer(
    player: newengine_ecs::EntityId,
    binding: &mut PlayerAnimationRuntimeBinding,
    frame: &PlayerAnimationFrameInput,
    event_occurrences: &mut Vec<AnimationEventOccurrence>,
    timeline_events: &mut Vec<newengine_animation_api::AnimationTimelineEventV1>,
) {
    let unarmed_active = frame.unarmed_active;
    let unarmed_attack_sequence = frame.unarmed_attack_sequence;
    let animation_state = frame.semantic.animation_state;
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
            "fps-character: character-owned unarmed overlay failed player={} state='{}' phase={:.3}: {}",
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
                    event_occurrences,
                    timeline_events,
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
                    event_occurrences,
                    timeline_events,
                )
            }),
            _ => None,
        };
        if let Some(Err(error)) = event_result {
            newengine_ulog_api::ulog::warn!(
            "fps-character: unarmed animation timeline event evaluation failed player={} state='{}': {}",
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
}

fn apply_equipment_presentation_layer(
    player: newengine_ecs::EntityId,
    binding: &mut PlayerAnimationRuntimeBinding,
    frame: &PlayerAnimationFrameInput,
    active_state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
    event_occurrences: &mut Vec<AnimationEventOccurrence>,
    timeline_events: &mut Vec<newengine_animation_api::AnimationTimelineEventV1>,
) {
    let semantic = frame.semantic;
    let look_context = semantic.look_context;
    let equipment_stance = semantic.equipment_stance;
    let equipment_pose_family = frame.equipment_pose_family.as_deref();
    let equipment_presentation_active = frame.equipment_presentation_active;
    let rifle_aim_alpha = semantic.aim_alpha;
    let rifle_reload_progress = semantic.reload_progress;
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
                "fps-character: authored equipment reload overlay failed player={} ref='{}' phase={:.3}: {}",
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
                    event_occurrences,
                    timeline_events,
                ) {
                    newengine_ulog_api::ulog::warn!(
                    "fps-character: equipment reload timeline event evaluation failed player={} clip='{}': {}",
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
            let body_stance = equipment_pose_body_stance(active_state, look_context);
            if let Err(error) = apply_equipment_ready_pose(
                ready_pose_set,
                body_stance,
                ready_sample_phase,
                binding.equipment_ik.as_ref(),
                &binding.animation_runtime,
                &mut binding.equipment_overlay_locals,
                &mut binding.equipment_overlay_locals_b,
                &mut binding.sampled_target_locals,
                binding.turn_root_joint,
                binding.equipment_ready_rotation_weights.as_slice(),
            ) {
                newengine_ulog_api::ulog::warn!(
                    "fps-character: authored equipment ready overlay failed player={}: {}",
                    player.stable_u64(),
                    error,
                );
            }
            if aim_timeline_active {
                let mut transition_applied = false;
                if let Some(transition) = binding.equipment_transition {
                    let transition_clip = select_equipment_pose_set(
                        &binding.equipment_default_pose_set,
                        &binding.equipment_pose_sets,
                        equipment_pose_family,
                    )
                    .and_then(|set| equipment_transition_clip(set, transition.kind));
                    if let Some(clip) = transition_clip {
                        let duration = clip.clip.duration_seconds.max(1.0 / 30.0);
                        let phase = (transition.elapsed_seconds / duration).clamp(0.0, 1.0);
                        if let Err(error) = apply_equipment_rotation_overlay(
                            Some(clip),
                            &binding.animation_runtime,
                            &mut binding.equipment_overlay_locals,
                            &mut binding.sampled_target_locals,
                            phase,
                            binding.equipment_aim_rotation_weights.as_slice(),
                            1.0,
                        ) {
                            newengine_ulog_api::ulog::warn!(
                            "fps-character: authored equipment aim transition failed player={} phase={:.3}: {}",
                            player.stable_u64(), phase, error,
                        );
                        }
                        transition_applied = true;
                    }
                }
                if !transition_applied {
                    let layered_result = if let Some(pose_set) = select_equipment_pose_set(
                        &binding.equipment_default_pose_set,
                        &binding.equipment_pose_sets,
                        equipment_pose_family,
                    ) {
                        (|| -> Result<bool, String> {
                        // Abby's source READY<->AIM transition belongs to the foreign 1074-node reference
                        // domain, so it cannot be sampled on the current 1033-node character. Preserve its
                        // essential invariant instead: interpolate one coherent weapon frame between the
                        // two valid authored endpoint poses, then let terminal bilateral constraint solve
                        // project both arms back onto that frame after the body pose blend.
                        let source_weapon_root = if equipment_pose_family == Some("rifle") {
                            frame
                                .weapon_presentation
                                .as_ref()
                                .filter(|presentation| presentation.enabled)
                                .zip(binding.equipment_ik.as_ref())
                                .map(|(presentation, rig)| {
                                    equipment_bilateral_weapon_root_for_pose(
                                        presentation,
                                        rig,
                                        &binding.animation_runtime,
                                        &binding.sampled_target_locals,
                                        &mut binding.joint_frames_scratch,
                                    )
                                })
                                .transpose()?
                                .flatten()
                        } else {
                            None
                        };
                        let result = apply_layered_equipment_aim_pose(
                            pose_set,
                            body_stance,
                            frame.aim_velocity_local,
                            binding.equipment_time_seconds,
                            rifle_aim_alpha,
                            semantic.obstruction_alpha,
                            &binding.animation_runtime,
                            &mut binding.equipment_overlay_locals,
                            &mut binding.equipment_overlay_locals_b,
                            &mut binding.equipment_composed_locals,
                            &mut binding.sampled_target_locals,
                            binding.turn_root_joint,
                            binding.equipment_aim_rotation_weights.as_slice(),
                        )?;
                        if result && equipment_pose_family == Some("rifle") {
                            let target_weapon_root = frame
                                .weapon_presentation
                                .as_ref()
                                .filter(|presentation| presentation.enabled)
                                .zip(binding.equipment_ik.as_ref())
                                .map(|(presentation, rig)| {
                                    equipment_bilateral_weapon_root_for_pose(
                                        presentation,
                                        rig,
                                        &binding.animation_runtime,
                                        &binding.equipment_composed_locals,
                                        &mut binding.joint_frames_scratch,
                                    )
                                })
                                .transpose()?
                                .flatten();
                            binding.equipment_transition_weapon_root = source_weapon_root
                                .zip(target_weapon_root)
                                .and_then(|(source, target)| {
                                    interpolate_equipment_weapon_root(source, target, rifle_aim_alpha)
                                });
                        }
                        Ok(result)
                        })()
                    } else {
                        Ok(false)
                    };
                    let layered_applied = match layered_result {
                        Ok(applied) => applied,
                        Err(error) => {
                            newengine_ulog_api::ulog::warn!(
                            "fps-character: layered equipment aim evaluation failed player={}: {}",
                            player.stable_u64(),
                            error,
                        );
                            false
                        }
                    };
                    if !layered_applied {
                        let aim_phase = select_equipment_pose_set(
                            &binding.equipment_default_pose_set,
                            &binding.equipment_pose_sets,
                            equipment_pose_family,
                        )
                        .and_then(|set| set.aim.as_ref())
                        .map(|clip| {
                            equipment_loop_phase(
                                clip.clip.duration_seconds,
                                binding.equipment_time_seconds,
                            )
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
                            "fps-character: authored equipment aim overlay failed player={} phase={:.3} alpha={:.3}: {}",
                            player.stable_u64(), aim_phase, rifle_aim_alpha, error,
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
                                event_occurrences,
                                timeline_events,
                            ) {
                                newengine_ulog_api::ulog::warn!(
                                "fps-character: equipment aim timeline event evaluation failed player={} clip='{}': {}",
                                player.stable_u64(), aim.clip_ref, error,
                            );
                            }
                        }
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
}
