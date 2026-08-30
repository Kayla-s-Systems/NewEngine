use super::*;
use newengine_game_data::{GameData, PlayerMotionResponseData};

/// GameReady metadata hydration from `.ytyp` Definition Entries.
///
/// `.ymap` is the launch/profile document. Runtime constants that belong
/// to authored game content are allowed to override the profile only when they
/// come from `engine.assets.definitions` as `.ytyp@entry` metadata. This keeps the
/// chain visible:
///
/// `.ytyp -> .ydd -> .nemat -> .ytd`
///
/// and prevents local ad-hoc material JSON from becoming a second source of
/// truth.
#[inline]
pub(super) fn value_path<'a>(
    mut value: &'a serde_json::Value,
    path: &[&str],
) -> Option<&'a serde_json::Value> {
    for key in path {
        value = value.get(*key)?;
    }
    Some(value)
}

#[inline]
pub(super) fn value_string(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.replace('\\', "/"))
}

#[inline]
pub(super) fn value_f32(value: &serde_json::Value) -> Option<f32> {
    value
        .as_f64()
        .map(|v| v as f32)
        .or_else(|| value.as_str().and_then(|v| v.trim().parse::<f32>().ok()))
        .filter(|v| v.is_finite())
}

#[inline]
pub(super) fn value_bool(value: &serde_json::Value) -> Option<bool> {
    value.as_bool().or_else(|| {
        let raw = value.as_str()?.trim().to_ascii_lowercase();
        match raw.as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    })
}

fn player_joint_channels_text(
    raw: &str,
) -> Option<newengine_engine_runtime::gameplay::PlayerJointChannels> {
    let mut channels = newengine_engine_runtime::gameplay::PlayerJointChannels {
        translation: false,
        rotation: false,
        scale: false,
    };
    for token in raw
        .trim()
        .to_ascii_lowercase()
        .replace('+', ",")
        .replace('|', ",")
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        match token {
            "t" | "translation" => channels.translation = true,
            "r" | "rotation" => channels.rotation = true,
            "s" | "scale" => channels.scale = true,
            "tr" | "rt" => {
                channels.translation = true;
                channels.rotation = true;
            }
            "trs" | "all" => {
                channels = newengine_engine_runtime::gameplay::PlayerJointChannels::all()
            }
            _ => return None,
        }
    }
    channels.any().then_some(channels)
}

fn player_joint_channels(
    value: Option<&serde_json::Value>,
) -> newengine_engine_runtime::gameplay::PlayerJointChannels {
    let Some(value) = value else {
        return newengine_engine_runtime::gameplay::PlayerJointChannels::rotation_only();
    };
    if let Some(raw) = value.as_str() {
        return player_joint_channels_text(raw).unwrap_or_else(
            newengine_engine_runtime::gameplay::PlayerJointChannels::rotation_only,
        );
    }
    if let Some(object) = value.as_object() {
        let channels = newengine_engine_runtime::gameplay::PlayerJointChannels {
            translation: object
                .get("translation")
                .and_then(value_bool)
                .unwrap_or(false),
            rotation: object.get("rotation").and_then(value_bool).unwrap_or(false),
            scale: object.get("scale").and_then(value_bool).unwrap_or(false),
        };
        if channels.any() {
            return channels;
        }
    }
    newengine_engine_runtime::gameplay::PlayerJointChannels::rotation_only()
}

fn player_joint_copy_rules(
    value: &serde_json::Value,
) -> Option<Vec<newengine_engine_runtime::gameplay::PlayerJointCopyRule>> {
    let mut result = Vec::new();
    if let Some(array) = value.as_array() {
        for entry in array {
            let Some(source_joint) = entry.get("source_joint").and_then(value_string) else {
                continue;
            };
            let Some(target_joint) = entry.get("target_joint").and_then(value_string) else {
                continue;
            };
            let channels = player_joint_channels(entry.get("channels"));
            if !source_joint.eq_ignore_ascii_case(&target_joint) && channels.any() {
                result.push(newengine_engine_runtime::gameplay::PlayerJointCopyRule {
                    source_joint,
                    target_joint,
                    channels,
                });
            }
        }
    } else if let Some(raw) = value.as_str() {
        for entry in raw
            .split(';')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            let (mapping, channels) = entry.rsplit_once(':').unwrap_or((entry, "trs"));
            let Some((source_joint, target_joint)) = mapping.split_once('>') else {
                continue;
            };
            let Some(channels) = player_joint_channels_text(channels) else {
                continue;
            };
            let source_joint = source_joint.trim();
            let target_joint = target_joint.trim();
            if !source_joint.is_empty()
                && !target_joint.is_empty()
                && !source_joint.eq_ignore_ascii_case(target_joint)
            {
                result.push(newengine_engine_runtime::gameplay::PlayerJointCopyRule {
                    source_joint: source_joint.to_owned(),
                    target_joint: target_joint.to_owned(),
                    channels,
                });
            }
        }
    }
    (!result.is_empty()).then_some(result)
}

fn authored_joint_list(value: &serde_json::Value) -> Vec<String> {
    if let Some(array) = value.as_array() {
        return array.iter().filter_map(value_string).collect();
    }
    value
        .as_str()
        .map(|raw| {
            raw.split(';')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn player_joint_rotation_weights(
    value: &serde_json::Value,
) -> Option<Vec<newengine_engine_runtime::gameplay::PlayerJointRotationWeight>> {
    fn item(
        joint: &str,
        weight: f32,
        channels: newengine_engine_runtime::gameplay::PlayerJointChannels,
    ) -> Option<newengine_engine_runtime::gameplay::PlayerJointRotationWeight> {
        let joint = joint.trim();
        if joint.is_empty() || !weight.is_finite() {
            return None;
        }
        Some(
            newengine_engine_runtime::gameplay::PlayerJointRotationWeight {
                joint: joint.to_owned(),
                weight: weight.clamp(0.0, 1.0),
                channels,
            },
        )
    }

    let mut result = Vec::new();
    if let Some(array) = value.as_array() {
        for entry in array {
            let Some(joint) = entry.get("joint").and_then(|value| value.as_str()) else {
                continue;
            };
            let Some(weight) = entry.get("weight").and_then(value_f32) else {
                continue;
            };
            let channels = player_joint_channels(entry.get("channels"));
            if let Some(weight) = item(joint, weight, channels) {
                result.push(weight);
            }
        }
    } else if let Some(raw) = value.as_str() {
        for entry in raw.split(';') {
            let mut parts = entry.split(':');
            let Some(joint) = parts.next() else { continue };
            let Some(weight) = parts.next() else { continue };
            let Ok(weight) = weight.trim().parse::<f32>() else {
                continue;
            };
            let channels = parts
                .next()
                .and_then(player_joint_channels_text)
                .unwrap_or_else(
                    newengine_engine_runtime::gameplay::PlayerJointChannels::rotation_only,
                );
            if let Some(weight) = item(joint, weight, channels) {
                result.push(weight);
            }
        }
    }
    (!result.is_empty()).then_some(result)
}

#[inline]
pub(super) fn is_nemat_ref(value: &str) -> bool {
    let normalized = value.trim().replace('\\', "/");
    newengine_assets::require_asset_reference_extension(&normalized, &["nemat"], true).is_ok()
}

#[inline]
pub(super) fn is_ytd_ref(value: &str) -> bool {
    let normalized = value.trim().replace('\\', "/");
    newengine_assets::require_asset_reference_extension(&normalized, &["ytd"], true).is_ok()
}

#[inline]
pub(super) fn material_spec_mut<'a>(
    profile: &'a mut GameReadyMapProfile,
    key: &str,
) -> Option<&'a mut GameReadyMaterialSpec> {
    match key {
        "terrain" => Some(&mut profile.materials.terrain),
        "sky" | "skydome" => Some(&mut profile.materials.sky),
        "sun" | "sun_disk" => Some(&mut profile.materials.sun),
        "moon" | "moon_disk" => Some(&mut profile.materials.moon),
        "tree_bark" | "bark" => Some(&mut profile.materials.tree_bark),
        "tree_leaf" | "leaf" => Some(&mut profile.materials.tree_leaf),
        "tree_branch" | "branch" => Some(&mut profile.materials.tree_branch),
        _ => None,
    }
}

pub(super) fn apply_material_refs_from_ytyp(
    profile: &mut GameReadyMapProfile,
    metadata: &serde_json::Value,
    definition_ref: &str,
) -> usize {
    let Some(materials) = metadata.get("materials").and_then(|v| v.as_object()) else {
        return 0;
    };
    let mut applied = 0usize;
    for (key, value) in materials {
        let Some(reference) = value_string(value) else {
            continue;
        };
        if !is_nemat_ref(&reference) {
            newengine_ulog_api::ulog::warn!(
                "game-ready ytyp metadata: rejected material key='{}' ref='{}' definition_ref='{}' reason='expected .nemat@entry'",
                key,
                reference,
                definition_ref
            );
            continue;
        }
        if let Some(spec) = material_spec_mut(profile, key) {
            spec.asset = Some(reference.clone());
            applied += 1;
            newengine_ulog_api::ulog::debug!(
                "game-ready ytyp metadata: material key='{}' asset='{}' source='{}'",
                key,
                reference,
                definition_ref
            );
        }
    }
    applied
}

pub(super) fn apply_ytd_constant(
    metadata: &serde_json::Value,
    path: &[&str],
    label: &str,
    definition_ref: &str,
) -> Option<String> {
    let reference = value_path(metadata, path).and_then(value_string)?;
    if !is_ytd_ref(&reference) {
        newengine_ulog_api::ulog::warn!(
            "game-ready ytyp metadata: rejected texture label='{}' ref='{}' definition_ref='{}' reason='expected .ytd@entry'",
            label,
            reference,
            definition_ref
        );
        return None;
    }
    Some(reference)
}

pub(super) fn apply_texture_refs_from_ytyp(
    profile: &mut GameReadyMapProfile,
    metadata: &serde_json::Value,
    definition_ref: &str,
) -> usize {
    let mut applied = 0usize;
    if let Some(reference) = apply_ytd_constant(
        metadata,
        &["terrain", "surface", "forest_base_texture"],
        "terrain.surface.forest_base_texture",
        definition_ref,
    ) {
        profile.terrain.surface.forest_base_texture = reference;
        applied += 1;
    }
    if let Some(reference) = apply_ytd_constant(
        metadata,
        &["terrain", "surface", "sand_base_texture"],
        "terrain.surface.sand_base_texture",
        definition_ref,
    ) {
        profile.terrain.surface.sand_base_texture = reference;
        applied += 1;
    }
    if let Some(reference) = apply_ytd_constant(
        metadata,
        &["terrain", "surface", "rock_base_texture"],
        "terrain.surface.rock_base_texture",
        definition_ref,
    ) {
        profile.terrain.surface.rock_base_texture = reference;
        applied += 1;
    }
    if let Some(reference) = apply_ytd_constant(
        metadata,
        &["sky", "moon_texture"],
        "sky.moon_texture",
        definition_ref,
    ) {
        profile.sky.moon_texture = reference;
        applied += 1;
    }
    applied
}

pub(super) fn apply_player_model_from_ytyp(
    profile: &mut GameReadyMapProfile,
    metadata: &serde_json::Value,
    definition_ref: &str,
) -> usize {
    let player_node = value_path(metadata, &["player"]);
    let Some(model) = player_node
        .and_then(|player| player.get("model"))
        .filter(|model| model.is_object())
        .or_else(|| {
            player_node.filter(|player| {
                player.get("source").is_some()
                    || player.get("texture_dictionary").is_some()
                    || player.get("metadata").is_some()
                    || player
                        .get("model")
                        .and_then(|value| value.as_str())
                        .is_some()
            })
        })
        .or_else(|| value_path(metadata, &["model"]))
        .or_else(|| player_node.and_then(|player| player.get("model")))
    else {
        return 0;
    };
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

    if let Some(reference) = value_path(model, &["idle_animation"]).and_then(value_string) {
        profile.player.model.idle_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["walk_animation"]).and_then(value_string) {
        profile.player.model.walk_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["run_animation"]).and_then(value_string) {
        profile.player.model.run_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["sprint_animation"]).and_then(value_string) {
        profile.player.model.sprint_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["crouch_idle_animation"]).and_then(value_string) {
        profile.player.model.crouch_idle_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["crouch_walk_animation"]).and_then(value_string) {
        profile.player.model.crouch_walk_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["jump_animation"]).and_then(value_string) {
        profile.player.model.jump_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["fall_animation"]).and_then(value_string) {
        profile.player.model.fall_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["fall_low_animation"]).and_then(value_string) {
        profile.player.model.fall_low_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["fall_medium_animation"]).and_then(value_string) {
        profile.player.model.fall_medium_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["fall_high_animation"]).and_then(value_string) {
        profile.player.model.fall_high_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["landing_soft_animation"]).and_then(value_string) {
        profile.player.model.landing_soft_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["landing_medium_animation"]).and_then(value_string)
    {
        profile.player.model.landing_medium_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["landing_hard_animation"]).and_then(value_string) {
        profile.player.model.landing_hard_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) =
        value_path(model, &["landing_hard_run_animation"]).and_then(value_string)
    {
        profile.player.model.landing_hard_run_animation = Some(reference);
        applied += 1;
    }
    if let Some(value) = value_path(model, &["fall_medium_min_distance"]).and_then(value_f32) {
        profile.player.model.fall_medium_min_distance = value.max(0.0);
        applied += 1;
    }
    if let Some(value) = value_path(model, &["fall_high_min_distance"]).and_then(value_f32) {
        profile.player.model.fall_high_min_distance =
            value.max(profile.player.model.fall_medium_min_distance);
        applied += 1;
    }
    if let Some(reference) =
        value_path(model, &["equipment_ready_animation"]).and_then(value_string)
    {
        profile.player.model.equipment_ready_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["equipment_aim_animation"]).and_then(value_string)
    {
        profile.player.model.equipment_aim_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) =
        value_path(model, &["equipment_reload_animation"]).and_then(value_string)
    {
        profile.player.model.equipment_reload_animation = Some(reference);
        applied += 1;
    }
    if let Some(phase) = value_path(model, &["equipment_ready_sample_phase"]).and_then(value_f32) {
        profile.player.model.equipment_ready_sample_phase = phase.clamp(0.0, 1.0);
        applied += 1;
    }
    if let Some(weights) = value_path(model, &["equipment_ready_rotation_weights"])
        .and_then(player_joint_rotation_weights)
    {
        profile.player.model.equipment_ready_rotation_weights = weights;
        applied += 1;
    }
    if let Some(weights) = value_path(model, &["equipment_aim_rotation_weights"])
        .and_then(player_joint_rotation_weights)
    {
        profile.player.model.equipment_aim_rotation_weights = weights;
        applied += 1;
    }
    if let Some(weights) = value_path(model, &["equipment_reload_rotation_weights"])
        .and_then(player_joint_rotation_weights)
    {
        profile.player.model.equipment_reload_rotation_weights = weights;
        applied += 1;
    }
    if let Some(enabled) = value_path(model, &["equipment_arm_ik"]).and_then(value_bool) {
        profile.player.model.equipment_arm_ik = enabled;
        applied += 1;
    }
    let ik_required = [
        "equipment_arm_ik_chest",
        "equipment_arm_ik_right_shoulder",
        "equipment_arm_ik_right_elbow",
        "equipment_arm_ik_right_wrist",
        "equipment_arm_ik_right_palm",
        "equipment_arm_ik_left_shoulder",
        "equipment_arm_ik_left_elbow",
        "equipment_arm_ik_left_wrist",
        "equipment_arm_ik_left_palm",
    ]
    .map(|key| value_path(model, &[key]).and_then(value_string));
    if let [Some(chest), Some(right_shoulder), Some(right_elbow), Some(right_wrist), Some(right_palm), Some(left_shoulder), Some(left_elbow), Some(left_wrist), Some(left_palm)] =
        ik_required
    {
        profile.player.model.equipment_arm_ik_rig = Some(
            newengine_engine_runtime::gameplay::PlayerWeaponArmIkRigDefinition {
                chest,
                right_shoulder,
                right_elbow,
                right_wrist,
                right_palm,
                right_prop_attachment: value_path(
                    model,
                    &["equipment_arm_ik_right_prop_attachment"],
                )
                .and_then(value_string),
                left_shoulder,
                left_elbow,
                left_wrist,
                left_palm,
                left_prop_attachment: value_path(model, &["equipment_arm_ik_left_prop_attachment"])
                    .and_then(value_string),
            },
        );
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["unarmed_ready_animation"]).and_then(value_string)
    {
        profile.player.model.unarmed_ready_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["unarmed_attack_animation"]).and_then(value_string)
    {
        profile.player.model.unarmed_attack_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_45_left_animation"]).and_then(value_string)
    {
        profile.player.model.turn_45_left_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_45_right_animation"]).and_then(value_string)
    {
        profile.player.model.turn_45_right_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_90_left_animation"]).and_then(value_string)
    {
        profile.player.model.turn_90_left_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_90_right_animation"]).and_then(value_string)
    {
        profile.player.model.turn_90_right_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_135_left_animation"]).and_then(value_string)
    {
        profile.player.model.turn_135_left_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_135_right_animation"]).and_then(value_string)
    {
        profile.player.model.turn_135_right_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_180_left_animation"]).and_then(value_string)
    {
        profile.player.model.turn_180_left_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_180_right_animation"]).and_then(value_string)
    {
        profile.player.model.turn_180_right_animation = Some(reference);
        applied += 1;
    }
    let player_values = player_node.unwrap_or(model);
    if let Some(value) = value_path(player_values, &["walk_speed"]).and_then(value_f32) {
        profile.player.walk_speed = value.clamp(0.05, 50.0);
        applied += 1;
    }
    if let Some(value) = value_path(player_values, &["run_speed"]).and_then(value_f32) {
        profile.player.run_speed = value.clamp(0.05, 50.0);
        profile.player.move_speed = profile.player.run_speed;
        applied += 1;
    }
    if let Some(value) = value_path(player_values, &["sprint_speed"]).and_then(value_f32) {
        profile.player.sprint_speed = value.clamp(0.05, 75.0);
        applied += 1;
    }
    if let Some(value) = value_path(player_values, &["crouch_speed"]).and_then(value_f32) {
        profile.player.crouch_speed = value.clamp(0.05, 50.0);
        applied += 1;
    }
    // Sanitize the set as one unit so an authored typo cannot invert movement modes.
    profile.player.walk_speed = profile.player.walk_speed.min(profile.player.run_speed);
    profile.player.sprint_speed = profile.player.sprint_speed.max(profile.player.run_speed);
    profile.player.crouch_speed = profile.player.crouch_speed.min(profile.player.run_speed);

    if let Some(visibility) = value_path(model, &["visibility"]).and_then(value_string) {
        let visibility = visibility.to_ascii_lowercase();
        profile.player.model.hide_in_first_person = visibility.contains("hide_in_first_person")
            || visibility.contains("first_person_hidden");
        applied += 1;
    }
    if let Some(target_height) = value_path(model, &["target_height"]).and_then(value_f32) {
        profile.player.model.target_height = target_height.clamp(0.25, 3.0);
        applied += 1;
    }
    if let Some(eye_height_ratio) = value_path(model, &["eye_height_ratio"]).and_then(value_f32) {
        profile.player.model.eye_height_ratio = eye_height_ratio.clamp(0.55, 0.98);
        applied += 1;
    }
    if let Some(yaw_offset) = value_path(model, &["yaw_offset"]).and_then(value_f32) {
        profile.player.model.yaw_offset = yaw_offset;
        applied += 1;
    }
    if applied > 0 {
        newengine_ulog_api::ulog::info!(
            "game-ready ytyp metadata: player model descriptor source='{}' properties_ref={:?} policy='.ytyp connects model source to material bindings'",
            profile.player.model.source,
            profile.player.model.properties_ref
        );
    }
    applied
}

fn player_motion_response_from_ytyp(
    player: &serde_json::Value,
) -> Option<PlayerMotionResponseData> {
    let response = value_path(player, &["motion_response"])?;
    if !response.is_object() {
        return None;
    }
    Some(PlayerMotionResponseData {
        velocity_spring_const: value_path(response, &["velocity_spring_const"])
            .and_then(value_f32)?,
        velocity_spring_const_decel: value_path(response, &["velocity_spring_const_decel"])
            .and_then(value_f32)?,
        velocity_spring_dampen_ratio: value_path(response, &["velocity_spring_dampen_ratio"])
            .and_then(value_f32)?,
        speed_spring_const: value_path(response, &["speed_spring_const"]).and_then(value_f32)?,
        max_accel: value_path(response, &["max_accel"]).and_then(value_f32)?,
        trans_clamp_dist: value_path(response, &["trans_clamp_dist"]).and_then(value_f32)?,
    })
}

fn apply_player_runtime_data_from_ytyp(
    profile: &mut GameReadyMapProfile,
    data: &mut GameData,
    metadata: &serde_json::Value,
) -> usize {
    let Some(player) = value_path(metadata, &["player"]) else {
        return 0;
    };
    if !player.is_object() {
        return 0;
    }

    // The Shared character definition is authoritative for character-owned model and locomotion
    // data. Project GameData remains authoritative for spawn/look and world physics.
    data.player.model.enabled = profile.player.model.enabled;
    data.player.model.source = profile.player.model.source.clone();
    data.player.model.target_height = profile.player.model.target_height;
    data.player.model.eye_height_ratio = profile.player.model.eye_height_ratio;
    data.player.model.local_offset = [
        profile.player.model.local_offset.x,
        profile.player.model.local_offset.y,
        profile.player.model.local_offset.z,
    ];
    data.player.model.yaw_offset = profile.player.model.yaw_offset;
    data.player.model.hide_in_first_person = profile.player.model.hide_in_first_person;
    data.player.move_speed = profile.player.run_speed;

    let tuning = &mut data.player.tuning;
    let mut applied = 1usize;
    if value_path(player, &["motion_response"]).is_some() {
        if let Some(response) = player_motion_response_from_ytyp(player) {
            tuning.motion_response = Some(response);
            applied += 6;
            newengine_ulog_api::ulog::info!(
                "game-ready ytyp player motion_response: velocity_k={:.3} decel_k={:.3} dampen={:.3} speed_k={:.3} max_accel={:.3} trans_clamp_dist={:.4} policy='authored spring/K payload; max_accel sentinel semantics unresolved'",
                response.velocity_spring_const,
                response.velocity_spring_const_decel,
                response.velocity_spring_dampen_ratio,
                response.speed_spring_const,
                response.max_accel,
                response.trans_clamp_dist,
            );
        } else {
            newengine_ulog_api::ulog::warn!(
                "game-ready ytyp player motion_response ignored: block must provide all six finite authored fields"
            );
        }
    }
    macro_rules! apply_tuning {
        ($key:literal, $field:ident, $min:expr, $max:expr) => {
            if let Some(value) = value_path(player, &[$key]).and_then(value_f32) {
                tuning.$field = value.clamp($min, $max);
                applied += 1;
            }
        };
    }

    apply_tuning!("body_radius", body_radius, 0.15, 1.0);
    apply_tuning!("body_half_height", body_half_height, 0.15, 1.5);
    apply_tuning!(
        "crouched_body_half_height",
        crouched_body_half_height,
        0.05,
        1.5
    );
    apply_tuning!("visual_radius", visual_radius, 0.15, 1.0);
    apply_tuning!("visual_half_height", visual_half_height, 0.15, 1.5);
    apply_tuning!("camera_eye_height", camera_eye_height, 0.05, 2.5);
    apply_tuning!(
        "crouched_camera_eye_height",
        crouched_camera_eye_height,
        0.05,
        2.5
    );
    apply_tuning!("crouch_camera_speed", crouch_camera_speed, 0.1, 100.0);
    apply_tuning!("jump_speed", jump_speed, 0.0, 30.0);
    apply_tuning!("ground_probe_distance", ground_probe_distance, 0.01, 2.0);
    apply_tuning!("max_slope_degrees", max_slope_degrees, 0.0, 89.0);
    apply_tuning!("footstep_stride", footstep_stride, 0.25, 10.0);
    apply_tuning!(
        "landing_speed_threshold",
        landing_speed_threshold,
        0.0,
        100.0
    );
    apply_tuning!(
        "locomotion_min_horizontal_speed",
        locomotion_min_horizontal_speed,
        0.0,
        20.0
    );
    apply_tuning!(
        "ground_probe_max_upward_velocity",
        ground_probe_max_upward_velocity,
        -20.0,
        20.0
    );
    apply_tuning!(
        "landing_min_airborne_seconds",
        landing_min_airborne_seconds,
        0.0,
        5.0
    );

    if let Some(value) = value_path(player, &["sprint_multiplier"]).and_then(value_f32) {
        tuning.sprint_multiplier = value.clamp(1.0, 8.0);
        applied += 1;
    } else if profile.player.run_speed > 0.0 {
        tuning.sprint_multiplier =
            (profile.player.sprint_speed / profile.player.run_speed).clamp(1.0, 8.0);
    }

    profile.gameplay.player_collision.radius = tuning.body_radius;
    profile.gameplay.player_collision.half_height = tuning.body_half_height;
    profile.gameplay.player_visual.radius = tuning.visual_radius;
    profile.gameplay.player_visual.half_height = tuning.visual_half_height;
    profile.gameplay.player_visual.camera_eye_height = tuning.camera_eye_height;
    profile.gameplay.player_visual.sprint_multiplier = tuning.sprint_multiplier;
    applied
}

pub(super) fn apply_gameplay_constants_from_ytyp(
    profile: &mut GameReadyMapProfile,
    metadata: &serde_json::Value,
) -> usize {
    let mut applied = 0usize;
    if let Some(radius) =
        value_path(metadata, &["gameplay", "player_collision", "radius"]).and_then(value_f32)
    {
        profile.gameplay.player_collision.radius = radius.clamp(0.15, 1.0);
        applied += 1;
    }
    if let Some(half_height) =
        value_path(metadata, &["gameplay", "player_collision", "half_height"]).and_then(value_f32)
    {
        profile.gameplay.player_collision.half_height = half_height.clamp(0.15, 1.5);
        applied += 1;
    }
    if let Some(radius) =
        value_path(metadata, &["gameplay", "player_visual", "radius"]).and_then(value_f32)
    {
        profile.gameplay.player_visual.radius = radius.clamp(0.15, 1.0);
        applied += 1;
    }
    if let Some(half_height) =
        value_path(metadata, &["gameplay", "player_visual", "half_height"]).and_then(value_f32)
    {
        profile.gameplay.player_visual.half_height = half_height.clamp(0.15, 1.5);
        applied += 1;
    }
    if let Some(camera_eye_height) = value_path(
        metadata,
        &["gameplay", "player_visual", "camera_eye_height"],
    )
    .and_then(value_f32)
    {
        profile.gameplay.player_visual.camera_eye_height = camera_eye_height.clamp(0.05, 2.5);
        applied += 1;
    }
    if let Some(sprint_multiplier) = value_path(
        metadata,
        &["gameplay", "player_visual", "sprint_multiplier"],
    )
    .and_then(value_f32)
    {
        profile.gameplay.player_visual.sprint_multiplier = sprint_multiplier.clamp(1.0, 4.0);
        applied += 1;
    }
    applied
}

pub(super) fn apply_sky_constants_from_ytyp(
    profile: &mut GameReadyMapProfile,
    metadata: &serde_json::Value,
) -> usize {
    let mut applied = 0usize;
    if let Some(radius) = value_path(metadata, &["sky", "radius"]).and_then(value_f32) {
        profile.sky.radius = radius.max(16.0);
        applied += 1;
    }
    if let Some(sun_radius) = value_path(metadata, &["sky", "sun_radius"]).and_then(value_f32) {
        profile.sky.sun_radius = sun_radius.clamp(1.0, 64.0);
        applied += 1;
    }
    if let Some(moon_radius) = value_path(metadata, &["sky", "moon_radius"]).and_then(value_f32) {
        profile.sky.moon_radius = moon_radius.clamp(1.0, 64.0);
        applied += 1;
    }
    if let Some(mesh) = value_path(metadata, &["sky", "mesh"]).and_then(value_string) {
        profile.sky.mesh = mesh;
        applied += 1;
    }
    if let Some(definition_ref) =
        value_path(metadata, &["sky", "definition_ref"]).and_then(value_string)
    {
        profile.sky.definition_ref = definition_ref;
        applied += 1;
    }
    if let Some(render_options) =
        value_path(metadata, &["sky", "render_options"]).and_then(|value| {
            serde_json::from_value::<newengine_model_domain_api::MeshRenderOptions>(value.clone())
                .ok()
        })
    {
        profile.sky.render_options = render_options;
        applied += 1;
    }
    if let Some(cloud_dictionary) =
        value_path(metadata, &["sky", "cloud_dictionary"]).and_then(value_string)
    {
        profile.sky.cloud_dictionary = cloud_dictionary;
        applied += 1;
    }
    if let Some(cloud_profile) =
        value_path(metadata, &["sky", "cloud_profile"]).and_then(value_string)
    {
        profile.sky.cloud_profile = cloud_profile;
        applied += 1;
    }
    applied
}

pub(super) fn apply_time_constants_from_ytyp(
    profile: &mut GameReadyMapProfile,
    metadata: &serde_json::Value,
) -> usize {
    let mut applied = 0usize;
    if let Some(hours) =
        value_path(metadata, &["lighting", "day_night", "time_of_day_hours"]).and_then(value_f32)
    {
        profile.lighting.day_night.time_of_day_hours = hours.rem_euclid(24.0);
        applied += 1;
    }
    if let Some(day_len) =
        value_path(metadata, &["lighting", "day_night", "day_length_seconds"]).and_then(value_f32)
    {
        profile.lighting.day_night.day_length_seconds = day_len.max(1.0);
        applied += 1;
    }
    if let Some(latitude) =
        value_path(metadata, &["lighting", "day_night", "latitude_degrees"]).and_then(value_f32)
    {
        profile.lighting.day_night.latitude_degrees = latitude.clamp(-89.0, 89.0);
        applied += 1;
    }
    if let Some(axial_tilt) =
        value_path(metadata, &["lighting", "day_night", "axial_tilt_degrees"]).and_then(value_f32)
    {
        profile.lighting.day_night.axial_tilt_degrees = axial_tilt.clamp(-45.0, 45.0);
        applied += 1;
    }
    applied
}

pub(super) fn load_game_ready_definition_entry(definition_ref: &str) -> Option<serde_json::Value> {
    let payload =
        serde_json::to_vec(&serde_json::json!({ "definition_ref": definition_ref })).ok()?;
    match call_service_v1_optional(
        newengine_assets::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        newengine_assets::definitions_method::ENTRY_JSON_V1,
        &payload,
    ) {
        Ok(Some(bytes)) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(value) => Some(value),
            Err(e) => {
                newengine_ulog_api::ulog::warn!("game-ready ytyp metadata: engine.assets.definitions returned invalid json ref='{}' err='{}'", definition_ref, e);
                None
            }
        },
        Ok(None) => {
            newengine_ulog_api::ulog::debug!("game-ready ytyp metadata: engine.assets.definitions route absent ref='{}'; metadata hydration skipped", definition_ref);
            None
        }
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "game-ready ytyp metadata: engine.assets.definitions call failed ref='{}' err='{}'",
                definition_ref,
                e
            );
            None
        }
    }
}

fn metadata_namespace<'a>(
    entry: &'a serde_json::Value,
    namespace: &str,
) -> Option<&'a serde_json::Value> {
    entry
        .get("arbitrary_metadata")
        .and_then(|v| v.get("metadata"))
        .and_then(|v| v.get(namespace))
        .or_else(|| {
            entry
                .get("arbitrary_metadata")
                .and_then(|v| v.get("namespaces"))
                .and_then(|v| v.get(namespace))
        })
        .or_else(|| {
            entry
                .get("arbitrary_metadata")
                .and_then(|v| v.get(namespace))
        })
        .or_else(|| entry.get("metadata").and_then(|v| v.get(namespace)))
        .or_else(|| entry.get("namespaces").and_then(|v| v.get(namespace)))
}

fn audio_metadata_namespace(entry: &serde_json::Value) -> Option<&serde_json::Value> {
    metadata_namespace(entry, "newengine.audio")
}

fn string_or_array(value: &serde_json::Value) -> Vec<String> {
    if let Some(value) = value.as_str() {
        let value = value.trim().to_ascii_lowercase();
        return (!value.is_empty()).then_some(value).into_iter().collect();
    }
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect()
}

fn acoustic_material_library_from_ytyp(
    metadata: &serde_json::Value,
) -> Option<newengine_audio_api::AcousticMaterialLibrary> {
    let library = metadata.get("acoustic_material_library")?;
    let raw_materials = library.get("material")?;
    let materials = raw_materials
        .as_array()
        .map(|values| values.iter().collect::<Vec<_>>())
        .unwrap_or_else(|| vec![raw_materials]);
    let mut rules = Vec::new();
    for material in materials {
        let Some(material_id) = material.get("material_id").and_then(value_string) else {
            continue;
        };
        let Some(transmission_gain) = material.get("transmission_gain").and_then(value_f32) else {
            continue;
        };
        let reflection_gain = material
            .get("reflection_gain")
            .and_then(value_f32)
            .unwrap_or_else(|| {
                newengine_audio_api::AcousticMaterialProfile::default().reflection_gain
            });
        let Some(high_frequency_absorption) = material
            .get("high_frequency_absorption")
            .and_then(value_f32)
        else {
            continue;
        };
        let Some(low_pass_hz) = material.get("low_pass_hz").and_then(value_f32) else {
            continue;
        };
        let surface_matches = material
            .get("match")
            .map(string_or_array)
            .unwrap_or_default();
        if surface_matches.is_empty() {
            continue;
        }
        rules.push(newengine_audio_api::AcousticMaterialRule {
            material_id,
            surface_matches,
            profile: newengine_audio_api::AcousticMaterialProfile {
                transmission_gain,
                reflection_gain,
                high_frequency_absorption,
                low_pass_hz,
            },
        });
    }
    (!rules.is_empty()).then(|| newengine_audio_api::AcousticMaterialLibrary::new(rules))
}

fn merge_acoustic_material_library(
    target: &mut newengine_audio_api::AcousticMaterialLibrary,
    incoming: newengine_audio_api::AcousticMaterialLibrary,
) {
    for incoming_rule in incoming.rules {
        let incoming_matches = incoming_rule.surface_matches.clone();
        for rule in &mut target.rules {
            rule.surface_matches
                .retain(|pattern| !incoming_matches.iter().any(|value| value == pattern));
        }
        target.rules.retain(|rule| !rule.surface_matches.is_empty());
        target.rules.push(incoming_rule);
    }
    *target = target.clone().sanitized();
}

pub(super) fn game_ready_metadata_namespace(
    entry: &serde_json::Value,
) -> Option<&serde_json::Value> {
    metadata_namespace(entry, "newengine.game_ready")
}

fn definition_render_options(
    entry: &serde_json::Value,
) -> Option<newengine_model_domain_api::MeshRenderOptions> {
    entry
        .get("model_explanation")
        .and_then(|value| value.get("render_options"))
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
}

fn apply_sky_drawable_from_ytyp(
    profile: &mut GameReadyMapProfile,
    entry: &serde_json::Value,
    definition_ref: &str,
) -> usize {
    let Some(options) = definition_render_options(entry) else {
        return 0;
    };
    if !matches!(
        options.role,
        newengine_model_domain_api::MeshRenderRole::SkyBackground
            | newengine_model_domain_api::MeshRenderRole::CelestialBillboard
    ) {
        return 0;
    }
    let drawable = entry
        .get("model_explanation")
        .and_then(|value| value.get("drawable_ref"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(drawable) = drawable else {
        return 0;
    };
    profile.sky.mesh = drawable.replace('\\', "/");
    newengine_ulog_api::ulog::info!(
        "game-ready ytyp sky drawable: definition_ref='{}' mesh='{}' source='model_explanation.drawable_ref' policy='YTYP dependency graph owns skydome asset identity'",
        definition_ref,
        profile.sky.mesh
    );
    1
}

fn apply_render_options_from_ytyp(
    profile: &mut GameReadyMapProfile,
    entry: &serde_json::Value,
    definition_ref: &str,
) -> usize {
    let Some(options) = definition_render_options(entry) else {
        return 0;
    };
    let target = match options.role {
        newengine_model_domain_api::MeshRenderRole::TerrainPatch => {
            profile.terrain.render_options = options.clone();
            "terrain"
        }
        newengine_model_domain_api::MeshRenderRole::FoliageInstanced => {
            profile.foliage.render_options = options.clone();
            "foliage"
        }
        newengine_model_domain_api::MeshRenderRole::SkyBackground
        | newengine_model_domain_api::MeshRenderRole::CelestialBillboard => {
            profile.sky.render_options = options.clone();
            "sky"
        }
        newengine_model_domain_api::MeshRenderRole::CharacterBody => {
            profile.player.model.render_options = options.clone();
            "player"
        }
        _ => {
            return 0;
        }
    };
    newengine_ulog_api::ulog::info!("game-ready ytyp render policy: target='{}' definition_ref='{}' role={:?} shadow_policy={:?} source='engine.assets.definitions/.ytyp'", target, definition_ref, options.role, options.shadow_policy);
    1
}

pub(crate) fn apply_game_ready_ytyp_metadata(
    profile: &mut GameReadyMapProfile,
    game_data: &mut GameData,
) {
    let definitions = profile.definitions.clone();
    if definitions.is_empty() {
        newengine_ulog_api::ulog::warn!(
            "game-ready ytyp metadata: scene profile declares no .ytyp Definition Entries; runtime will use sanitized profile defaults"
        );
        return;
    }

    let mut applied_total = 0usize;
    for spec in definitions {
        let definition_ref = spec.definition_ref.trim();
        let Some(entry) = load_game_ready_definition_entry(definition_ref) else {
            continue;
        };
        let null_metadata = serde_json::Value::Null;
        let metadata = game_ready_metadata_namespace(&entry).unwrap_or(&null_metadata);
        let mut audio_applied = 0usize;
        if let Some(audio_metadata) = audio_metadata_namespace(&entry) {
            if let Some(library) = acoustic_material_library_from_ytyp(audio_metadata) {
                let count = library.rules.len();
                merge_acoustic_material_library(&mut profile.acoustic_materials, library);
                audio_applied = count;
                newengine_ulog_api::ulog::info!(
                    "game-ready ytyp audio metadata: definition_ref='{}' acoustic_material_rules={} total_rules={} policy='Shared baseline first; later definitions replace matching surface rules'",
                    definition_ref,
                    count,
                    profile.acoustic_materials.rules.len(),
                );
            }
        }
        if metadata.is_null() {
            newengine_ulog_api::ulog::debug!(
                "game-ready ytyp metadata: definition_ref='{}' has no newengine.game_ready namespace; graph-only definition",
                definition_ref
            );
        }
        let applied = audio_applied
            + apply_render_options_from_ytyp(profile, &entry, definition_ref)
            + apply_sky_drawable_from_ytyp(profile, &entry, definition_ref)
            + apply_material_refs_from_ytyp(profile, metadata, definition_ref)
            + apply_texture_refs_from_ytyp(profile, metadata, definition_ref)
            + apply_player_model_from_ytyp(profile, metadata, definition_ref)
            + apply_player_runtime_data_from_ytyp(profile, game_data, metadata)
            + apply_gameplay_constants_from_ytyp(profile, metadata)
            + apply_sky_constants_from_ytyp(profile, metadata)
            + apply_time_constants_from_ytyp(profile, metadata);
        applied_total += applied;
        newengine_ulog_api::ulog::info!(
            "game-ready ytyp metadata: consumed definition_ref='{}' applied_constants={} policy='metadata constants from engine.assets.definitions; no runtime json material source'",
            definition_ref,
            applied
        );
    }

    newengine_ulog_api::ulog::info!(
        "game-ready ytyp metadata: completed definitions={} applied_constants={} chain='.ytyp -> .ytyd -> .ydd -> .nemat -> .ytd'",
        profile.definitions.len(),
        applied_total
    );
}

pub(crate) fn resolve_game_ready_asset_graph(
    root_ref: &str,
) -> Option<newengine_model_domain_api::ResolvedAssetGraphV2> {
    let payload = serde_json::to_vec(&serde_json::json!({ "root_ref": root_ref })).ok()?;
    match call_service_v1_optional(
        newengine_model_domain_api::ENGINE_ASSETS_GRAPH_SERVICE_ID,
        newengine_model_domain_api::ASSET_GRAPH_METHOD_RESOLVE_V1,
        &payload,
    ) {
        Ok(Some(bytes)) => {
            match serde_json::from_slice::<newengine_model_domain_api::ResolvedAssetGraphV2>(&bytes)
            {
                Ok(graph) => Some(graph),
                Err(e) => {
                    newengine_ulog_api::ulog::warn!(
                        "assets.graph.resolve_v1: invalid json graph root_ref='{}' err='{}'",
                        root_ref,
                        e
                    );
                    None
                }
            }
        }
        Ok(None) => {
            newengine_ulog_api::ulog::debug!(
                "assets.graph.resolve_v1: route absent root_ref='{}'; graph hydration skipped",
                root_ref
            );
            None
        }
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "assets.graph.resolve_v1: gateway call failed root_ref='{}' err='{}'",
                root_ref,
                e
            );
            None
        }
    }
}

#[cfg(test)]
mod player_presentation_metadata_tests {
    use super::*;

    #[test]
    fn complete_motion_response_block_is_typed_without_invented_fields() {
        let player = serde_json::json!({
            "motion_response": {
                "velocity_spring_const": 7.0,
                "velocity_spring_const_decel": 10.0,
                "velocity_spring_dampen_ratio": 1.0,
                "speed_spring_const": 4.6,
                "max_accel": -1.0,
                "trans_clamp_dist": 0.01
            }
        });
        let response = player_motion_response_from_ytyp(&player).expect("typed response");
        assert_eq!(response.velocity_spring_const, 7.0);
        assert_eq!(response.velocity_spring_const_decel, 10.0);
        assert_eq!(response.velocity_spring_dampen_ratio, 1.0);
        assert_eq!(response.speed_spring_const, 4.6);
        assert_eq!(response.max_accel, -1.0);
        assert_eq!(response.trans_clamp_dist, 0.01);
    }

    #[test]
    fn partial_motion_response_block_is_rejected_instead_of_filling_guesses() {
        let player = serde_json::json!({
            "motion_response": {
                "velocity_spring_const": 7.0,
                "velocity_spring_const_decel": 10.0
            }
        });
        assert!(player_motion_response_from_ytyp(&player).is_none());
    }

    #[test]
    fn compact_equipment_rotation_weights_parse_from_ytyp_attribute() {
        let value = serde_json::Value::String("spineb:0.22;r_shoulder:0.92;r_palm:1.0".to_owned());
        let weights = player_joint_rotation_weights(&value).expect("weights");
        assert_eq!(weights.len(), 3);
        assert_eq!(weights[0].joint, "spineb");
        assert!((weights[0].weight - 0.22).abs() < 1.0e-6);
        assert_eq!(weights[1].joint, "r_shoulder");
        assert!((weights[1].weight - 0.92).abs() < 1.0e-6);
        assert_eq!(weights[2].joint, "r_palm");
        assert!((weights[2].weight - 1.0).abs() < 1.0e-6);
    }
    #[test]
    fn acoustic_material_library_hydrates_from_definition_metadata_projection() {
        let metadata = serde_json::json!({
            "acoustic_material_library": {
                "schema": "newengine.audio.acoustic-material-library.v2",
                "version": 2,
                "material": [
                    {
                        "material_id": "material.test.a",
                        "transmission_gain": 0.25,
                        "reflection_gain": 0.72,
                        "high_frequency_absorption": 0.75,
                        "low_pass_hz": 2400.0,
                        "match": "solid_a"
                    },
                    {
                        "material_id": "material.test.b",
                        "transmission_gain": 0.55,
                        "high_frequency_absorption": 0.40,
                        "low_pass_hz": 6400.0,
                        "match": ["panel_b", "sheet_b"]
                    }
                ]
            }
        });
        let library = acoustic_material_library_from_ytyp(&metadata).expect("acoustic library");
        assert_eq!(library.rules.len(), 2);
        assert_eq!(
            library.resolve("surface.wall.solid_a").unwrap().material_id,
            "material.test.a"
        );
        assert_eq!(
            library.resolve("surface.sheet_b").unwrap().material_id,
            "material.test.b"
        );
        assert!(
            (library
                .resolve("surface.wall.solid_a")
                .unwrap()
                .profile
                .reflection_gain
                - 0.72)
                .abs()
                < 1.0e-6
        );
        assert!(
            (library
                .resolve("surface.sheet_b")
                .unwrap()
                .profile
                .reflection_gain
                - newengine_audio_api::AcousticMaterialProfile::default().reflection_gain)
                .abs()
                < 1.0e-6
        );
    }

    #[test]
    fn later_acoustic_library_replaces_matching_shared_rule_only() {
        let mut shared = newengine_audio_api::AcousticMaterialLibrary::new(vec![
            newengine_audio_api::AcousticMaterialRule {
                material_id: "material.shared.wall".to_owned(),
                surface_matches: vec!["wall".to_owned(), "masonry".to_owned()],
                profile: newengine_audio_api::AcousticMaterialProfile::default(),
            },
        ]);
        let project = newengine_audio_api::AcousticMaterialLibrary::new(vec![
            newengine_audio_api::AcousticMaterialRule {
                material_id: "material.project.wall".to_owned(),
                surface_matches: vec!["wall".to_owned()],
                profile: newengine_audio_api::AcousticMaterialProfile {
                    transmission_gain: 0.9,
                    reflection_gain: 0.2,
                    high_frequency_absorption: 0.1,
                    low_pass_hz: 12_000.0,
                },
            },
        ]);
        merge_acoustic_material_library(&mut shared, project);
        assert_eq!(
            shared.resolve("surface.wall").unwrap().material_id,
            "material.project.wall"
        );
        assert_eq!(
            shared.resolve("surface.masonry").unwrap().material_id,
            "material.shared.wall"
        );
    }
}
