use newengine_assets_api::AssetServiceClient;
use newengine_ecs::{EntityId, World};
use newengine_math::Mat4;
use newengine_render_api::{
    HairCollisionMode, HairGroomAssetV1, HairGroomRegistryV1, HairInstanceDescV1, HairQualityTier,
    HairSceneV1, HairShaderSetV1, HairSimulationMode, HairSkinPoseRegistryV1, HairSkinPoseV1,
    HairTransparencyMode,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(crate) struct PreparedPlayerHairV1 {
    pub(crate) groom: HairGroomAssetV1,
    pub(crate) instance: HairInstanceDescV1,
    pub(crate) shaders: HairShaderSetV1,
    pub(crate) source_mesh_prefixes: Vec<String>,
    pub(crate) hide_in_first_person: bool,
}

#[derive(Clone, Copy, Debug)]
struct PlayerHairBindingStateV1 {
    instance_id: u64,
    authored_quality: HairQualityTier,
    hide_in_first_person: bool,
}

#[derive(Default)]
struct PlayerHairBindingRegistryV1 {
    bindings: BTreeMap<u64, PlayerHairBindingStateV1>,
}

#[inline]
fn hair_value<'a>(player: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    let flat = format!("hair_{key}");
    player
        .get(&flat)
        .or_else(|| player.get("hair").and_then(|hair| hair.get(key)))
}

fn hair_string(player: &serde_json::Value, key: &str) -> Option<String> {
    hair_value(player, key).and_then(newengine_definitions_runtime::metadata_value_string)
}

fn hair_f32(player: &serde_json::Value, key: &str) -> Option<f32> {
    hair_value(player, key).and_then(newengine_definitions_runtime::metadata_value_f32)
}

fn hair_bool(player: &serde_json::Value, key: &str) -> Option<bool> {
    hair_value(player, key).and_then(newengine_definitions_runtime::metadata_value_bool)
}

fn hair_u8(player: &serde_json::Value, key: &str) -> Option<u8> {
    let raw = hair_value(player, key)?;
    raw.as_u64()
        .and_then(|value| u8::try_from(value).ok())
        .or_else(|| {
            raw.as_str()
                .and_then(|value| value.trim().parse::<u8>().ok())
        })
}

fn hair_vec3(player: &serde_json::Value, key: &str) -> Option<[f32; 3]> {
    let value = hair_value(player, key)?;
    if let Some(values) = value.as_array() {
        if values.len() == 3 {
            let mut out = [0.0_f32; 3];
            for (index, value) in values.iter().enumerate() {
                out[index] = newengine_definitions_runtime::metadata_value_f32(value)?;
            }
            return Some(out);
        }
    }
    let raw = value.as_str()?;
    let values = raw
        .split([',', ';'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (values.len() == 3 && values.iter().all(|value| value.is_finite()))
        .then(|| [values[0], values[1], values[2]])
}

fn hair_string_list(player: &serde_json::Value, key: &str) -> Vec<String> {
    let Some(value) = hair_value(player, key) else {
        return Vec::new();
    };
    if let Some(array) = value.as_array() {
        return array
            .iter()
            .filter_map(newengine_definitions_runtime::metadata_value_string)
            .collect();
    }
    value
        .as_str()
        .map(|raw| {
            raw.split(';')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.replace('\\', "/"))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_quality(value: Option<String>) -> Result<HairQualityTier, String> {
    match value.as_deref().map(str::to_ascii_lowercase).as_deref() {
        None | Some("medium") => Ok(HairQualityTier::Medium),
        Some("off") => Ok(HairQualityTier::Off),
        Some("low") => Ok(HairQualityTier::Low),
        Some("high") => Ok(HairQualityTier::High),
        Some("ultra") => Ok(HairQualityTier::Ultra),
        Some(other) => Err(format!("unsupported authored hair quality '{other}'")),
    }
}

fn parse_simulation_mode(value: Option<String>) -> Result<HairSimulationMode, String> {
    match value.as_deref().map(str::to_ascii_lowercase).as_deref() {
        None | Some("guide_strands") | Some("guides") => Ok(HairSimulationMode::GuideStrands),
        Some("disabled") | Some("off") => Ok(HairSimulationMode::Disabled),
        Some(other) => Err(format!(
            "unsupported authored hair simulation mode '{other}'"
        )),
    }
}

fn parse_collision_mode(value: Option<String>) -> Result<HairCollisionMode, String> {
    match value.as_deref().map(str::to_ascii_lowercase).as_deref() {
        None | Some("capsules") => Ok(HairCollisionMode::Capsules),
        Some("none") | Some("off") => Ok(HairCollisionMode::None),
        Some("sdf") => Ok(HairCollisionMode::Sdf),
        Some(other) => Err(format!(
            "unsupported authored hair collision mode '{other}'"
        )),
    }
}

fn parse_transparency(value: Option<String>) -> Result<HairTransparencyMode, String> {
    match value.as_deref().map(str::to_ascii_lowercase).as_deref() {
        None | Some("alpha_to_coverage") | Some("a2c") => Ok(HairTransparencyMode::AlphaToCoverage),
        Some("alpha_blend") | Some("blend") => Ok(HairTransparencyMode::AlphaBlend),
        Some(other) => Err(format!(
            "unsupported authored hair transparency mode '{other}'"
        )),
    }
}

fn validate_groom_against_skeleton(
    groom: &HairGroomAssetV1,
    skeleton: &newengine_model_skeleton_api::ModelSkeletonMetadata,
) -> Result<(), String> {
    for strand in &groom.guide_strands {
        if usize::from(strand.root_joint_index) >= skeleton.joints.len() {
            return Err(format!(
                "NEHAIR groom '{}' root_joint_index={} exceeds skeleton joint count {}",
                groom.groom.as_str(),
                strand.root_joint_index,
                skeleton.joints.len()
            ));
        }
    }
    for capsule in &groom.collision_capsules {
        if usize::from(capsule.joint_index) >= skeleton.joints.len() {
            return Err(format!(
                "NEHAIR groom '{}' capsule joint_index={} exceeds skeleton joint count {}",
                groom.groom.as_str(),
                capsule.joint_index,
                skeleton.joints.len()
            ));
        }
    }
    Ok(())
}
