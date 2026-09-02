fn apply_player_model_identity_and_rig(
    profile: &mut AuthoredWorldProfile,
    model: &serde_json::Value,
    definition_ref: &str,
) -> usize {
    let mut applied = 0usize;
    let source = value_path(model, &["source"])
        .or_else(|| value_path(model, &["model"]))
        .and_then(value_string)
        .or_else(|| value_string(model));
    if let Some(source) = source {
        profile.player.model.source = source;
        profile.player.model.enabled = true;
        applied += 1;
    }
    if let Some(properties_ref) = value_path(model, &["properties_ref"])
        .or_else(|| value_path(model, &["descriptor_ref"]))
        .or_else(|| value_path(model, &["ytyp_ref"]))
        .and_then(value_string)
    {
        profile.player.model.properties_ref = Some(properties_ref);
        applied += 1;
    }
    if applied > 0 && profile.player.model.properties_ref.is_none() {
        let normalized_ref = definition_ref.trim().replace('\\', "/");
        if !normalized_ref.is_empty() {
            profile.player.model.properties_ref = Some(normalized_ref);
            applied += 1;
        }
    }
    if let Some(texture_dictionary) = value_path(model, &["texture_dictionary"])
        .or_else(|| value_path(model, &["textures"]))
        .and_then(value_string)
    {
        profile.player.model.texture_dictionary = Some(texture_dictionary);
        applied += 1;
    }
    if let Some(skeleton) = value_path(model, &["skeleton"])
        .or_else(|| value_path(model, &["metadata"]))
        .or_else(|| value_path(model, &["skeleton_ref"]))
        .and_then(value_string)
    {
        profile.player.model.skeleton = Some(skeleton);
        applied += 1;
    }
    let skin_sidecar_model = value_path(model, &["skin_sidecar_model"]).and_then(value_string);
    let skin_sidecar_skeleton =
        value_path(model, &["skin_sidecar_skeleton"]).and_then(value_string);
    let skin_sidecar_joint_suffix =
        value_path(model, &["skin_sidecar_joint_suffix"]).and_then(value_string);
    let skin_sidecar_local_joint_prefix =
        value_path(model, &["skin_sidecar_local_joint_prefix"]).and_then(value_string);
    let skin_sidecar_authored = [
        skin_sidecar_model.is_some(),
        skin_sidecar_skeleton.is_some(),
        skin_sidecar_joint_suffix.is_some(),
        skin_sidecar_local_joint_prefix.is_some(),
    ];
    if skin_sidecar_authored.iter().all(|present| *present) {
        profile.player.model.skin_sidecar = Some(
            newengine_engine_runtime::gameplay::PlayerSkinSidecarDefinition {
                model: skin_sidecar_model.expect("checked sidecar model"),
                skeleton: skin_sidecar_skeleton.expect("checked sidecar skeleton"),
                joint_name_suffix: skin_sidecar_joint_suffix.expect("checked sidecar suffix"),
                local_joint_prefix: skin_sidecar_local_joint_prefix
                    .expect("checked sidecar local prefix"),
            },
        );
        applied += 1;
    } else if skin_sidecar_authored.iter().any(|present| *present) {
        newengine_ulog_api::ulog::warn!(
            "fps-content: incomplete authored player skin sidecar definition definition_ref='{}' model={} skeleton={} suffix={} local_prefix={} action='reject_sidecar_contract'",
            definition_ref,
            skin_sidecar_authored[0],
            skin_sidecar_authored[1],
            skin_sidecar_authored[2],
            skin_sidecar_authored[3],
        );
    }
    if let Some(enabled) = value_path(model, &["detached_head_follow"]).and_then(value_bool) {
        profile.player.model.detached_head_follow = enabled;
        applied += 1;
    }
    let detached_driver =
        value_path(model, &["detached_head_follow_driver"]).and_then(value_string);
    let detached_roots = value_path(model, &["detached_head_follow_roots"])
        .map(authored_joint_list)
        .unwrap_or_default();
    if let Some(driver_joint) = detached_driver.filter(|_| !detached_roots.is_empty()) {
        profile.player.model.detached_head_follow_rule = Some(
            newengine_engine_runtime::gameplay::PlayerPaletteFollowRule {
                driver_joint,
                follower_roots: detached_roots,
                include_descendants: value_path(model, &["detached_head_follow_descendants"])
                    .and_then(value_bool)
                    .unwrap_or(true),
            },
        );
        applied += 1;
    }

    if let Some(enabled) = value_path(model, &["eye_parent_follow"]).and_then(value_bool) {
        profile.player.model.eye_parent_follow = enabled;
        applied += 1;
    }
    let eye_left = value_path(model, &["eye_left_joint"]).and_then(value_string);
    let eye_right = value_path(model, &["eye_right_joint"]).and_then(value_string);
    let eye_parent = value_path(model, &["eye_parent_joint"]).and_then(value_string);
    if let (Some(left_joint), Some(right_joint), Some(parent_joint)) =
        (eye_left, eye_right, eye_parent)
    {
        profile.player.model.eye_parent_follow_rule = Some(
            newengine_engine_runtime::gameplay::PlayerEyeParentFollowRule {
                left_joint,
                right_joint,
                parent_joint,
                preserve_bind_local: value_path(model, &["eye_preserve_bind_local"])
                    .and_then(value_bool)
                    .unwrap_or(true),
            },
        );
        applied += 1;
    }

    if let Some(rules) =
        value_path(model, &["helper_pose_copies"]).and_then(player_joint_copy_rules)
    {
        profile.player.model.helper_pose_copies = rules;
        applied += 1;
    }

    match parse_skeletal_secondary_motion(model) {
        Ok(Some(rig)) => {
            profile.player.model.skeletal_secondary_motion = Some(rig);
            applied += 1;
        }
        Ok(None) => {}
        Err(error) => newengine_ulog_api::ulog::warn!(
            "game-ready ytyp metadata: invalid project-authored skeletal_secondary_motion definition_ref='{}' err='{}'",
            definition_ref,
            error
        ),
    }

    let braid_chain_joints = value_path(model, &["braid_secondary_motion_chain_joints"])
        .and_then(value_string)
        .map(|raw| {
            raw.split(';')
                .map(str::trim)
                .filter(|joint| !joint.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        });
    let braid_collision_joints = [
        "braid_secondary_motion_head_joint",
        "braid_secondary_motion_head_base_joint",
        "braid_secondary_motion_upper_back_joint",
        "braid_secondary_motion_middle_back_joint",
        "braid_secondary_motion_lower_back_joint",
        "braid_secondary_motion_left_shoulder_joint",
        "braid_secondary_motion_right_shoulder_joint",
    ]
    .map(|key| value_path(model, &[key]).and_then(value_string));
    if let (
        Some(chain_joints),
        [Some(head_joint), Some(head_base_joint), Some(upper_back_joint), Some(middle_back_joint), Some(lower_back_joint), Some(left_shoulder_joint), Some(right_shoulder_joint)],
    ) = (braid_chain_joints, braid_collision_joints)
    {
        profile.player.model.braid_secondary_motion = Some(
            newengine_engine_runtime::gameplay::PlayerBraidSecondaryMotionRig {
                chain_joints,
                head_joint,
                head_base_joint,
                upper_back_joint,
                middle_back_joint,
                lower_back_joint,
                left_shoulder_joint,
                right_shoulder_joint,
            },
        );
        applied += 1;
    }

    if let Some(bindings) =
        value_path(model, &["animation_event_bindings"]).and_then(animation_event_bindings)
    {
        profile.player.model.animation_event_bindings = bindings;
        applied += 1;
    }

    applied
}
