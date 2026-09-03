include!("clip_loading/io.rs");

include!("clip_loading/presentation.rs");

pub(super) fn prepare_player_animation_binding(
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    parts: &[PlayerRuntimeModelPart],
    skeleton: Option<&ModelSkeletonMetadata>,
) -> Result<Option<PlayerAnimationRuntimeBinding>, String> {
    use newengine_engine_runtime::gameplay::PlayerLocomotionAnimation as L;

    let skinned_parts = parts
        .iter()
        .filter_map(|part| part.skin.as_ref())
        .collect::<Vec<_>>();
    if skinned_parts.is_empty() {
        return Ok(None);
    }
    let skeleton = skeleton
        .ok_or_else(|| "skinned player model requires authored skeleton metadata".to_owned())?;
    let source_to_model = skinned_parts[0].source_to_model;
    for (part_index, skin) in skinned_parts.iter().enumerate() {
        if skin.source_to_model != source_to_model {
            return Err(format!(
                "skinned player model source-space transform mismatch part={part_index}"
            ));
        }
    }
    let animation_runtime = AnimationSkeletonRuntime::compile(skeleton, source_to_model)
        .map_err(|error| format!("player animation skeleton compile failed: {error}"))?;

    let Some(idle_ref) = assignment.animation_for_slot("locomotion.idle") else {
        return Ok(None);
    };
    let mut clips: [Option<PlayerAnimationRuntimeClip>; 8] =
        [None, None, None, None, None, None, None, None];
    clips[locomotion_slot(L::Idle)] = Some(load_runtime_animation_clip(
        idle_ref,
        assignment,
        skeleton,
        &animation_runtime,
    )?);

    for (state, reference) in [
        (L::Walk, assignment.animation_for_slot("locomotion.walk")),
        (L::Run, assignment.animation_for_slot("locomotion.run")),
        (
            L::Sprint,
            assignment.animation_for_slot("locomotion.sprint"),
        ),
        (
            L::CrouchIdle,
            assignment.animation_for_slot("locomotion.crouch_idle"),
        ),
        (
            L::CrouchWalk,
            assignment.animation_for_slot("locomotion.crouch_walk"),
        ),
        (L::Jump, assignment.animation_for_slot("locomotion.jump")),
        (L::Fall, assignment.animation_for_slot("locomotion.fall")),
    ] {
        if let Some(reference) = reference {
            clips[locomotion_slot(state)] = Some(load_runtime_animation_clip(
                reference,
                assignment,
                skeleton,
                &animation_runtime,
            )?);
        }
    }

    let locomotion_graph = compile_game_ready_locomotion_graph(&clips, &animation_runtime)
        .map_err(|error| format!("FPS character locomotion graph compile failed: {error}"))?;
    let mut locomotion_graph_instance = AnimationGraphInstance::new(&locomotion_graph);
    let mut locomotion_graph_evaluation = AnimationGraphEvaluation::default();
    locomotion_graph_instance
        .evaluate(
            &locomotion_graph,
            &animation_runtime,
            0.0,
            &mut locomotion_graph_evaluation,
        )
        .map_err(|error| {
            format!("FPS character locomotion graph initial evaluation failed: {error}")
        })?;
    let helper_pose_copies = match resolve_helper_pose_copy_rules(
        skeleton,
        &assignment.presentation.helper_pose_copies,
    ) {
        Ok(rules) => rules,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "fps-character: player presentation degraded capability='helper_pose_copies' err='{}' action='keep locomotion animation'",
                error
            );
            Vec::new()
        }
    };
    // Rig reconstruction is definition-authored. Runtime only resolves exact authored joints and
    // executes generic copy/follow operators; no character, franchise, or naming convention is inferred.
    let head_follow = if !assignment.presentation.detached_head_follow {
        None
    } else if let Some(rule) = assignment.presentation.detached_head_follow_rule.as_ref() {
        match build_detached_head_follow(skeleton, rule) {
            Ok(binding) => Some(binding),
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "fps-character: player presentation degraded capability='detached_head_follow' err='{}' action='keep locomotion animation'",
                    error
                );
                None
            }
        }
    } else {
        newengine_ulog_api::ulog::warn!(
            "fps-character: player presentation degraded capability='detached_head_follow' reason='authored follow rule unavailable' action='keep locomotion animation'"
        );
        None
    };
    let eye_contract = if !assignment.presentation.eye_parent_follow {
        None
    } else if let Some(rule) = assignment.presentation.eye_parent_follow_rule.as_ref() {
        match build_eye_runtime_contract(skeleton, rule) {
            Ok(binding) => Some(binding),
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "fps-character: player presentation degraded capability='eye_parent_follow' err='{}' action='keep locomotion animation'",
                    error
                );
                None
            }
        }
    } else {
        newengine_ulog_api::ulog::warn!(
            "fps-character: player presentation degraded capability='eye_parent_follow' reason='authored eye contract unavailable' action='keep locomotion animation'"
        );
        None
    };
    let bind_locals = animation_runtime.bind_locals().to_vec();
    let bind_joint_frames = animation_runtime.bind_joint_frames().to_vec();
    let skeletal_secondary_motion = match prepare_skeletal_secondary_motion(
        parts,
        skeleton,
        assignment.presentation.skeletal_secondary_motion.as_ref(),
        source_to_model,
        &bind_joint_frames,
    ) {
        Ok(binding) => binding,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "fps-character: player presentation degraded capability='skeletal_secondary_motion' err='{}' action='keep locomotion animation'",
                error
            );
            None
        }
    };
    let mut current_locals = locomotion_graph_evaluation.local_pose.clone();
    synchronize_helper_pose(&helper_pose_copies, &mut current_locals);
    stabilize_eye_locals(eye_contract.as_ref(), skeleton, &mut current_locals)?;
    let mut palette_scratch = Vec::with_capacity(skeleton.joints.len());
    animation_runtime.build_skin_palette_from_local_pose(&current_locals, &mut palette_scratch)?;
    apply_detached_head_follow_palette(head_follow.as_ref(), &mut palette_scratch)?;
    validate_eye_palette(eye_contract.as_ref(), &palette_scratch)?;
    debug_dump_eye_matrices(
        eye_contract.as_ref(),
        &bind_joint_frames,
        &current_locals,
        &palette_scratch,
        "initial",
    );
    let (equipment_default_pose_set, equipment_pose_sets) =
        load_equipment_pose_sets(assignment, skeleton, &animation_runtime)?;
    let unarmed_ready_pose = load_authored_presentation_clip(
        "unarmed_ready",
        assignment.animation_for_slot("unarmed.ready"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    let unarmed_attack_pose = load_authored_presentation_clip(
        "unarmed_attack",
        assignment.animation_for_slot("unarmed.attack"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    let turn_45_left_pose = load_authored_presentation_clip(
        "turn_45_left",
        assignment.animation_for_slot("turn.left.45"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    let turn_45_right_pose = load_authored_presentation_clip(
        "turn_45_right",
        assignment.animation_for_slot("turn.right.45"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    let turn_90_left_pose = load_authored_presentation_clip(
        "turn_90_left",
        assignment.animation_for_slot("turn.left.90"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    let turn_90_right_pose = load_authored_presentation_clip(
        "turn_90_right",
        assignment.animation_for_slot("turn.right.90"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    let turn_135_left_pose = load_authored_presentation_clip(
        "turn_135_left",
        assignment.animation_for_slot("turn.left.135"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    let turn_135_right_pose = load_authored_presentation_clip(
        "turn_135_right",
        assignment.animation_for_slot("turn.right.135"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    let turn_180_left_pose = load_authored_presentation_clip(
        "turn_180_left",
        assignment.animation_for_slot("turn.left.180"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    let turn_180_right_pose = load_authored_presentation_clip(
        "turn_180_right",
        assignment.animation_for_slot("turn.right.180"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    // NoClip is a full-body traversal mode, not a degradable upper-body presentation overlay.
    // If a character authors a seated-flight clip, accepting a load/binding failure and falling
    // back to locomotion would make the character visibly walk/run while collisionless. Treat the
    // authored clip as required so the character either binds the intended seated pose or reports
    // an explicit model-binding error instead of silently presenting the wrong state.
    let noclip_pose = match assignment.animation_for_slot("traversal.noclip") {
        Some(reference) => Some(
            load_runtime_animation_clip(reference, assignment, skeleton, &animation_runtime)
                .map_err(|error| {
                    format!(
                        "required NoClip full-body animation unavailable ref='{reference}' err='{error}'"
                    )
                })?,
        ),
        None => None,
    };
    let fall_low_pose = load_authored_presentation_clip(
        "fall_low",
        assignment.animation_for_slot("fall.low"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    let fall_medium_pose = load_authored_presentation_clip(
        "fall_medium",
        assignment.animation_for_slot("fall.medium"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    let fall_high_pose = load_authored_presentation_clip(
        "fall_high",
        assignment.animation_for_slot("fall.high"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    let landing_soft_pose = load_authored_presentation_clip(
        "landing_soft",
        assignment.animation_for_slot("landing.soft"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    let landing_medium_pose = load_authored_presentation_clip(
        "landing_medium",
        assignment.animation_for_slot("landing.medium"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    let landing_hard_pose = load_authored_presentation_clip(
        "landing_hard",
        assignment.animation_for_slot("landing.hard"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    let landing_hard_run_pose = load_authored_presentation_clip(
        "landing_hard_run",
        assignment.animation_for_slot("landing.hard_run"),
        assignment,
        skeleton,
        &animation_runtime,
    )?;
    let fall_medium_min_distance = if assignment.presentation.fall_medium_min_distance.is_finite() {
        assignment.presentation.fall_medium_min_distance.max(0.0)
    } else {
        0.0
    };
    let fall_high_min_distance = if assignment.presentation.fall_high_min_distance.is_finite() {
        assignment
            .presentation
            .fall_high_min_distance
            .max(fall_medium_min_distance)
    } else {
        fall_medium_min_distance
    };

    let equipment_ready_sample_phase = if assignment
        .presentation
        .equipment_ready_sample_phase
        .is_finite()
    {
        assignment
            .presentation
            .equipment_ready_sample_phase
            .clamp(0.0, 1.0)
    } else {
        newengine_ulog_api::ulog::warn!(
            "fps-character: player presentation degraded capability='equipment_ready_sample_phase' reason='non-finite authored value' action='use phase 0'"
        );
        0.0
    };
    let equipment_ready_rotation_weights = match resolve_joint_blend_rules(
        skeleton,
        &assignment.presentation.equipment_ready_rotation_weights,
    ) {
        Ok(rules) => rules,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "fps-character: player presentation degraded capability='equipment_ready_rotation_weights' err='{}' action='disable overlay weights'",
                error
            );
            Vec::new()
        }
    };
    let equipment_aim_rotation_weights = match resolve_joint_blend_rules(
        skeleton,
        &assignment.presentation.equipment_aim_rotation_weights,
    ) {
        Ok(rules) => rules,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "fps-character: player presentation degraded capability='equipment_aim_rotation_weights' err='{}' action='disable overlay weights'",
                error
            );
            Vec::new()
        }
    };
    let equipment_reload_rotation_weights = match resolve_joint_blend_rules(
        skeleton,
        &assignment.presentation.equipment_reload_rotation_weights,
    ) {
        Ok(rules) => rules,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "fps-character: player presentation degraded capability='equipment_reload_rotation_weights' err='{}' action='disable overlay weights'",
                error
            );
            Vec::new()
        }
    };
    let equipment_ik = resolve_authored_equipment_arm_ik(skeleton, &assignment.presentation);
    let joint_frames_scratch = Vec::with_capacity(skeleton.joints.len());
    let foot_joints = resolve_foot_joint_binding(skeleton);
    let turn_root_joint = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == skeleton.anchors.root);
    let authored_look = load_authored_look_binding(assignment, skeleton, &animation_runtime)?;
    let sampled_target_locals = current_locals.clone();
    let pose_continuity = PoseContinuityBridge::new(&current_locals);
    if !helper_pose_copies.is_empty() {
        newengine_ulog_api::ulog::info!(
            "fps-character: authored joint-copy rig channels={} policy='definition rules -> generic local-pose copy before skin palette'",
            helper_pose_copies.len()
        );
    }
    if let Some(rig) = head_follow.as_ref() {
        newengine_ulog_api::ulog::info!(
            "fps-character: authored palette-follow contract driver={} followers={} reserved={} policy='definition-selected driver deformation -> authored follower branches'",
            rig.driver_joint,
            rig.followers.len(),
            0usize,
        );
    }
    if let Some(eyes) = eye_contract.as_ref() {
        newengine_ulog_api::ulog::info!(
            "fps-character: authored eye-parent contract left={} right={} parent={} policy='locomotion keeps authored eye-local bind; eye palette follows authored parent deformation'",
            eyes.left,
            eyes.right,
            eyes.parent,
        );
    }
    let authored_look_roles = [
        authored_look.relaxed.as_ref(),
        authored_look.crouch.as_ref(),
        authored_look.tense.as_ref(),
        authored_look.eyes.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(|space| {
        format!(
            "{}:{}samples/{}joints/{:.2}deg-turn-hysteresis",
            space.role,
            space.samples.len(),
            space.joints.len(),
            space.turn_hysteresis_radians.to_degrees(),
        )
    })
    .collect::<Vec<_>>();
    if !authored_look_roles.is_empty() {
        newengine_ulog_api::ulog::info!(
            "fps-character: authored look-at pose spaces [{}] policy='native base+range -> sampled 2D pose-space -> residual eyes -> native body turn'",
            authored_look_roles.join(", "),
        );
    }
    let native_turn_clip_count = [
        turn_45_left_pose.is_some(),
        turn_45_right_pose.is_some(),
        turn_90_left_pose.is_some(),
        turn_90_right_pose.is_some(),
        turn_135_left_pose.is_some(),
        turn_135_right_pose.is_some(),
        turn_180_left_pose.is_some(),
        turn_180_right_pose.is_some(),
    ]
    .into_iter()
    .filter(|ready| *ready)
    .count();
    if native_turn_clip_count > 0 {
        newengine_ulog_api::ulog::info!(
            "fps-character: native turn-in-place prepared clips={} policy=authored-step-bounded-fixed-step-yaw-no-snap-rebase",
            native_turn_clip_count,
        );
    }
    if equipment_default_pose_set.any() || !equipment_pose_sets.is_empty() || equipment_ik.is_some()
    {
        let families = equipment_pose_sets
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        newengine_ulog_api::ulog::info!(
            "fps-character: character equipment presentation prepared generic_ready='{}' generic_aim='{}' generic_reload='{}' families='{}' arm_ik={} ready_weights={} aim_weights={} reload_weights={} policy='equipped weapon class selects isolated authored pose family; generic slots apply only to unclassified legacy items; IK requires weapon presentation contract'",
            assignment
                .animation_for_slot("equipment.ready")
                .unwrap_or("none"),
            assignment
                .animation_for_slot("equipment.aim")
                .unwrap_or("none"),
            assignment
                .animation_for_slot("equipment.reload")
                .unwrap_or("none"),
            families,
            equipment_ik.is_some(),
            equipment_ready_rotation_weights.len(),
            equipment_aim_rotation_weights.len(),
            equipment_reload_rotation_weights.len(),
        );
    }

    Ok(Some(PlayerAnimationRuntimeBinding {
        clips,
        animation_event_bindings: assignment.presentation.animation_event_bindings.clone(),
        semantic_input: PlayerAnimationSemanticInput::default(),
        consumed_pulse_sequence: 0,
        active_state: L::Idle,
        active_slot: locomotion_slot(L::Idle),
        locomotion_graph,
        locomotion_graph_instance,
        locomotion_graph_evaluation,
        skeleton: skeleton.clone(),
        animation_runtime,
        current_locals,
        sampled_target_locals,
        palette_scratch,
        bind_joint_frames,
        joint_frames_scratch,
        foot_joints,
        turn_root_joint,
        turn_45_left_pose,
        turn_45_right_pose,
        turn_90_left_pose,
        turn_90_right_pose,
        turn_135_left_pose,
        turn_135_right_pose,
        turn_180_left_pose,
        turn_180_right_pose,
        turn_in_place: None,
        turn_sequence: 0,
        pose_continuity,
        authored_look,
        skeletal_secondary_motion,
        helper_pose_copies,
        eye_contract,
        head_follow,
        noclip_pose,
        noclip_time_seconds: 0.0,
        noclip_active: false,
        fall_low_pose,
        fall_medium_pose,
        fall_high_pose,
        landing_soft_pose,
        landing_medium_pose,
        landing_hard_pose,
        landing_hard_run_pose,
        landing_active_band: None,
        landing_active_run: false,
        landing_time_seconds: 0.0,
        landing_active_distance: 0.0,
        landing_active_downward_speed: 0.0,
        landing_active_horizontal_speed: 0.0,
        landing_last_revision: 0,
        fall_medium_min_distance,
        fall_high_min_distance,
        fall_active_band: None,
        fall_time_seconds: 0.0,
        equipment_default_pose_set,
        equipment_pose_sets,
        unarmed_ready_pose,
        unarmed_attack_pose,
        unarmed_attack_sequence: 0,
        unarmed_attack_time_seconds: 0.0,
        equipment_ready_sample_phase,
        equipment_time_seconds: 0.0,
        equipment_reload_active: false,
        equipment_previous_stance: EquipmentPresentationStance::None,
        equipment_transition: None,
        equipment_trace_active: false,
        equipment_trace_family: None,
        equipment_trace_stance: EquipmentPresentationStance::None,
        equipment_ready_rotation_weights,
        equipment_aim_rotation_weights,
        equipment_reload_rotation_weights,
        equipment_overlay_locals: bind_locals.clone(),
        equipment_overlay_locals_b: bind_locals,
        equipment_ik,
        equipment_ik_residual_diag_cooldown: 0.0,
        equipment_aim_controller: ThirdPersonWeaponAimState::default(),
        equipment_resolved_weapon_root: None,
    }))
}
