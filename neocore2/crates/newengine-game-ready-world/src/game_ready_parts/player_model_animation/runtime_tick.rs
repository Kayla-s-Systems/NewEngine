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
        let active_weapon = newengine_engine_runtime::gameplay::active_equipped_weapon_binding(world, player);
        let unarmed_active = active_weapon.is_some_and(|binding| {
            binding.weapon.weapon_type == newengine_engine_runtime::gameplay::WeaponType::Unarmed
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
        let rifle_secondary_rotation_offset_local =
            super::equipment_visual::equipped_weapon_secondary_rotation_offset_local(world, player);
        let first_person_active = world
            .resource::<newengine_engine_runtime::gameplay::PlayerViewState>()
            .copied()
            .unwrap_or_default()
            .first_person_active;
        let rifle_view_forward_model = if first_person_active || rifle_aim_alpha > 0.001 {
            player_rifle_view_forward_model(world, player)
        } else {
            None
        };
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
        let equipment_presentation_active = equipment_stance != EquipmentPresentationStance::None
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
        let model_to_world = root_transform.to_mat4() * model_root_local.to_mat4();
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

            let desired_slot = binding.resolve_slot(animation_state.locomotion);
            let state_changed = binding.active_state != animation_state.locomotion;
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
                binding.active_state = animation_state.locomotion;
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

            // Preserve the old GameReady phase contract: a newly selected clip is sampled at t=0
            // on the transition frame. From the next frame onward the generic graph owns clocks,
            // synchronization and cross-fade progression.
            let graph_dt = if slot_changed {
                0.0
            } else {
                dt * locomotion_playback_rate(animation_state)
            };
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
                    "game-ready: player locomotion graph transition player={} state='{}' graph_state='{}' clip='{}' duration={:.3}s normalized_speed={:.3}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    target_graph_state,
                    clip_ref,
                    duration,
                    animation_state.normalized_speed
                );
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
                        &binding.skeleton,
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
                        &binding.skeleton,
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
                            &binding.skeleton,
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
            synchronize_helper_pose(
                &binding.helper_mirror_pairs,
                &mut binding.sampled_target_locals,
            );

            binding
                .current_locals
                .clone_from(&binding.sampled_target_locals);

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
            synchronize_helper_pose(&binding.helper_mirror_pairs, &mut binding.current_locals);
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
            let foot_pose = binding.foot_joints.and_then(|feet| {
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
                Some(newengine_model_contact_api::ModelFootPoseState::from_world_positions(
                    next_foot_pose_revision,
                    left_world,
                    right_world,
                    previous_foot_pose,
                    dt,
                ))
            });
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
