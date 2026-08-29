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
    clips[locomotion_slot(L::Idle)] =
        Some(load_runtime_animation_clip(
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
        .map_err(|error| format!("GameReady locomotion graph initial evaluation failed: {error}"))?;
    let helper_mirror_pairs = build_helper_mirror_pairs(skeleton);
    // Compatibility reconstruction is explicit project-authored presentation metadata. Runtime
    // behavior is never inferred from a character name, model path, or source franchise.
    let head_follow = assignment
        .presentation
        .detached_head_follow
        .then(|| build_detached_head_follow(skeleton))
        .flatten();
    let eye_contract = assignment
        .presentation
        .eye_parent_follow
        .then(|| build_eye_runtime_contract(skeleton))
        .flatten();
    let bind_locals = animation_runtime.bind_locals().to_vec();
    let bind_joint_frames = animation_runtime.bind_joint_frames().to_vec();
    let braid_secondary_motion = prepare_native_braid_secondary_motion(
        parts,
        skeleton,
        source_to_model,
        &bind_joint_frames,
    )?;
    let mut current_locals = locomotion_graph_evaluation.local_pose.clone();
    synchronize_helper_pose(&helper_mirror_pairs, &mut current_locals);
    stabilize_eye_locals(eye_contract.as_ref(), skeleton, &mut current_locals)?;
    let mut palette_scratch = Vec::with_capacity(skeleton.joints.len());
    animation_runtime
        .build_skin_palette_from_local_pose(&current_locals, &mut palette_scratch)?;
    apply_detached_head_follow_palette(head_follow.as_ref(), &mut palette_scratch)?;
    validate_eye_palette(eye_contract.as_ref(), &palette_scratch)?;
    debug_dump_eye_matrices(
        eye_contract.as_ref(),
        &bind_joint_frames,
        &current_locals,
        &palette_scratch,
        "initial",
    );
    let equipment_ready_pose = assignment
        .presentation
        .equipment_ready_animation
        .as_deref()
        .map(|reference| load_runtime_animation_clip(reference, assignment, skeleton, &animation_runtime))
        .transpose()?;
    let equipment_aim_pose = assignment
        .presentation
        .equipment_aim_animation
        .as_deref()
        .map(|reference| load_runtime_animation_clip(reference, assignment, skeleton, &animation_runtime))
        .transpose()?;
    let equipment_reload_pose = assignment
        .presentation
        .equipment_reload_animation
        .as_deref()
        .map(|reference| load_runtime_animation_clip(reference, assignment, skeleton, &animation_runtime))
        .transpose()?;
    let unarmed_ready_pose = assignment
        .presentation
        .unarmed_ready_animation
        .as_deref()
        .map(|reference| load_runtime_animation_clip(reference, assignment, skeleton, &animation_runtime))
        .transpose()?;
    let unarmed_attack_pose = assignment
        .presentation
        .unarmed_attack_animation
        .as_deref()
        .map(|reference| load_runtime_animation_clip(reference, assignment, skeleton, &animation_runtime))
        .transpose()?;
    let equipment_ready_sample_phase = assignment
        .presentation
        .equipment_ready_sample_phase
        .clamp(0.0, 1.0);
    let equipment_ready_rotation_weights = assignment
        .presentation
        .equipment_ready_rotation_weights
        .iter()
        .map(|item| (item.joint.clone(), item.weight.clamp(0.0, 1.0)))
        .collect::<Vec<_>>();
    let equipment_aim_rotation_weights = assignment
        .presentation
        .equipment_aim_rotation_weights
        .iter()
        .map(|item| (item.joint.clone(), item.weight.clamp(0.0, 1.0)))
        .collect::<Vec<_>>();
    let equipment_reload_rotation_weights = assignment
        .presentation
        .equipment_reload_rotation_weights
        .iter()
        .map(|item| (item.joint.clone(), item.weight.clamp(0.0, 1.0)))
        .collect::<Vec<_>>();
    let equipment_ik = assignment
        .presentation
        .equipment_arm_ik
        .then(|| build_weapon_arm_ik_rig(skeleton))
        .flatten();
    let joint_frames_scratch = Vec::with_capacity(skeleton.joints.len());
    let foot_joints = resolve_foot_joint_binding(skeleton);
    let sampled_target_locals = current_locals.clone();
    if !helper_mirror_pairs.is_empty() {
        newengine_ulog_api::ulog::info!(
            "game-ready: mirrored North Star helper rig channels={} policy='primary local pose -> *_helper deform branch before skin palette'",
            helper_mirror_pairs.len()
        );
    }
    if let Some(rig) = head_follow.as_ref() {
        newengine_ulog_api::ulog::info!(
            "game-ready: restored authored detached head-space headb_driver={} control_followers={} face_followers={} policy='primary deform hierarchy -> detached controls + face/eyes'",
            rig.headb_driver,
            rig.control_followers.len(),
            rig.face_followers.len(),
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
        braid_secondary_motion,
        helper_mirror_pairs,
        eye_contract,
        head_follow,
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
