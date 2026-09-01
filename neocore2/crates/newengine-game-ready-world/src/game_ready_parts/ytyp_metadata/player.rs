use super::*;

/// Maps an authored character attribute such as `equipment.knife.ready`'s XML projection
/// (`equipment_knife_ready_animation`) into the opaque runtime semantic slot. Weapon families
/// are deliberately open-ended: the engine never enumerates knife/rifle/pistol/etc.
pub(super) fn equipment_animation_slot_from_attribute(attribute: &str) -> Option<String> {
    let normalized = attribute.trim().to_ascii_lowercase();
    let body = normalized
        .strip_prefix("equipment_")?
        .strip_suffix("_animation")?;
    let (family, stance) = body.rsplit_once('_')?;
    if family.is_empty() || !matches!(stance, "ready" | "aim" | "reload") {
        return None;
    }
    let family = family.replace('-', "_");
    family
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        .then(|| format!("equipment.{family}.{stance}"))
}

/// Maps `equipment_<family>_ready_sample_phase` to the same open-ended normalized family id used
/// by `weapon.class`. This lets a character freeze an authored transition at its READY endpoint
/// without changing other equipment families or teaching runtime any weapon names.
pub(super) fn equipment_ready_sample_phase_family_from_attribute(
    attribute: &str,
) -> Option<String> {
    let normalized = attribute.trim().to_ascii_lowercase();
    let family = normalized
        .strip_prefix("equipment_")?
        .strip_suffix("_ready_sample_phase")?
        .replace('-', "_");
    (!family.is_empty()
        && family
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'))
    .then_some(family)
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

fn animation_event_bindings(
    value: &serde_json::Value,
) -> Option<std::collections::BTreeMap<String, String>> {
    let mut result = std::collections::BTreeMap::new();
    if let Some(object) = value.as_object() {
        for (event, target) in object {
            let Some(target) = value_string(target) else {
                continue;
            };
            let event = event.trim();
            let target = target.trim();
            if !event.is_empty() && !target.is_empty() {
                result.insert(event.to_owned(), target.to_owned());
            }
        }
    } else if let Some(raw) = value.as_str() {
        for entry in raw
            .split(';')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            let Some((event, target)) = entry.split_once('>') else {
                continue;
            };
            let event = event.trim();
            let target = target.trim();
            if !event.is_empty() && !target.is_empty() {
                result.insert(event.to_owned(), target.to_owned());
            }
        }
    }
    (!result.is_empty()).then_some(result)
}

pub(super) fn player_joint_rotation_weights(
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
            "game-ready: incomplete authored player skin sidecar definition definition_ref='{}' model={} skeleton={} suffix={} local_prefix={} action='reject_sidecar_contract'",
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
    // Character-owned equipment pose families are open-ended. The selected item's authored
    // `weapon.class` chooses one of these slots at runtime; this parser must therefore discover
    // families from metadata instead of encoding a knife/rifle/pistol enum here.
    if let Some(attributes) = model.as_object() {
        for (attribute, value) in attributes {
            if let Some(family) = equipment_ready_sample_phase_family_from_attribute(attribute) {
                if let Some(phase) = value_f32(value) {
                    profile
                        .player
                        .model
                        .equipment_ready_sample_phases
                        .insert(family, phase.clamp(0.0, 1.0));
                    applied += 1;
                }
                continue;
            }
            let Some(semantic) = equipment_animation_slot_from_attribute(attribute) else {
                continue;
            };
            let Some(reference) = value_string(value) else {
                continue;
            };
            profile
                .player
                .model
                .animation_slots
                .insert(semantic, reference);
            applied += 1;
        }
    }

    for (attribute, semantic) in [
        ("look_relaxed_base_animation", "look.relaxed.base"),
        ("look_relaxed_range_animation", "look.relaxed.range"),
        ("look_crouch_base_animation", "look.crouch.base"),
        ("look_crouch_range_animation", "look.crouch.range"),
        ("look_tense_base_animation", "look.tense.base"),
        ("look_tense_range_animation", "look.tense.range"),
        ("look_eyes_base_animation", "look.eyes.base"),
        ("look_eyes_range_animation", "look.eyes.range"),
        (
            "look_context_cover_low_left_base_animation",
            "look.context.cover_low_left.base",
        ),
        (
            "look_context_cover_low_left_range_animation",
            "look.context.cover_low_left.range",
        ),
        (
            "look_context_cover_low_right_base_animation",
            "look.context.cover_low_right.base",
        ),
        (
            "look_context_cover_low_right_range_animation",
            "look.context.cover_low_right.range",
        ),
        (
            "look_context_prone_base_animation",
            "look.context.prone.base",
        ),
        (
            "look_context_prone_range_animation",
            "look.context.prone.range",
        ),
        (
            "look_context_supine_base_animation",
            "look.context.supine.base",
        ),
        (
            "look_context_supine_range_animation",
            "look.context.supine.range",
        ),
        ("look_context_rope_base_animation", "look.context.rope.base"),
        (
            "look_context_rope_range_animation",
            "look.context.rope.range",
        ),
        (
            "look_context_ladder_base_animation",
            "look.context.ladder.base",
        ),
        (
            "look_context_ladder_range_animation",
            "look.context.ladder.range",
        ),
        (
            "look_context_swim_idle_base_animation",
            "look.context.swim_idle.base",
        ),
        (
            "look_context_swim_idle_range_animation",
            "look.context.swim_idle.range",
        ),
        (
            "look_context_injured_base_animation",
            "look.context.injured.base",
        ),
        (
            "look_context_injured_range_animation",
            "look.context.injured.range",
        ),
        (
            "look_context_relaxed_injured_base_animation",
            "look.context.relaxed_injured.base",
        ),
        (
            "look_context_relaxed_injured_range_animation",
            "look.context.relaxed_injured.range",
        ),
    ] {
        if let Some(reference) = value_path(model, &[attribute]).and_then(value_string) {
            profile
                .player
                .model
                .animation_slots
                .insert(semantic.to_owned(), reference);
            applied += 1;
        }
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
    if let Some(reference) = value_path(model, &["turn_45_left_animation"]).and_then(value_string) {
        profile.player.model.turn_45_left_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_45_right_animation"]).and_then(value_string)
    {
        profile.player.model.turn_45_right_animation = Some(reference);
        applied += 1;
    }
    if let Some(reference) = value_path(model, &["turn_90_left_animation"]).and_then(value_string) {
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
    // Normalize legacy per-animation attributes into the project-owned slot table. Runtime
    // consumers resolve only these semantic slots; filenames/clip names never escape authoring.
    for (slot, reference) in [
        (
            "locomotion.idle",
            profile.player.model.idle_animation.clone(),
        ),
        (
            "locomotion.walk",
            profile.player.model.walk_animation.clone(),
        ),
        ("locomotion.run", profile.player.model.run_animation.clone()),
        (
            "locomotion.sprint",
            profile.player.model.sprint_animation.clone(),
        ),
        (
            "locomotion.crouch_idle",
            profile.player.model.crouch_idle_animation.clone(),
        ),
        (
            "locomotion.crouch_walk",
            profile.player.model.crouch_walk_animation.clone(),
        ),
        (
            "locomotion.jump",
            profile.player.model.jump_animation.clone(),
        ),
        (
            "locomotion.fall",
            profile.player.model.fall_animation.clone(),
        ),
        ("fall.low", profile.player.model.fall_low_animation.clone()),
        (
            "fall.medium",
            profile.player.model.fall_medium_animation.clone(),
        ),
        (
            "fall.high",
            profile.player.model.fall_high_animation.clone(),
        ),
        (
            "landing.soft",
            profile.player.model.landing_soft_animation.clone(),
        ),
        (
            "landing.medium",
            profile.player.model.landing_medium_animation.clone(),
        ),
        (
            "landing.hard",
            profile.player.model.landing_hard_animation.clone(),
        ),
        (
            "landing.hard_run",
            profile.player.model.landing_hard_run_animation.clone(),
        ),
        (
            "equipment.ready",
            profile.player.model.equipment_ready_animation.clone(),
        ),
        (
            "equipment.aim",
            profile.player.model.equipment_aim_animation.clone(),
        ),
        (
            "equipment.reload",
            profile.player.model.equipment_reload_animation.clone(),
        ),
        (
            "unarmed.ready",
            profile.player.model.unarmed_ready_animation.clone(),
        ),
        (
            "unarmed.attack",
            profile.player.model.unarmed_attack_animation.clone(),
        ),
        (
            "turn.left.45",
            profile.player.model.turn_45_left_animation.clone(),
        ),
        (
            "turn.right.45",
            profile.player.model.turn_45_right_animation.clone(),
        ),
        (
            "turn.left.90",
            profile.player.model.turn_90_left_animation.clone(),
        ),
        (
            "turn.right.90",
            profile.player.model.turn_90_right_animation.clone(),
        ),
        (
            "turn.left.135",
            profile.player.model.turn_135_left_animation.clone(),
        ),
        (
            "turn.right.135",
            profile.player.model.turn_135_right_animation.clone(),
        ),
        (
            "turn.left.180",
            profile.player.model.turn_180_left_animation.clone(),
        ),
        (
            "turn.right.180",
            profile.player.model.turn_180_right_animation.clone(),
        ),
    ] {
        if let Some(reference) = reference {
            profile
                .player
                .model
                .animation_slots
                .insert(slot.to_owned(), reference);
        }
    }
    if let Some(reference) = value_path(model, &["noclip_animation"]).and_then(value_string) {
        profile
            .player
            .model
            .animation_slots
            .insert("traversal.noclip".to_owned(), reference);
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
