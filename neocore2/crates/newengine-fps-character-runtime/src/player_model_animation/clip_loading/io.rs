fn load_animation_clip(reference: &str) -> Result<std::sync::Arc<AnimationClip>, String> {
    let parsed = AnimationClipReference::parse(reference)?;
    let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let descriptor = assets.resolve_file_type_v1(&parsed.logical_path)?;
    if !descriptor
        .semantic_gateway
        .eq_ignore_ascii_case("engine.animation")
    {
        return Err(format!(
            "player animation ref='{reference}' resolves to format module='{}' gateway='{}', expected engine.animation",
            descriptor.module_id, descriptor.semantic_gateway
        ));
    }
    global_animation_clip_store()
        .load_ycd_clip(reference, |logical_path| {
            assets
                .decode_v1(&AssetDecodeRequest {
                    logical_path: logical_path.to_owned(),
                    output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
                    selector: serde_json::Value::Null,
                                    format_descriptor: None,
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

fn load_authored_presentation_clip(
    role: &str,
    reference: Option<&str>,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    skeleton: &ModelSkeletonMetadata,
    animation_runtime: &AnimationSkeletonRuntime,
) -> Result<Option<PlayerAnimationRuntimeClip>, String> {
    let Some(reference) = reference else {
        return Ok(None);
    };
    load_runtime_animation_clip(reference, assignment, skeleton, animation_runtime)
        .map(Some)
        .map_err(|error| {
            format!(
                "authored player presentation animation unavailable role={} ref={} err={} policy=some_ref_is_binding_contract_no_idle_locomotion_bind_fallback",
                role, reference, error
            )
        })
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
            "fps-character: player presentation degraded capability='equipment_arm_ik' reason='authored rig unavailable' action='disable arm IK and keep authored animation'"
        );
        return None;
    };
    match build_weapon_arm_ik_rig(skeleton, authored) {
        Ok(binding) => Some(binding),
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "fps-character: player presentation degraded capability='equipment_arm_ik' err='{}' action='disable arm IK and keep authored animation'",
                error
            );
            None
        }
    }
}
