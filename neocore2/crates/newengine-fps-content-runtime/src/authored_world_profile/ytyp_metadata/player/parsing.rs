#![forbid(unsafe_op_in_unsafe_fn)]

use super::super::{value_bool, value_f32, value_string};

/// Maps an authored character attribute such as `equipment.knife.ready`'s XML projection
/// (`equipment_knife_ready_animation`) into the opaque runtime semantic slot. Weapon families
/// are deliberately open-ended: the engine never enumerates knife/rifle/pistol/etc.
pub fn equipment_animation_slot_from_attribute(attribute: &str) -> Option<String> {
    let normalized = attribute.trim().to_ascii_lowercase();
    let body = normalized
        .strip_prefix("equipment_")?
        .strip_suffix("_animation")?;

    // Match the universal presentation grammar from the right so opaque family ids may contain `_`.
    const SUFFIXES: &[(&str, &str)] = &[
        ("transition_ready_to_aim", "transition.ready_to_aim"),
        ("transition_aim_to_ready", "transition.aim_to_ready"),
        ("grip_stand_ref", "grip.stand.ref"),
        ("grip_stand_arms", "grip.stand.arms"),
        ("grip_stand_hands", "grip.stand.hands"),
        ("grip_stand_fingers", "grip.stand.fingers"),
        ("grip_stand_add", "grip.stand.add"),
        ("grip_crouch_ref", "grip.crouch.ref"),
        ("grip_crouch_arms", "grip.crouch.arms"),
        ("grip_crouch_hands", "grip.crouch.hands"),
        ("grip_crouch_fingers", "grip.crouch.fingers"),
        ("grip_crouch_add", "grip.crouch.add"),
        ("grip_prone_ref", "grip.prone.ref"),
        ("grip_prone_arms", "grip.prone.arms"),
        ("grip_prone_hands", "grip.prone.hands"),
        ("grip_prone_fingers", "grip.prone.fingers"),
        ("grip_prone_add", "grip.prone.add"),
        ("crouch_aim_blocked_add", "crouch.aim.blocked.add"),
        ("crouch_aim_blocked_sub", "crouch.aim.blocked.sub"),
        ("prone_aim_blocked_add", "prone.aim.blocked.add"),
        ("prone_aim_blocked_sub", "prone.aim.blocked.sub"),
        ("aim_blocked_add", "aim.blocked.add"),
        ("aim_blocked_sub", "aim.blocked.sub"),
        ("crouch_aim_move_b135r", "crouch.aim.move.b135r"),
        ("crouch_aim_move_b135l", "crouch.aim.move.b135l"),
        ("crouch_aim_move_fw45r", "crouch.aim.move.fw45r"),
        ("crouch_aim_move_fw45l", "crouch.aim.move.fw45l"),
        ("crouch_aim_move_b180", "crouch.aim.move.b180"),
        ("crouch_aim_move_r90", "crouch.aim.move.r90"),
        ("crouch_aim_move_l90", "crouch.aim.move.l90"),
        ("crouch_aim_move_fw", "crouch.aim.move.fw"),
        ("prone_aim_move_b135r", "prone.aim.move.b135r"),
        ("prone_aim_move_b135l", "prone.aim.move.b135l"),
        ("prone_aim_move_fw45r", "prone.aim.move.fw45r"),
        ("prone_aim_move_fw45l", "prone.aim.move.fw45l"),
        ("prone_aim_move_b180", "prone.aim.move.b180"),
        ("prone_aim_move_r90", "prone.aim.move.r90"),
        ("prone_aim_move_l90", "prone.aim.move.l90"),
        ("prone_aim_move_fw", "prone.aim.move.fw"),
        ("aim_move_b135r", "aim.move.b135r"),
        ("aim_move_b135l", "aim.move.b135l"),
        ("aim_move_fw45r", "aim.move.fw45r"),
        ("aim_move_fw45l", "aim.move.fw45l"),
        ("aim_move_b180", "aim.move.b180"),
        ("aim_move_r90", "aim.move.r90"),
        ("aim_move_l90", "aim.move.l90"),
        ("aim_move_fw", "aim.move.fw"),
        ("crouch_aim_idle", "crouch.aim.idle"),
        ("prone_aim_idle", "prone.aim.idle"),
        ("aim_idle", "aim.idle"),
        ("ready", "ready"),
        ("reload", "reload"),
        ("aim", "aim"),
    ];

    for (suffix, semantic) in SUFFIXES {
        let marker = format!("_{suffix}");
        let Some(family) = body.strip_suffix(&marker) else {
            continue;
        };
        let family = family.replace("-", "_");
        if family.is_empty()
            || !family
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            return None;
        }
        return Some(format!("equipment.{family}.{semantic}"));
    }
    None
}

/// Maps `equipment_<family>_ready_sample_phase` to the same open-ended normalized family id used
/// by `weapon.class`. This lets a character freeze an authored transition at its READY endpoint
/// without changing other equipment families or teaching runtime any weapon names.
pub fn equipment_ready_sample_phase_family_from_attribute(attribute: &str) -> Option<String> {
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
        .replace(['+', '|'], ",")
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

pub(super) fn player_joint_copy_rules(
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

pub(super) fn authored_joint_list(value: &serde_json::Value) -> Vec<String> {
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

pub(super) fn animation_event_bindings(
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

pub fn player_joint_rotation_weights(
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
