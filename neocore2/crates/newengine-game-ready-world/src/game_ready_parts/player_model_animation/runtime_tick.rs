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
        let rifle_aim_alpha = super::equipment_visual::equipped_weapon_aim_alpha(world, player);
        let rifle_recoil_alpha =
            super::equipment_visual::equipped_weapon_recoil_alpha(world, player);
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
        let weapon_presentation = world
            .get::<newengine_engine_runtime::gameplay::EquippedWeaponBinding>(player)
            .copied()
            .and_then(|equipped| {
                world
                    .resource::<newengine_engine_runtime::gameplay::ItemCatalog>()?
                    .get(equipped.item)
                    .map(|definition| definition.weapon_presentation.clone().sanitized())
            })
            .filter(|presentation| presentation.enabled);
        let equipment_presentation_active = weapon_presentation.is_some()
            && world
                .get::<PlayerAnimationRuntimeBinding>(player)
                .is_some_and(|binding| {
                    binding.equipment_ready_pose.is_some()
                        || binding.equipment_aim_pose.is_some()
                        || binding.equipment_reload_pose.is_some()
                        || binding.equipment_ik.is_some()
                });
        let rifle_reload_progress = if equipment_presentation_active {
            world
                .get::<newengine_engine_runtime::gameplay::PlayerWeaponState>(player)
                .and_then(|state| {
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
        let (palette, clip_ref, active_state) = {
            let Some(binding) = world.get_mut::<PlayerAnimationRuntimeBinding>(player) else {
                continue;
            };
            binding.equipment_time_seconds += dt;
            let desired_slot = binding.resolve_slot(animation_state.locomotion);
            let state_changed = binding.active_state != animation_state.locomotion;
            let slot_changed = binding.active_slot != desired_slot;
            let transitioned = state_changed || slot_changed;
            if slot_changed {
                // Cross-fade from the pose that was actually visible, not merely from
                // the previous clip. This keeps hands/forearms continuous even if the
                // player changes locomotion state again before the prior fade finishes.
                binding
                    .transition_from_locals
                    .clone_from(&binding.current_locals);
                binding.active_slot = desired_slot;
                binding.time_seconds = 0.0;
            }
            if state_changed {
                // A semantic transition is not necessarily a clip transition. Fall can
                // intentionally resolve to the active Jump slot when no authored fall
                // clip exists. Preserve playback time in that case so the airborne
                // phase continues through the apex instead of restarting the jump.
                binding.active_state = animation_state.locomotion;
            }
            if !slot_changed {
                let playback_rate = match animation_state.locomotion {
                    newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Walk => {
                        (animation_state.normalized_speed / 0.40).clamp(0.65, 1.45)
                    }
                    newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Run => {
                        (animation_state.normalized_speed / 0.85).clamp(0.75, 1.45)
                    }
                    newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Sprint => {
                        animation_state.normalized_speed.clamp(1.0, 1.65)
                    }
                    newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::CrouchWalk => {
                        // Authored crouch speed is ~1.0 m/s while normalized_speed is expressed
                        // against the 3.0 m/s run speed. Keep foot cadence centered at 1x at
                        // full crouch speed and only stretch modestly near the movement threshold.
                        (animation_state.normalized_speed / 0.333_333_34).clamp(0.70, 1.25)
                    }
                    _ => 1.0,
                };
                binding.time_seconds += dt * playback_rate;
            }

            let active_slot = binding.active_slot;
            let active_state = binding.active_state;
            let active_clip = binding.clips[active_slot]
                .as_ref()
                .expect("resolved player animation slot must contain a clip");
            let clip_ref = active_clip.clip_ref.clone();
            if transitioned {
                newengine_ulog_api::ulog::info!(
                    "game-ready: player locomotion animation transition player={} state='{}' clip='{}' duration={:.3}s normalized_speed={:.3}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    clip_ref,
                    active_clip.clip.duration_seconds,
                    animation_state.normalized_speed
                );
            }
            if let Err(error) = active_clip.clip.sample_local_pose_for_skeleton(
                binding.time_seconds,
                &binding.skeleton,
                &mut binding.sampled_target_locals,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: player animation sample failed player={} state='{}' clip='{}': {}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    clip_ref,
                    error
                );
                continue;
            }

            if equipment_presentation_active {
                if let Some(progress) = rifle_reload_progress {
                    let overlay = binding.equipment_reload_pose.as_ref();
                    let overlay_ref = overlay
                        .map(|clip| clip.clip_ref.as_str())
                        .unwrap_or("<none>");
                    if let Err(error) = apply_equipment_rotation_overlay(
                        overlay,
                        &binding.skeleton,
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
                } else {
                    if let Err(error) = apply_equipment_rotation_overlay(
                        binding.equipment_ready_pose.as_ref(),
                        &binding.skeleton,
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
                    if rifle_aim_alpha > 0.001 {
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
                    }
                }
            }
            synchronize_helper_pose(
                &binding.helper_mirror_pairs,
                &mut binding.sampled_target_locals,
            );

            let alpha = if state_changed && !slot_changed {
                // Same-slot semantic continuation (notably Jump -> Fall fallback) must
                // not re-enter a cross-fade against stale transition_from_locals.
                1.0
            } else {
                animation_state.transition_alpha.clamp(0.0, 1.0)
            };
            if alpha < 1.0 {
                if let Err(error) = blend_local_poses(
                    &binding.transition_from_locals,
                    &binding.sampled_target_locals,
                    alpha,
                    &mut binding.current_locals,
                ) {
                    newengine_ulog_api::ulog::warn!(
                        "game-ready: player animation transition failed player={} state='{}' clip='{}': {}",
                        player.stable_u64(),
                        active_state.clip_hint(),
                        clip_ref,
                        error
                    );
                    continue;
                }
            } else {
                binding
                    .current_locals
                    .clone_from(&binding.sampled_target_locals);
            }

            if equipment_presentation_active {
                match apply_equipped_weapon_support_ik(
                    weapon_presentation
                        .as_ref()
                        .expect("active equipment presentation requires weapon descriptor"),
                    binding.equipment_ik.as_ref(),
                    &binding.skeleton,
                    binding.source_to_model,
                    &mut binding.current_locals,
                    &mut binding.joint_frames_scratch,
                    rifle_view_forward_model,
                    rifle_aim_alpha,
                    rifle_recoil_alpha,
                    rifle_reload_progress
                        .map(|progress| progress <= 0.08 || progress >= 0.92)
                        .unwrap_or(true),
                    rifle_reload_progress
                        .map(|progress| progress <= 0.08 || progress >= 0.92)
                        .unwrap_or(true),
                ) {
                    Ok(Some(error)) if error > 0.025 => {
                        newengine_ulog_api::ulog::warn!(
                            "game-ready: authored equipment support IK residual player={} error_m={:.5}",
                            player.stable_u64(),
                            error,
                        );
                    }
                    Ok(_) => {}
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

            if let Err(error) = build_skin_palette_from_local_pose(
                &binding.skeleton,
                binding.source_to_model,
                &binding.current_locals,
                &mut binding.palette_scratch,
            ) {
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
            )
        };

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
