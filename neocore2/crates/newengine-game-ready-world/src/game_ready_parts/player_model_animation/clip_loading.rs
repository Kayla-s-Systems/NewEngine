fn load_animation_clip(reference: &str) -> Result<std::sync::Arc<AnimationClip>, String> {
    let parsed = AnimationClipReference::parse(reference)?;
    if !parsed.logical_path.to_ascii_lowercase().ends_with(".ycd") {
        return Err(format!(
            "player animation must reference .ycd asset: '{reference}'"
        ));
    }
    let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
    global_animation_clip_store()
        .load_ycd_clip(reference, |logical_path| {
            assets
                .decode_v1(&AssetDecodeRequest {
                    logical_path: logical_path.to_owned(),
                    output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
                    selector: serde_json::Value::Null,
                })
                .map_err(|error| {
                    format!(
                        "player animation asset decode failed ref='{reference}' path='{logical_path}' err='{error}'"
                    )
                })
        })
        .map_err(|error| {
            format!("player animation shared clip load failed ref='{reference}': {error}")
        })
}

fn validate_animation_clip(
    clip_ref: &str,
    clip: &AnimationClip,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    skeleton: &ModelSkeletonMetadata,
) -> Result<(), String> {
    if !clip.skeleton_ref.trim().is_empty()
        && !clip
            .skeleton_ref
            .eq_ignore_ascii_case(assignment.skeleton_source.as_deref().unwrap_or_default())
    {
        return Err(format!(
            "player animation skeleton ref mismatch clip='{}' assignment='{}'",
            clip.skeleton_ref,
            assignment.skeleton_source.as_deref().unwrap_or("<none>")
        ));
    }
    for (clip_index, &tag) in clip.joint_tags.iter().enumerate() {
        if clip.joint_tags[..clip_index].contains(&tag) {
            return Err(format!(
                "player animation contains duplicate skeleton tag ref='{}' tag={}",
                clip_ref, tag
            ));
        }
        let dense = tag as usize;
        let present = dense < skeleton.joints.len() && skeleton.joints[dense].tag == tag
            || skeleton.joints.iter().any(|joint| joint.tag == tag);
        if !present {
            return Err(format!(
                "player animation skeleton tag is absent ref='{}' clip_index={} tag={} skeleton_joints={}",
                clip_ref,
                clip_index,
                tag,
                skeleton.joints.len()
            ));
        }
    }
    Ok(())
}

fn load_runtime_animation_clip(
    reference: &str,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    skeleton: &ModelSkeletonMetadata,
    animation_runtime: &AnimationSkeletonRuntime,
) -> Result<PlayerAnimationRuntimeClip, String> {
    let clip = load_animation_clip(reference)?;
    validate_animation_clip(reference, &clip, assignment, skeleton)?;
    let binding = clip.bind_to_skeleton(animation_runtime).map_err(|error| {
        format!("player animation runtime binding failed ref='{reference}' err='{error}'")
    })?;
    Ok(PlayerAnimationRuntimeClip {
        clip_ref: reference.to_owned(),
        clip,
        binding,
        event_cursor: AnimationEventCursor::default(),
    })
}

fn load_optional_presentation_clip(
    role: &str,
    reference: Option<&str>,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    skeleton: &ModelSkeletonMetadata,
    animation_runtime: &AnimationSkeletonRuntime,
) -> Option<PlayerAnimationRuntimeClip> {
    let reference = reference?;
    match load_runtime_animation_clip(reference, assignment, skeleton, animation_runtime) {
        Ok(clip) => Some(clip),
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "game-ready: optional player presentation animation unavailable role='{}' ref='{}' err='{}' action='keep visual model and locomotion binding'",
                role,
                reference,
                error
            );
            None
        }
    }
}

fn resolve_authored_equipment_arm_ik(
    skeleton: &ModelSkeletonMetadata,
    presentation: &newengine_engine_runtime::gameplay::PlayerCharacterPresentation,
) -> Option<WeaponArmIkRig> {
    if !presentation.equipment_arm_ik {
        return None;
    }
    let Some(authored) = presentation.equipment_arm_ik_rig.as_ref() else {
        newengine_ulog_api::ulog::warn!(
            "game-ready: player presentation degraded capability='equipment_arm_ik' reason='authored rig unavailable' action='disable arm IK and keep authored animation'"
        );
        return None;
    };
    match build_weapon_arm_ik_rig(skeleton, authored) {
        Ok(binding) => Some(binding),
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "game-ready: player presentation degraded capability='equipment_arm_ik' err='{}' action='disable arm IK and keep authored animation'",
                error
            );
            None
        }
    }
}

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

    let Some(idle_ref) = assignment.idle_animation.as_deref() else {
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
        (L::Walk, assignment.walk_animation.as_deref()),
        (L::Run, assignment.run_animation.as_deref()),
        (L::Sprint, assignment.sprint_animation.as_deref()),
        (L::CrouchIdle, assignment.crouch_idle_animation.as_deref()),
        (L::CrouchWalk, assignment.crouch_walk_animation.as_deref()),
        (L::Jump, assignment.jump_animation.as_deref()),
        (L::Fall, assignment.fall_animation.as_deref()),
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
        .map_err(|error| format!("GameReady locomotion graph compile failed: {error}"))?;
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
            format!("GameReady locomotion graph initial evaluation failed: {error}")
        })?;
    let helper_pose_copies = match resolve_helper_pose_copy_rules(
        skeleton,
        &assignment.presentation.helper_pose_copies,
    ) {
        Ok(rules) => rules,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "game-ready: player presentation degraded capability='helper_pose_copies' err='{}' action='keep locomotion animation'",
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
                    "game-ready: player presentation degraded capability='detached_head_follow' err='{}' action='keep locomotion animation'",
                    error
                );
                None
            }
        }
    } else {
        newengine_ulog_api::ulog::warn!(
            "game-ready: player presentation degraded capability='detached_head_follow' reason='authored follow rule unavailable' action='keep locomotion animation'"
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
                    "game-ready: player presentation degraded capability='eye_parent_follow' err='{}' action='keep locomotion animation'",
                    error
                );
                None
            }
        }
    } else {
        newengine_ulog_api::ulog::warn!(
            "game-ready: player presentation degraded capability='eye_parent_follow' reason='authored eye contract unavailable' action='keep locomotion animation'"
        );
        None
    };
    let bind_locals = animation_runtime.bind_locals().to_vec();
    let bind_joint_frames = animation_runtime.bind_joint_frames().to_vec();
    let braid_secondary_motion = match prepare_native_braid_secondary_motion(
        parts,
        skeleton,
        assignment.presentation.braid_secondary_motion.as_ref(),
        source_to_model,
        &bind_joint_frames,
    ) {
        Ok(binding) => binding,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "game-ready: player presentation degraded capability='braid_secondary_motion' err='{}' action='keep locomotion animation'",
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
    // Presentation overlays are feature-level assets, not avatar-existence invariants.
    // A missing rifle/unarmed clip may degrade the corresponding stance, but must never
    // tear down a successfully decoded/skinned playable character.
    let equipment_ready_pose = load_optional_presentation_clip(
        "equipment_ready",
        assignment.presentation.equipment_ready_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let equipment_aim_pose = load_optional_presentation_clip(
        "equipment_aim",
        assignment.presentation.equipment_aim_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let equipment_reload_pose = load_optional_presentation_clip(
        "equipment_reload",
        assignment
            .presentation
            .equipment_reload_animation
            .as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let unarmed_ready_pose = load_optional_presentation_clip(
        "unarmed_ready",
        assignment.presentation.unarmed_ready_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let unarmed_attack_pose = load_optional_presentation_clip(
        "unarmed_attack",
        assignment.presentation.unarmed_attack_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let turn_45_left_pose = load_optional_presentation_clip(
        "turn_45_left",
        assignment.presentation.turn_45_left_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let turn_45_right_pose = load_optional_presentation_clip(
        "turn_45_right",
        assignment.presentation.turn_45_right_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let turn_90_left_pose = load_optional_presentation_clip(
        "turn_90_left",
        assignment.presentation.turn_90_left_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let turn_90_right_pose = load_optional_presentation_clip(
        "turn_90_right",
        assignment.presentation.turn_90_right_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let turn_135_left_pose = load_optional_presentation_clip(
        "turn_135_left",
        assignment.presentation.turn_135_left_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let turn_135_right_pose = load_optional_presentation_clip(
        "turn_135_right",
        assignment.presentation.turn_135_right_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let turn_180_left_pose = load_optional_presentation_clip(
        "turn_180_left",
        assignment.presentation.turn_180_left_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let turn_180_right_pose = load_optional_presentation_clip(
        "turn_180_right",
        assignment.presentation.turn_180_right_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    // NoClip is a full-body traversal mode, not a degradable upper-body presentation overlay.
    // If a character authors a seated-flight clip, accepting a load/binding failure and falling
    // back to locomotion would make the character visibly walk/run while collisionless. Treat the
    // authored clip as required so the character either binds the intended seated pose or reports
    // an explicit model-binding error instead of silently presenting the wrong state.
    let noclip_pose = match assignment.presentation.noclip_animation.as_deref() {
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
    let fall_low_pose = load_optional_presentation_clip(
        "fall_low",
        assignment.presentation.fall_low_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let fall_medium_pose = load_optional_presentation_clip(
        "fall_medium",
        assignment.presentation.fall_medium_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let fall_high_pose = load_optional_presentation_clip(
        "fall_high",
        assignment.presentation.fall_high_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let landing_soft_pose = load_optional_presentation_clip(
        "landing_soft",
        assignment.presentation.landing_soft_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let landing_medium_pose = load_optional_presentation_clip(
        "landing_medium",
        assignment.presentation.landing_medium_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let landing_hard_pose = load_optional_presentation_clip(
        "landing_hard",
        assignment.presentation.landing_hard_animation.as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
    let landing_hard_run_pose = load_optional_presentation_clip(
        "landing_hard_run",
        assignment
            .presentation
            .landing_hard_run_animation
            .as_deref(),
        assignment,
        skeleton,
        &animation_runtime,
    );
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
            "game-ready: player presentation degraded capability='equipment_ready_sample_phase' reason='non-finite authored value' action='use phase 0'"
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
                "game-ready: player presentation degraded capability='equipment_ready_rotation_weights' err='{}' action='disable overlay weights'",
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
                "game-ready: player presentation degraded capability='equipment_aim_rotation_weights' err='{}' action='disable overlay weights'",
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
                "game-ready: player presentation degraded capability='equipment_reload_rotation_weights' err='{}' action='disable overlay weights'",
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
    let body_turn_layers = resolve_body_turn_layers(skeleton);
    let sampled_target_locals = current_locals.clone();
    let pose_continuity = PoseContinuityBridge::new(&current_locals);
    if !helper_pose_copies.is_empty() {
        newengine_ulog_api::ulog::info!(
            "game-ready: authored joint-copy rig channels={} policy='definition rules -> generic local-pose copy before skin palette'",
            helper_pose_copies.len()
        );
    }
    if let Some(rig) = head_follow.as_ref() {
        newengine_ulog_api::ulog::info!(
            "game-ready: authored palette-follow contract driver={} followers={} reserved={} policy='definition-selected driver deformation -> authored follower branches'",
            rig.driver_joint,
            rig.followers.len(),
            0usize,
        );
    }
    if let Some(eyes) = eye_contract.as_ref() {
        newengine_ulog_api::ulog::info!(
            "game-ready: authored eye-parent contract left={} right={} parent={} policy='locomotion keeps authored eye-local bind; eye palette follows authored parent deformation'",
            eyes.left,
            eyes.right,
            eyes.parent,
        );
    }
    if !body_turn_layers.is_empty() {
        newengine_ulog_api::ulog::info!(
            "game-ready: turn-in-place torso chain resolved layers={} policy='view residual yaw -> spine hierarchy; hips/legs stay root-owned'",
            body_turn_layers.len(),
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
            "game-ready: native turn-in-place prepared clips={} policy=authored-step-bounded-fixed-step-yaw-no-snap-rebase",
            native_turn_clip_count,
        );
    }
    if equipment_ready_pose.is_some()
        || equipment_aim_pose.is_some()
        || equipment_reload_pose.is_some()
        || equipment_ik.is_some()
    {
        newengine_ulog_api::ulog::info!(
            "game-ready: character equipment presentation prepared ready='{}' aim='{}' reload='{}' arm_ik={} ready_weights={} aim_weights={} reload_weights={} policy='firearm stance owns upper-body overlay; hand IK runs after locomotion blend'",
            assignment
                .presentation
                .equipment_ready_animation
                .as_deref()
                .unwrap_or("none"),
            assignment
                .presentation
                .equipment_aim_animation
                .as_deref()
                .unwrap_or("none"),
            assignment
                .presentation
                .equipment_reload_animation
                .as_deref()
                .unwrap_or("none"),
            equipment_ik.is_some(),
            equipment_ready_rotation_weights.len(),
            equipment_aim_rotation_weights.len(),
            equipment_reload_rotation_weights.len(),
        );
    }

    Ok(Some(PlayerAnimationRuntimeBinding {
        clips,
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
        body_turn_layers,
        body_turn_yaw_radians: 0.0,
        braid_secondary_motion,
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
        landing_last_revision: 0,
        fall_medium_min_distance,
        fall_high_min_distance,
        fall_active_band: None,
        fall_time_seconds: 0.0,
        equipment_ready_pose,
        equipment_aim_pose,
        equipment_reload_pose,
        unarmed_ready_pose,
        unarmed_attack_pose,
        unarmed_attack_sequence: 0,
        unarmed_attack_time_seconds: 0.0,
        equipment_ready_sample_phase,
        equipment_time_seconds: 0.0,
        equipment_reload_active: false,
        equipment_ready_rotation_weights,
        equipment_aim_rotation_weights,
        equipment_reload_rotation_weights,
        equipment_overlay_locals: bind_locals,
        equipment_ik,
        equipment_resolved_weapon_root: None,
    }))
}
