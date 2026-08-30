#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum EquipmentPresentationStance {
    #[default]
    None,
    Ready,
    Aim,
    Reload,
}

fn resolve_equipment_presentation_stance(
    weapon_type: Option<newengine_engine_runtime::gameplay::WeaponType>,
    weapon_state: Option<newengine_engine_runtime::gameplay::PlayerWeaponState>,
    authored_presentation: bool,
) -> EquipmentPresentationStance {
    if !authored_presentation
        || weapon_type != Some(newengine_engine_runtime::gameplay::WeaponType::Firearm)
    {
        return EquipmentPresentationStance::None;
    }
    let Some(state) = weapon_state else {
        return EquipmentPresentationStance::Ready;
    };
    if state.reload_remaining > 0.0 {
        EquipmentPresentationStance::Reload
    } else if state.aiming {
        EquipmentPresentationStance::Aim
    } else {
        EquipmentPresentationStance::Ready
    }
}

pub(crate) fn tick_player_skin_animation(world: &mut newengine_ecs::World, dt: f32) {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    let players = world
        .query::<PlayerAnimationRuntimeBinding>()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();

    for player in players {
        let animation_state = world
            .get::<newengine_engine_runtime::gameplay::PlayerAnimationState>(player)
            .copied()
            .unwrap_or_default();
        let noclip_enabled = world
            .resource::<newengine_gameplay_fps_api::FpsCharacterTraversalState>()
            .copied()
            .is_some_and(|state| state.noclip_enabled());
        let fall_state = world
            .get::<newengine_engine_runtime::gameplay::PlayerFallState>(player)
            .copied()
            .unwrap_or_default();
        let landing_state = world
            .get::<newengine_engine_runtime::gameplay::PlayerLandingState>(player)
            .copied()
            .unwrap_or_default();
        let fall_presentation_requested = !noclip_enabled && fall_state.falling;
        let active_weapon =
            newengine_engine_runtime::gameplay::active_equipped_weapon_binding(world, player);
        let unarmed_active = !noclip_enabled
            && !fall_presentation_requested
            && active_weapon.is_some_and(|binding| {
                binding.weapon.weapon_type
                    == newengine_engine_runtime::gameplay::WeaponType::Unarmed
            });
        let unarmed_attack_sequence = if unarmed_active {
            world
                .get::<newengine_engine_runtime::gameplay::PlayerWeaponState>(player)
                .map(|state| state.shot_sequence)
                .unwrap_or(0)
        } else {
            0
        };
        let rifle_aim_alpha = super::equipment_visual::equipped_weapon_aim_alpha(world, player);
        let rifle_recoil_alpha =
            super::equipment_visual::equipped_weapon_recoil_alpha(world, player);
        let rifle_recoil_yaw_radians =
            super::equipment_visual::equipped_weapon_recoil_yaw_radians(world, player);
        let rifle_obstruction_alpha = world
            .get::<newengine_engine_runtime::gameplay::WeaponObstructionState>(player)
            .map(|state| state.alpha.clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let first_person_active = world
            .resource::<newengine_engine_runtime::gameplay::PlayerViewState>()
            .copied()
            .unwrap_or_default()
            .first_person_active;
        // Secondary long-gun inertia is a third-person presentation effect. Feeding its previous
        // frame offset back into first-person arm IK creates a hand <-> weapon feedback loop and
        // manifests as visible weapon tremor. First-person hands solve against the canonical root.
        let rifle_secondary_rotation_offset_local = if first_person_active {
            Vec3::ZERO
        } else {
            super::equipment_visual::equipped_weapon_secondary_rotation_offset_local(world, player)
        };
        let rifle_view_rotation_model = if first_person_active || rifle_aim_alpha > 0.001 {
            player_rifle_view_rotation_model(world, player)
        } else {
            None
        };
        let rifle_view_forward_model = rifle_view_rotation_model
            .map(|rotation| (rotation * -Vec3::Z).normalize_or_zero())
            .filter(|forward| forward.is_finite() && forward.length_squared() > 1.0e-8);
        let weapon_state = world
            .get::<newengine_engine_runtime::gameplay::PlayerWeaponState>(player)
            .copied();
        let weapon_presentation = active_weapon
            .and_then(|equipped| {
                world
                    .resource::<newengine_engine_runtime::gameplay::ItemCatalog>()?
                    .get(equipped.item)
                    .map(|definition| definition.weapon_presentation.clone().sanitized())
            })
            .filter(|presentation| presentation.enabled);
        let equipment_stance = resolve_equipment_presentation_stance(
            active_weapon.map(|binding| binding.weapon.weapon_type),
            weapon_state,
            weapon_presentation.is_some(),
        );
        let equipment_presentation_active = !noclip_enabled
            && !fall_presentation_requested
            && equipment_stance != EquipmentPresentationStance::None
            && world
                .get::<PlayerAnimationRuntimeBinding>(player)
                .is_some_and(|binding| {
                    binding.equipment_ready_pose.is_some()
                        || binding.equipment_aim_pose.is_some()
                        || binding.equipment_reload_pose.is_some()
                        || binding.equipment_ik.is_some()
                });
        let rifle_reload_progress = if equipment_stance == EquipmentPresentationStance::Reload {
            weapon_state.and_then(|state| {
                (state.reload_remaining > 0.0).then(|| {
                    let duration = world
                        .get::<newengine_engine_runtime::gameplay::HitscanWeaponTuning>(player)
                        .map(|tuning| tuning.sanitized().reload_duration)
                        .filter(|duration| *duration > 1.0e-4)
                        .unwrap_or(2.0);
                    (1.0 - state.reload_remaining / duration).clamp(0.0, 1.0)
                })
            })
        } else {
            None
        };
        let world_velocity = world
            .get::<newengine_sim::Velocity>(player)
            .copied()
            .unwrap_or_default()
            .0;

        let root_transform = world.get::<Transform>(player).copied().unwrap_or_default();
        let model_root_local = world
            .get::<newengine_engine_runtime::gameplay::PlayerModelBinding>(player)
            .and_then(|binding| binding.visual_root)
            .and_then(|visual_root| world.get::<Transform>(visual_root).copied())
            .unwrap_or_default();
        // Gameplay facing is the PlayerActor root. `visual_root.rotation` is an imported-model basis
        // correction (Ellie is a good example with yaw_offset=PI) and must never be interpreted as
        // additional gameplay yaw, otherwise an aligned character appears to require a 180° turn.
        let rendered_body_rotation = root_transform.rotation.normalize_or_identity();
        let body_forward = (rendered_body_rotation * -Vec3::Z).normalize_or_zero();
        let body_yaw = if body_forward.length_squared() > 1.0e-8 {
            (-body_forward.x).atan2(-body_forward.z)
        } else {
            0.0
        };
        let view_yaw = world
            .get::<newengine_sim::CharacterMotor>(player)
            .map(|motor| motor.yaw)
            .filter(|yaw| yaw.is_finite())
            .unwrap_or(body_yaw);
        let view_body_yaw_delta = newengine_math::wrap_pi(view_yaw - body_yaw);
        let horizontal_speed = Vec3::new(world_velocity.x, 0.0, world_velocity.z).length();
        let native_turn_allowed = !noclip_enabled
            && !fall_presentation_requested
            && horizontal_speed < 0.08
            && animation_state.locomotion
                == newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Idle;
        // Before a native step starts, a small upper-body anticipation may follow view yaw. Once
        // the turn clip owns the body this procedural layer is disabled completely.
        let body_turn_target_yaw = if native_turn_allowed {
            view_body_yaw_delta.clamp(-32.0_f32.to_radians(), 32.0_f32.to_radians())
        } else {
            0.0
        };
        let model_to_world = root_transform.to_mat4() * model_root_local.to_mat4();
        let first_person_eye_model = if first_person_active {
            world
                .get::<newengine_engine_runtime::gameplay::PlayerFirstPersonCameraAnchor>(player)
                .copied()
                .filter(|anchor| anchor.eye_center_ws.is_finite())
                .map(|anchor| {
                    model_to_world
                        .inverse()
                        .transform_point3(anchor.eye_center_ws)
                })
                .filter(|position| position.is_finite())
        } else {
            None
        };
        let previous_foot_pose = world
            .get::<newengine_model_contact_api::ModelFootPoseState>(player)
            .copied();
        let next_foot_pose_revision = previous_foot_pose
            .map(|pose| pose.revision.saturating_add(1).max(1))
            .unwrap_or(1);
        let root_velocity_local = root_transform.rotation.inverse() * world_velocity;
        let root_position = root_transform.position;
        let root_rotation = root_transform.rotation;
        let mut timeline_events = Vec::new();
        let mut turn_step_request = None;
        let (palette, clip_ref, active_state, foot_pose) = {
            let Some(binding) = world.get_mut::<PlayerAnimationRuntimeBinding>(player) else {
                continue;
            };
            let mut event_occurrences = Vec::new();
            binding.equipment_time_seconds += dt;
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
                if let Some(reload) = binding.equipment_reload_pose.as_mut() {
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
                    continue;
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
                continue;
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
                continue;
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
            // Native turn-in-place keeps the authored full-body step, but extracts facing into
            // the simulation root continuously. The actor follows the visible weight transfer
            // instead of staying frozen and jumping 45/90/135/180 degrees at clip completion.
            if !native_turn_allowed && binding.turn_in_place.is_some() {
                binding.turn_in_place = None;
            }
            if native_turn_allowed && binding.turn_in_place.is_none() {
                if let Some(slot) = nearest_turn_in_place_slot(view_body_yaw_delta, |candidate| {
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
                    binding.body_turn_yaw_radians = 0.0;
                    let selected_turn_ref = binding
                        .turn_clip(slot)
                        .map(|clip| clip.clip_ref.as_str())
                        .unwrap_or("<missing>");
                    newengine_ulog_api::ulog::info!(
                        "game-ready: native turn-in-place started player={} side={} angle_deg={:.0} view_delta_deg={:.1} clip={} policy=authored-stepping-continuous-yaw",
                        player.stable_u64(),
                        if slot.signed_yaw_radians() > 0.0 { "left" } else { "right" },
                        slot.angle_degrees(),
                        view_body_yaw_delta.to_degrees(),
                        selected_turn_ref,
                    );
                }
            }
            let active_turn = binding.turn_in_place;
            if let Some(mut turn) = active_turn {
                // Accumulate accepted simulation yaw without wrapping the whole 180-degree turn.
                turn.applied_yaw_radians = accumulate_turn_in_place_yaw(
                    turn.applied_yaw_radians,
                    turn.last_body_yaw,
                    body_yaw,
                );
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
                        if clip_finished
                            && final_error.abs() <= TURN_IN_PLACE_FINISH_EPSILON_RADIANS
                        {
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
                            binding.body_turn_yaw_radians = 0.0;
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

            let native_turn_active = binding.turn_in_place.is_some();

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
                            (binding.equipment_time_seconds.rem_euclid(duration) / duration)
                                .clamp(0.0, 1.0)
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
                    let overlay_ref = binding
                        .equipment_reload_pose
                        .as_ref()
                        .map(|clip| clip.clip_ref.clone())
                        .unwrap_or_else(|| "<none>".to_owned());
                    if let Err(error) = apply_equipment_rotation_overlay(
                        binding.equipment_reload_pose.as_ref(),
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
                    if let Some(reload) = binding.equipment_reload_pose.as_mut() {
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
                    if let Err(error) = apply_equipment_rotation_overlay(
                        binding.equipment_ready_pose.as_ref(),
                        &binding.animation_runtime,
                        &mut binding.equipment_overlay_locals,
                        &mut binding.sampled_target_locals,
                        binding.equipment_ready_sample_phase,
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
                        let aim_phase = binding
                            .equipment_aim_pose
                            .as_ref()
                            .map(|clip| {
                                let duration = clip.clip.duration_seconds.max(1.0 / 30.0);
                                (binding.equipment_time_seconds.rem_euclid(duration) / duration)
                                    .clamp(0.0, 1.0)
                            })
                            .unwrap_or(0.0);
                        if let Err(error) = apply_equipment_rotation_overlay(
                            binding.equipment_aim_pose.as_ref(),
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
                        if let Some(aim) = binding.equipment_aim_pose.as_mut() {
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
                if let Some(aim) = binding.equipment_aim_pose.as_mut() {
                    let _ = aim.event_cursor.seek(binding.equipment_time_seconds);
                }
            }

            let requested_fall_band = if fall_presentation_requested {
                select_fall_presentation_band(
                    fall_state.distance,
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
                        fall_state.distance,
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
                            fall_state.distance,
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

            // Landing response is driven by a persistent impact revision because PlayerFallState
            // is reset on the grounded physics tick. This lets authored character-native landing
            // performances play on the following animation frames without replaying every frame.
            if landing_state.revision > binding.landing_last_revision {
                binding.landing_last_revision = landing_state.revision;
                let band = select_fall_presentation_band(
                    landing_state.distance,
                    binding.landing_soft_pose.is_some(),
                    binding.landing_medium_pose.is_some(),
                    binding.landing_hard_pose.is_some() || binding.landing_hard_run_pose.is_some(),
                    binding.fall_medium_min_distance,
                    binding.fall_high_min_distance,
                );
                binding.landing_active_band = band;
                binding.landing_active_run = matches!(band, Some(FallPresentationBand::High))
                    && binding.landing_hard_run_pose.is_some()
                    && landing_state.horizontal_speed > 1.5;
                binding.landing_time_seconds = 0.0;
                let selected = match (band, binding.landing_active_run) {
                    (Some(FallPresentationBand::Low), _) => binding.landing_soft_pose.as_mut(),
                    (Some(FallPresentationBand::Medium), _) => binding.landing_medium_pose.as_mut(),
                    (Some(FallPresentationBand::High), true) => {
                        binding.landing_hard_run_pose.as_mut()
                    }
                    (Some(FallPresentationBand::High), false) => {
                        binding.landing_hard_pose.as_mut()
                    },
                    (None, _) => None,
                };
                if let Some(clip) = selected {
                    clip.event_cursor.restart();
                    newengine_ulog_api::ulog::info!(
                        "game-ready: landing presentation selected player={} band={:?} distance_m={:.3} downward_speed={:.3} horizontal_speed={:.3} clip='{}'",
                        player.stable_u64(),
                        band.expect("selected landing band"),
                        landing_state.distance,
                        landing_state.downward_speed,
                        landing_state.horizontal_speed,
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
                        (FallPresentationBand::High, true) => {
                            binding.landing_hard_run_pose.as_mut()
                        }
                        (FallPresentationBand::High, false) => {
                            binding.landing_hard_pose.as_mut()
                        },
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
                                landing_state.distance,
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

            synchronize_helper_pose(
                &binding.helper_pose_copies,
                &mut binding.sampled_target_locals,
            );

            binding
                .current_locals
                .clone_from(&binding.sampled_target_locals);

            let torso_target =
                if noclip_enabled || fall_presentation_requested || native_turn_active {
                    0.0
                } else {
                    body_turn_target_yaw
                };
            binding.body_turn_yaw_radians =
                smooth_body_turn_yaw(binding.body_turn_yaw_radians, torso_target, dt);
            if let Err(error) = apply_body_turn_pose(
                &binding.skeleton,
                &binding.animation_runtime,
                &binding.body_turn_layers,
                &mut binding.current_locals,
                &mut binding.joint_frames_scratch,
                binding.body_turn_yaw_radians,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: procedural turn-in-place torso solve failed player={} yaw_deg={:.2}: {}",
                    player.stable_u64(),
                    binding.body_turn_yaw_radians.to_degrees(),
                    error,
                );
            }

            if equipment_presentation_active {
                match apply_equipped_weapon_support_ik(
                    weapon_presentation
                        .as_ref()
                        .expect("active equipment presentation requires weapon descriptor"),
                    binding.equipment_ik.as_ref(),
                    &binding.skeleton,
                    &binding.animation_runtime,
                    &mut binding.current_locals,
                    &mut binding.joint_frames_scratch,
                    rifle_view_forward_model,
                    rifle_view_rotation_model,
                    first_person_eye_model,
                    first_person_active,
                    rifle_aim_alpha,
                    rifle_recoil_alpha,
                    rifle_recoil_yaw_radians,
                    rifle_obstruction_alpha,
                    rifle_secondary_rotation_offset_local,
                    equipment_stance != EquipmentPresentationStance::Reload
                        && binding.equipment_ready_pose.is_some(),
                    rifle_reload_progress
                        .map(|progress| progress <= 0.08 || progress >= 0.92)
                        .unwrap_or(true),
                    rifle_reload_progress
                        .map(|progress| progress <= 0.08 || progress >= 0.92)
                        .unwrap_or(true),
                ) {
                    Ok(Some(result)) => {
                        binding.equipment_resolved_weapon_root = Some(result.base_root);
                        if result.error_m > 0.025 {
                            newengine_ulog_api::ulog::warn!(
                                "game-ready: authored equipment support IK residual player={} error_m={:.5}",
                                player.stable_u64(),
                                result.error_m,
                            );
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        newengine_ulog_api::ulog::warn!(
                            "game-ready: authored equipment support IK failed player={}: {}",
                            player.stable_u64(),
                            error,
                        );
                    }
                }
            }
            let continuity_key = PoseContinuityKey {
                clip_hash: animation_source_hash(&clip_ref),
                turn_sequence: binding.turn_sequence,
                unarmed_attack_sequence,
                equipment_stance: equipment_stance as u8,
            };
            binding
                .pose_continuity
                .apply(continuity_key, &mut binding.current_locals, dt);
            synchronize_helper_pose(&binding.helper_pose_copies, &mut binding.current_locals);
            if let Err(error) = stabilize_eye_locals(
                binding.eye_contract.as_ref(),
                &binding.skeleton,
                &mut binding.current_locals,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: authored eye-local stabilization failed player={} clip='{}': {}",
                    player.stable_u64(),
                    clip_ref,
                    error
                );
                continue;
            }
            binding
                .pose_continuity
                .commit_visible_pose(&binding.current_locals);

            if let Err(error) = binding
                .animation_runtime
                .build_skin_palette_from_local_pose(
                    &binding.current_locals,
                    &mut binding.palette_scratch,
                )
            {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: player skin palette update failed player={} state='{}' clip='{}': {}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    clip_ref,
                    error
                );
                continue;
            }
            if let Err(error) = apply_detached_head_follow_palette(
                binding.head_follow.as_ref(),
                &mut binding.palette_scratch,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: detached face/head follow projection failed player={} clip='{}': {}",
                    player.stable_u64(),
                    clip_ref,
                    error
                );
                continue;
            }
            if let Err(error) =
                validate_eye_palette(binding.eye_contract.as_ref(), &binding.palette_scratch)
            {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: authored eye palette rejected player={} clip='{}': {}",
                    player.stable_u64(),
                    clip_ref,
                    error
                );
                continue;
            }
            if transitioned {
                debug_dump_eye_matrices(
                    binding.eye_contract.as_ref(),
                    &binding.bind_joint_frames,
                    &binding.current_locals,
                    &binding.palette_scratch,
                    &format!("transition:{clip_ref}"),
                );
            }
            binding.joint_frames_scratch.clear();
            binding
                .joint_frames_scratch
                .reserve(binding.skeleton.joints.len());
            for index in 0..binding.skeleton.joints.len() {
                // Skin palette is a deformation matrix. Multiplying it by the authored bind
                // frame reconstructs the absolute current-frame joint transform after all
                // animation/head-follow corrections: P * (S*B) = S*A.
                let frame = binding.palette_scratch[index] * binding.bind_joint_frames[index];
                binding.joint_frames_scratch.push(frame);
            }
            let foot_pose = if noclip_enabled {
                None
            } else {
                binding.foot_joints.and_then(|feet| {
                    let left = binding.joint_frames_scratch.get(feet.left)?;
                    let right = binding.joint_frames_scratch.get(feet.right)?;
                    let left_bind = binding.bind_joint_frames.get(feet.left)?;
                    let right_bind = binding.bind_joint_frames.get(feet.right)?;

                    // Skeleton foot anchors normally sit at the ankle/foot-bone origin rather than
                    // on the shoe sole. Calibrate that static bind-height out before contact testing.
                    // X/Z remain animated joint truth; only the authored rest height becomes y=0.
                    let left_bind_y = left_bind.transform_point3(Vec3::ZERO).y.clamp(-0.30, 0.40);
                    let right_bind_y = right_bind.transform_point3(Vec3::ZERO).y.clamp(-0.30, 0.40);
                    let left_model = left.transform_point3(Vec3::ZERO) - Vec3::Y * left_bind_y;
                    let right_model = right.transform_point3(Vec3::ZERO) - Vec3::Y * right_bind_y;
                    let left_world = model_to_world.transform_point3(left_model);
                    let right_world = model_to_world.transform_point3(right_model);
                    Some(
                        newengine_model_contact_api::ModelFootPoseState::from_world_positions(
                            next_foot_pose_revision,
                            left_world,
                            right_world,
                            previous_foot_pose,
                            dt,
                        ),
                    )
                })
            };
            let (braid_secondary_motion, joint_frames_scratch, palette_scratch) = (
                &mut binding.braid_secondary_motion,
                &binding.joint_frames_scratch,
                &mut binding.palette_scratch,
            );
            if let Some(braid) = braid_secondary_motion.as_mut() {
                if let Err(error) = braid.tick(
                    dt,
                    root_velocity_local,
                    root_position,
                    root_rotation,
                    joint_frames_scratch,
                    palette_scratch,
                ) {
                    newengine_ulog_api::ulog::warn!(
                        "game-ready: native braid secondary motion update failed player={} clip='{}': {}",
                        player.stable_u64(),
                        clip_ref,
                        error
                    );
                    continue;
                }
            }
            let expected_palette_joints = binding.skeleton.joints.len();
            if let Err(error) = super::validation::validate_player_palette(
                &binding.palette_scratch,
                expected_palette_joints,
                &clip_ref,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: unstable player skin palette rejected player={} state='{}' clip='{}': {}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    clip_ref,
                    error
                );
                continue;
            }
            (
                std::mem::take(&mut binding.palette_scratch),
                clip_ref,
                active_state,
                foot_pose,
            )
        };

        if let Some(foot_pose) = foot_pose {
            let _ = world.insert(player, foot_pose);
        }

        let recycled_palette = if let Some(pose) =
            world.get_mut::<newengine_engine_runtime::gameplay::PlayerSkinPose>(player)
        {
            let recycled = std::mem::replace(&mut pose.palette, palette);
            pose.revision = pose.revision.saturating_add(1).max(1);
            Some(recycled)
        } else {
            let _ = world.insert(
                player,
                newengine_engine_runtime::gameplay::PlayerSkinPose {
                    palette,
                    revision: 1,
                },
            );
            None
        };
        if let Some(recycled_palette) = recycled_palette {
            if let Some(binding) = world.get_mut::<PlayerAnimationRuntimeBinding>(player) {
                binding.palette_scratch = recycled_palette;
            }
        }
        if let Some(yaw_delta) = turn_step_request {
            let _ = world.insert(
                player,
                newengine_sim::CharacterFacingTurnStepRequest { yaw_delta },
            );
        }
        crate::player_hair::publish_player_hair_pose(world, player, model_to_world);
        crate::animation_events::publish_timeline_events(world, timeline_events);

        if dt > 0.0
            && world
                .get::<newengine_engine_runtime::gameplay::PlayerSkinPose>(player)
                .is_some_and(|pose| pose.revision == 2)
        {
            newengine_ulog_api::ulog::info!(
                "game-ready: first animated player palette committed player={} state='{}' clip='{}'",
                player.stable_u64(),
                active_state.clip_hint(),
                clip_ref
            );
        }
    }
}

#[cfg(test)]
mod equipment_stance_tests {
    use super::*;
    use newengine_engine_runtime::gameplay::{PlayerWeaponState, WeaponType};

    #[test]
    fn firearm_equipment_stance_resolves_ready_aim_reload() {
        let mut state = PlayerWeaponState::melee();
        assert_eq!(
            resolve_equipment_presentation_stance(Some(WeaponType::Firearm), Some(state), true),
            EquipmentPresentationStance::Ready
        );
        state.aiming = true;
        assert_eq!(
            resolve_equipment_presentation_stance(Some(WeaponType::Firearm), Some(state), true),
            EquipmentPresentationStance::Aim
        );
        state.reload_remaining = 0.5;
        assert_eq!(
            resolve_equipment_presentation_stance(Some(WeaponType::Firearm), Some(state), true),
            EquipmentPresentationStance::Reload
        );
    }

    #[test]
    fn unarmed_and_melee_never_activate_firearm_presentation() {
        for weapon_type in [WeaponType::Unarmed, WeaponType::Melee] {
            assert_eq!(
                resolve_equipment_presentation_stance(
                    Some(weapon_type),
                    Some(PlayerWeaponState::melee()),
                    true,
                ),
                EquipmentPresentationStance::None
            );
        }
        assert_eq!(
            resolve_equipment_presentation_stance(
                Some(WeaponType::Firearm),
                Some(PlayerWeaponState::melee()),
                false,
            ),
            EquipmentPresentationStance::None
        );
    }
}
