#[derive(Clone, Copy, Debug, Default)]
struct PlayerAnimationFinalizeTiming {
    pose_copy_ms: f32,
    look_ms: f32,
    support_ik_ms: f32,
    continuity_eye_ms: f32,
    palette_ms: f32,
    joint_frames_ms: f32,
    braid_ms: f32,
    validation_ms: f32,
    overhead_ms: f32,
}

/// Phase 3: compose the visible pose, solve authored IK, enforce continuity/eye invariants,
/// build the skin palette, derive foot contacts and run secondary motion.
fn finalize_player_pose_and_palette(
    player: newengine_ecs::EntityId,
    binding: &mut PlayerAnimationRuntimeBinding,
    dt: f32,
    frame: &PlayerAnimationFrameInput,
    clip_ref: &str,
    active_state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
    unarmed_attack_sequence: u64,
    equipment_stance: EquipmentPresentationStance,
    transitioned: bool,
) -> Option<(
    Vec<Mat4>,
    Option<newengine_model_contact_api::ModelFootPoseState>,
    PlayerAnimationFinalizeTiming,
)> {
    let finalize_started = std::time::Instant::now();
    let mut timing = PlayerAnimationFinalizeTiming::default();
    let semantic = frame.semantic;
    let look_context = semantic.look_context;
    let noclip_enabled = semantic.noclip_enabled;
    let fall_presentation_requested = frame.fall_presentation_requested;
    let rifle_aim_alpha = semantic.aim_alpha;
    let rifle_recoil_alpha = semantic.recoil_alpha;
    let rifle_recoil_yaw_radians = semantic.recoil_yaw_radians;
    let rifle_obstruction_alpha = semantic.obstruction_alpha;
    let rifle_reload_progress = semantic.reload_progress;
    let equipment_presentation_active = frame.equipment_presentation_active;
    let weapon_presentation = frame.weapon_presentation.as_ref();
    let rifle_view_forward_model = frame.rifle_view_forward_model;
    let rifle_view_rotation_model = frame.rifle_view_rotation_model;
    let first_person_eye_model = frame.first_person_eye_model;
    let first_person_active = frame.first_person_active;
    let rifle_secondary_rotation_offset_local = frame.rifle_secondary_rotation_offset_local;
    let view_body_yaw_delta = frame.view_body_yaw_delta;
    let view_pitch = frame.view_pitch;
    let model_to_world = frame.model_to_world;
    let next_foot_pose_revision = frame.next_foot_pose_revision;
    let previous_foot_pose = frame.previous_foot_pose;
    let root_velocity_local = frame.root_velocity_local;
    let root_position = frame.root_position;
    let root_rotation = frame.root_rotation;
    let phase_started = std::time::Instant::now();
    synchronize_helper_pose(
        &binding.helper_pose_copies,
        &mut binding.sampled_target_locals,
    );

    binding
        .current_locals
        .clone_from(&binding.sampled_target_locals);
    timing.pose_copy_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    // Original-content look-at contract: select the authored state range, solve the view
    // intent inside its native 2D sample cloud, then give only the uncovered residual to
    // the eye range. No procedural neck/spine weights or engine-defined head angle clamps.
    let phase_started = std::time::Instant::now();
    let look_allowed = !noclip_enabled
        && !fall_presentation_requested
        && unarmed_attack_sequence == 0
        && equipment_stance != EquipmentPresentationStance::Reload;
    if look_allowed {
        let look_state = resolve_authored_look_state(active_state, equipment_stance, look_context);
        let _ = binding.authored_look.apply(
            look_state,
            view_body_yaw_delta,
            view_pitch,
            &mut binding.current_locals,
        );
    }

    timing.look_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    // Pose continuity belongs to the authored/base pose, before terminal procedural contacts.
    // Blending a previously visible pose *after* weapon IK reintroduces the old arm transforms and
    // visibly detaches the palms from the weapon during Ready/Aim/locomotion transitions. Blend the
    // base pose first; then let weapon IK be the last writer for the arm chains.
    let phase_started = std::time::Instant::now();
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
    timing.continuity_eye_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    let phase_started = std::time::Instant::now();
    let equipment_ready_pose_present = select_equipment_pose_set(
        &binding.equipment_default_pose_set,
        &binding.equipment_pose_sets,
        frame.equipment_pose_family.as_deref(),
    )
    .and_then(|set| set.ready.as_ref())
    .is_some();
    if equipment_presentation_active {
        // Support IK is valid only when both sides of the authored binding resolved:
        // a sanitized weapon presentation and a skeleton-resolved arm IK rig. Poses may
        // still be authored without IK, but that must not manufacture a procedural target.
        if let (Some(presentation), Some(rig)) =
            (weapon_presentation.as_ref(), binding.equipment_ik.as_ref())
        {
            match apply_equipped_weapon_support_ik(
                presentation,
                Some(rig),
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
                    && equipment_ready_pose_present,
                rifle_reload_progress
                    .map(|progress| progress <= 0.08 || progress >= 0.92)
                    .unwrap_or(true),
                rifle_reload_progress
                    .map(|progress| progress <= 0.08 || progress >= 0.92)
                    .unwrap_or(true),
            ) {
                Ok(Some(result)) => {
                    binding.equipment_resolved_weapon_root = Some(result.base_root);
                    if result.error_m > EQUIPMENT_SUPPORT_IK_RESIDUAL_WARN_THRESHOLD_M
                        && binding.equipment_ik_residual_diag_cooldown <= 0.0
                    {
                        newengine_ulog_api::ulog::warn!(
                            "game-ready: authored equipment support IK residual player={} error_m={:.5} right_error_m={:.5} left_error_m={:.5} threshold_m={:.5} diagnostic_interval_s={:.1}",
                            player.stable_u64(),
                            result.error_m,
                            result.right_error_m,
                            result.left_error_m,
                            EQUIPMENT_SUPPORT_IK_RESIDUAL_WARN_THRESHOLD_M,
                            EQUIPMENT_SUPPORT_IK_RESIDUAL_DIAG_INTERVAL_SECONDS,
                        );
                        binding.equipment_ik_residual_diag_cooldown =
                            EQUIPMENT_SUPPORT_IK_RESIDUAL_DIAG_INTERVAL_SECONDS;
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    if binding.equipment_ik_residual_diag_cooldown <= 0.0 {
                        newengine_ulog_api::ulog::warn!(
                            "game-ready: authored equipment support IK failed player={}: {}",
                            player.stable_u64(),
                            error,
                        );
                        binding.equipment_ik_residual_diag_cooldown =
                            EQUIPMENT_SUPPORT_IK_RESIDUAL_DIAG_INTERVAL_SECONDS;
                    }
                }
            }
        }
    }
    timing.support_ik_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    let phase_started = std::time::Instant::now();
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
        return None;
    }
    binding
        .pose_continuity
        .commit_visible_pose(&binding.current_locals);

    timing.continuity_eye_ms += phase_started.elapsed().as_secs_f32() * 1000.0;

    let phase_started = std::time::Instant::now();
    if let Err(error) = binding
        .animation_runtime
        .build_skin_palette_from_local_pose(&binding.current_locals, &mut binding.palette_scratch)
    {
        newengine_ulog_api::ulog::warn!(
            "game-ready: player skin palette update failed player={} state='{}' clip='{}': {}",
            player.stable_u64(),
            active_state.clip_hint(),
            clip_ref,
            error
        );
        return None;
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
        return None;
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
        return None;
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
    timing.palette_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    let phase_started = std::time::Instant::now();
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
    timing.joint_frames_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    let phase_started = std::time::Instant::now();
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
            return None;
        }
    }
    timing.braid_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    let phase_started = std::time::Instant::now();
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
        return None;
    }

    timing.validation_ms = phase_started.elapsed().as_secs_f32() * 1000.0;
    let measured_ms = timing.pose_copy_ms
        + timing.look_ms
        + timing.support_ik_ms
        + timing.continuity_eye_ms
        + timing.palette_ms
        + timing.joint_frames_ms
        + timing.braid_ms
        + timing.validation_ms;
    timing.overhead_ms = (finalize_started.elapsed().as_secs_f32() * 1000.0 - measured_ms).max(0.0);

    Some((
        std::mem::take(&mut binding.palette_scratch),
        foot_pose,
        timing,
    ))
}

/// Phase 4: publish the evaluated frame back to ECS/resources only after all mutable binding work
/// has completed. This keeps side-effect ordering explicit and prevents nested world borrows.
fn commit_player_animation_frame(
    world: &mut newengine_ecs::World,
    player: newengine_ecs::EntityId,
    dt: f32,
    output: PlayerAnimationFrameOutput,
) {
    let PlayerAnimationFrameOutput {
        palette,
        clip_ref,
        active_state,
        foot_pose,
        turn_step_request,
        model_to_world,
        timeline_events,
        presentation_core_ms: _,
        finalize_ms: _,
        finalize_timing: _,
    } = output;
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
