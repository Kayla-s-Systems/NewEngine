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
    hair_value(player, key).and_then(crate::ytyp_metadata::value_string)
}

fn hair_f32(player: &serde_json::Value, key: &str) -> Option<f32> {
    hair_value(player, key).and_then(crate::ytyp_metadata::value_f32)
}

fn hair_bool(player: &serde_json::Value, key: &str) -> Option<bool> {
    hair_value(player, key).and_then(crate::ytyp_metadata::value_bool)
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
                out[index] = crate::ytyp_metadata::value_f32(value)?;
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
            .filter_map(crate::ytyp_metadata::value_string)
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

pub(crate) fn prepare_player_hair_from_assignment_v1(
    player: EntityId,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    skeleton: Option<&newengine_model_skeleton_api::ModelSkeletonMetadata>,
) -> Result<Option<PreparedPlayerHairV1>, String> {
    let Some(definition_ref) = assignment.properties_ref.as_deref() else {
        return Ok(None);
    };
    let Some(entry) = crate::ytyp_metadata::load_game_ready_definition_entry(definition_ref) else {
        return Ok(None);
    };
    let metadata = crate::ytyp_metadata::game_ready_metadata_namespace(&entry).unwrap_or(&entry);
    let Some(player_metadata) = metadata.get("player") else {
        return Ok(None);
    };
    let Some(groom_path) = hair_string(player_metadata, "groom") else {
        return Ok(None);
    };
    if hair_bool(player_metadata, "enabled") == Some(false) {
        return Ok(None);
    }
    let skeleton = skeleton.ok_or_else(|| {
        format!("player hair groom '{groom_path}' requires authored skeleton metadata")
    })?;
    let groom = load_nehair_groom_v1(&groom_path)?;
    validate_groom_against_skeleton(&groom, skeleton)?;

    let mut instance = HairInstanceDescV1 {
        instance_id: runtime_player_hair_instance_id(player, &groom_path),
        quality: parse_quality(hair_string(player_metadata, "quality"))?,
        casts_shadows: hair_bool(player_metadata, "casts_shadows").unwrap_or(true),
        receives_shadows: hair_bool(player_metadata, "receives_shadows").unwrap_or(true),
        ..HairInstanceDescV1::default()
    };
    instance.wind_velocity = hair_vec3(player_metadata, "wind_velocity").unwrap_or([0.0; 3]);
    instance.simulation.mode = parse_simulation_mode(hair_string(player_metadata, "simulation"))?;
    instance.simulation.collision =
        parse_collision_mode(hair_string(player_metadata, "collision"))?;
    if let Some(value) = hair_f32(player_metadata, "gravity_scale") {
        instance.simulation.gravity_scale = value;
    }
    if let Some(value) = hair_f32(player_metadata, "damping") {
        instance.simulation.damping = value;
    }
    if let Some(value) = hair_f32(player_metadata, "stretch_stiffness") {
        instance.simulation.stretch_stiffness = value;
    }
    if let Some(value) = hair_f32(player_metadata, "bend_stiffness") {
        instance.simulation.bend_stiffness = value;
    }
    if let Some(value) = hair_f32(player_metadata, "root_stiffness") {
        instance.simulation.root_stiffness = value;
    }
    if let Some(value) = hair_f32(player_metadata, "wind_response") {
        instance.simulation.wind_response = value;
    }
    if let Some(value) = hair_u8(player_metadata, "solver_iterations") {
        instance.simulation.solver_iterations = value;
    }
    if let Some(value) = hair_f32(player_metadata, "max_delta_seconds") {
        instance.simulation.max_delta_seconds = value;
    }

    if let Some(value) = hair_vec3(player_metadata, "base_color") {
        instance.material.base_color = value;
    }
    if let Some(value) = hair_f32(player_metadata, "roughness") {
        instance.material.roughness = value;
    }
    if let Some(value) = hair_f32(player_metadata, "secondary_specular") {
        instance.material.secondary_specular = value;
    }
    if let Some(value) = hair_f32(player_metadata, "melanin") {
        instance.material.melanin = value;
    }
    if let Some(value) = hair_f32(player_metadata, "redness") {
        instance.material.redness = value;
    }
    if let Some(value) = hair_f32(player_metadata, "opacity") {
        instance.material.opacity = value;
    }
    if let Some(value) = hair_f32(player_metadata, "strand_width_mm") {
        instance.material.strand_width_mm = value;
    }
    if let Some(value) = hair_f32(player_metadata, "tip_scale") {
        instance.material.tip_scale = value;
    }
    instance.material.transparency =
        parse_transparency(hair_string(player_metadata, "transparency"))?;

    if let Some(value) = hair_f32(player_metadata, "lod_density_start") {
        instance.lod.density_start_distance = value;
    }
    if let Some(value) = hair_f32(player_metadata, "lod_density_end") {
        instance.lod.density_end_distance = value;
    }
    if let Some(value) = hair_f32(player_metadata, "lod_minimum_density") {
        instance.lod.minimum_density = value;
    }
    if let Some(value) = hair_f32(player_metadata, "lod_simulation_distance") {
        instance.lod.simulation_distance = value;
    }
    instance = instance.normalized()?;

    let simulation_shader = hair_string(player_metadata, "simulation_shader")
        .ok_or_else(|| "player hair requires authored hair_simulation_shader".to_owned())?;
    let strands_vertex_shader = hair_string(player_metadata, "strands_vertex_shader")
        .ok_or_else(|| "player hair requires authored hair_strands_vertex_shader".to_owned())?;
    let strands_fragment_shader = hair_string(player_metadata, "strands_fragment_shader")
        .ok_or_else(|| "player hair requires authored hair_strands_fragment_shader".to_owned())?;
    let mut shaders = HairShaderSetV1::new(
        simulation_shader,
        strands_vertex_shader,
        strands_fragment_shader,
    );
    match (
        hair_string(player_metadata, "shadow_vertex_shader"),
        hair_string(player_metadata, "shadow_fragment_shader"),
    ) {
        (Some(vs), Some(fs)) => shaders = shaders.with_shadows(vs, fs),
        (None, None) => {}
        _ => return Err("player hair shadow shader pair must be authored atomically".to_owned()),
    }
    shaders = shaders.normalized()?;

    let source_mesh_prefixes = hair_string_list(player_metadata, "source_mesh_prefixes");
    Ok(Some(PreparedPlayerHairV1 {
        groom,
        instance,
        shaders,
        source_mesh_prefixes,
        hide_in_first_person: hair_bool(player_metadata, "hide_in_first_person")
            .unwrap_or(assignment.hide_in_first_person),
    }))
}

#[inline]
fn runtime_player_hair_instance_id(player: EntityId, groom_path: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in player
        .stable_u64()
        .to_le_bytes()
        .into_iter()
        .chain(groom_path.as_bytes().iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash.max(1)
}

pub(crate) fn source_mesh_replaced_by_hair_v1(
    prepared: &PreparedPlayerHairV1,
    source_mesh_name: &str,
) -> bool {
    let name = source_mesh_name.trim().to_ascii_lowercase();
    !name.is_empty()
        && prepared.source_mesh_prefixes.iter().any(|prefix| {
            let prefix = prefix.trim().to_ascii_lowercase();
            !prefix.is_empty() && name.starts_with(&prefix)
        })
}

pub(crate) fn unbind_player_hair_v1(world: &mut World, player: EntityId) -> usize {
    let pose_id = player.stable_u64();
    if let Some(registry) = world.resource_mut::<PlayerHairBindingRegistryV1>() {
        registry.bindings.remove(&pose_id);
    }
    let Some(scene) = world.resource_mut::<HairSceneV1>() else {
        return 0;
    };
    let before = scene.instances.len();
    scene
        .instances
        .retain(|instance| instance.skin_pose_id != Some(pose_id));
    before.saturating_sub(scene.instances.len())
}

pub(crate) fn bind_prepared_player_hair_v1(
    world: &mut World,
    player: EntityId,
    prepared: PreparedPlayerHairV1,
) -> Result<(), String> {
    let pose_id = player.stable_u64();
    let state = PlayerHairBindingStateV1 {
        instance_id: prepared.instance.instance_id,
        authored_quality: prepared.instance.quality,
        hide_in_first_person: prepared.hide_in_first_person,
    };
    bind_compiled_player_groom_v1(
        world,
        player,
        prepared.groom,
        prepared.instance,
        prepared.shaders,
    )?;
    if world.resource::<PlayerHairBindingRegistryV1>().is_none() {
        world.insert_resource(PlayerHairBindingRegistryV1::default());
    }
    world
        .resource_mut::<PlayerHairBindingRegistryV1>()
        .ok_or_else(|| "PlayerHairBindingRegistryV1 unavailable after insertion".to_owned())?
        .bindings
        .insert(pose_id, state);
    Ok(())
}

/// Decode one compiled NorthStar NEHAIR groom through the canonical engine.assets VFS path.
pub fn load_nehair_groom_v1(logical_path: &str) -> Result<HairGroomAssetV1, String> {
    let logical_path = logical_path.trim().replace('\\', "/");
    if logical_path.is_empty()
        || logical_path.starts_with('/')
        || logical_path.contains(":/")
        || logical_path.contains("../")
    {
        return Err("NEHAIR path must be a VFS-relative logical asset path".to_owned());
    }
    let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let bytes = assets
        .raw_bytes_v1(&logical_path)
        .map_err(|error| format!("NEHAIR VFS read failed path='{logical_path}': {error}"))?;
    newengine_asset_format_nehair::decode_nehair(&bytes)
        .map_err(|error| format!("NEHAIR decode failed path='{logical_path}': {error}"))
}

/// Publish one compiled groom into the renderer-owned HairGroomRegistryV1.
pub fn install_nehair_groom_v1(world: &mut World, logical_path: &str) -> Result<(), String> {
    let groom = load_nehair_groom_v1(logical_path)?;
    if world.resource::<HairGroomRegistryV1>().is_none() {
        world.insert_resource(HairGroomRegistryV1::default());
    }
    world
        .resource_mut::<HairGroomRegistryV1>()
        .ok_or_else(|| "HairGroomRegistryV1 unavailable after insertion".to_owned())?
        .insert(groom)
}

/// Bind an already-decoded groom to an animated player. Content owns the instance/material
/// settings and shader set; this bridge only resolves the opaque pose id and registry ownership.
pub fn bind_compiled_player_groom_v1(
    world: &mut World,
    player: EntityId,
    groom: HairGroomAssetV1,
    mut instance: HairInstanceDescV1,
    shaders: HairShaderSetV1,
) -> Result<(), String> {
    let pose_id = player.stable_u64();
    if pose_id == 0 {
        return Err("player entity does not expose a stable non-zero hair pose id".to_owned());
    }
    let groom = groom.normalized()?;
    let groom_ref = groom.groom.clone();
    if world.resource::<HairGroomRegistryV1>().is_none() {
        world.insert_resource(HairGroomRegistryV1::default());
    }
    world
        .resource_mut::<HairGroomRegistryV1>()
        .ok_or_else(|| "HairGroomRegistryV1 unavailable after insertion".to_owned())?
        .insert(groom)?;

    instance.groom = groom_ref;
    instance.skin_pose_id = Some(pose_id);
    instance = instance.normalized()?;

    if world.resource::<HairSceneV1>().is_none() {
        world.insert_resource(HairSceneV1::new(shaders.clone()));
    }
    let scene = world
        .resource_mut::<HairSceneV1>()
        .ok_or_else(|| "HairSceneV1 unavailable after insertion".to_owned())?;
    if scene.shaders != shaders {
        if scene.instances.is_empty() {
            scene.shaders = shaders.clone();
        } else {
            return Err(
                "HairSceneV1 already exists with a different shader set while live instances remain"
                    .to_owned(),
            );
        }
    }
    if scene
        .instances
        .iter()
        .any(|existing| existing.instance_id == instance.instance_id)
    {
        return Err(format!(
            "hair instance id {} is already bound",
            instance.instance_id
        ));
    }
    scene.instances.push(instance);
    Ok(())
}

/// VFS-backed convenience path: decode NEHAIR through engine.assets and bind it to a player.
pub fn bind_player_nehair_v1(
    world: &mut World,
    player: EntityId,
    logical_path: &str,
    instance: HairInstanceDescV1,
    shaders: HairShaderSetV1,
) -> Result<(), String> {
    let groom = load_nehair_groom_v1(logical_path)?;
    bind_compiled_player_groom_v1(world, player, groom, instance, shaders)
}

/// Bridge the already-authoritative player animation palette into the provider-neutral
/// render.hair pose registry. Hair never evaluates animation graphs itself.
pub(crate) fn publish_player_hair_pose(world: &mut World, player: EntityId, model_to_world: Mat4) {
    let Some(pose) = world
        .get::<newengine_engine_runtime::gameplay::PlayerSkinPose>(player)
        .cloned()
    else {
        return;
    };
    let pose_id = player.stable_u64();
    if pose_id == 0 {
        return;
    }
    let hair_pose = HairSkinPoseV1 {
        pose_id,
        revision: pose.revision,
        joint_deforms: pose
            .palette
            .iter()
            .map(|matrix| matrix.to_cols_array())
            .collect(),
    };

    if world.resource::<HairSkinPoseRegistryV1>().is_none() {
        world.insert_resource(HairSkinPoseRegistryV1::default());
    }
    let upsert_result = world
        .resource_mut::<HairSkinPoseRegistryV1>()
        .ok_or_else(|| "HairSkinPoseRegistryV1 unavailable after insertion".to_owned())
        .and_then(|registry| registry.upsert(hair_pose));
    if let Err(error) = upsert_result {
        newengine_ulog_api::ulog::warn!(
            "game-ready: player hair pose publication rejected player={}: {}",
            player.stable_u64(),
            error
        );
        return;
    }

    // Instances are authored/configured by content. The product bridge keeps the root transform
    // synchronized and mirrors the character's first-person visibility policy without teaching
    // the renderer about gameplay camera modes.
    let first_person_active = world
        .resource::<newengine_engine_runtime::gameplay::PlayerViewState>()
        .map(|state| state.first_person_active)
        .unwrap_or(false);
    let binding_state = world
        .resource::<PlayerHairBindingRegistryV1>()
        .and_then(|registry| registry.bindings.get(&pose_id).copied());
    if let Some(scene) = world.resource_mut::<HairSceneV1>() {
        let root = model_to_world.to_cols_array();
        for instance in &mut scene.instances {
            if instance.skin_pose_id == Some(pose_id) {
                instance.root_transform = root;
                if let Some(binding) =
                    binding_state.filter(|binding| binding.instance_id == instance.instance_id)
                {
                    instance.quality = if binding.hide_in_first_person && first_person_active {
                        HairQualityTier::Off
                    } else {
                        binding.authored_quality
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_engine_runtime::gameplay::PlayerSkinPose;
    use newengine_math::{Mat4, Vec3};
    use newengine_render_api::{HairGroomRef, HairInstanceDescV1, HairShaderSetV1};

    fn prepared_with_prefixes(prefixes: &[&str]) -> PreparedPlayerHairV1 {
        PreparedPlayerHairV1 {
            groom: HairGroomAssetV1 {
                groom: HairGroomRef::new("characters/test/hair.nehair"),
                guide_points: Vec::new(),
                guide_strands: Vec::new(),
                collision_capsules: Vec::new(),
                follow_strands_per_guide: 0,
            },
            instance: HairInstanceDescV1::default(),
            shaders: HairShaderSetV1::new(
                "shaders/hair/guide_sim.comp",
                "shaders/hair/strand_ribbon.vert",
                "shaders/hair/strand_ribbon.frag",
            ),
            source_mesh_prefixes: prefixes.iter().map(|value| (*value).to_owned()).collect(),
            hide_in_first_person: true,
        }
    }

    #[test]
    fn ellie_cutover_keeps_cap_back_and_scrunchy() {
        let prepared = prepared_with_prefixes(&[
            "default_hair_wet_extra_",
            "default_hair_thick_",
            "default_hair_tail_thin_",
            "default_hair_flying_",
            "default_strand_hair_loose_",
        ]);
        assert!(source_mesh_replaced_by_hair_v1(
            &prepared,
            "default_hair_thick_lod0_LODShape0_shader2_merged_partition0"
        ));
        assert!(source_mesh_replaced_by_hair_v1(
            &prepared,
            "default_strand_hair_loose_lod0_LODShape0_shader1_merged_partition0"
        ));
        assert!(!source_mesh_replaced_by_hair_v1(
            &prepared,
            "default_hair_base_lod0_LODShape0_shader1_merged_partition0"
        ));
        assert!(!source_mesh_replaced_by_hair_v1(
            &prepared,
            "default_hair_back_lod0_LODShape0_shader7_merged_partition0"
        ));
        assert!(!source_mesh_replaced_by_hair_v1(
            &prepared,
            "default_scrunchy_lod0_LODShape0_shader5_merged_partition0"
        ));
    }

    #[test]
    fn isaac_cutover_excludes_beard_brows_fuzz_and_scalp_cap() {
        let prepared = prepared_with_prefixes(&[
            "hair_LODShape0_shader8_",
            "hair_LODShape0_shader10_",
            "hair_LODShape0_shader11_",
        ]);
        assert!(source_mesh_replaced_by_hair_v1(
            &prepared,
            "hair_LODShape0_shader8_merged_partition0"
        ));
        assert!(source_mesh_replaced_by_hair_v1(
            &prepared,
            "hair_LODShape0_shader10_merged_partition0"
        ));
        assert!(!source_mesh_replaced_by_hair_v1(
            &prepared,
            "hair_LODShape0_shader7_merged_partition0"
        ));
        assert!(!source_mesh_replaced_by_hair_v1(
            &prepared,
            "hair_LODShape0_shader9_merged_partition0"
        ));
        assert!(!source_mesh_replaced_by_hair_v1(
            &prepared,
            "beard_LODShape0_shader12_merged_partition0"
        ));
        assert!(!source_mesh_replaced_by_hair_v1(
            &prepared,
            "eyebrows_LODShape0_shader4_merged_partition0"
        ));
    }

    #[test]
    fn compiled_groom_binding_targets_player_pose_id() {
        let mut world = World::new();
        let player = world.spawn();
        let groom = HairGroomAssetV1 {
            groom: HairGroomRef::new("characters/test/hair.nehair"),
            guide_points: vec![
                newengine_render_api::HairGuidePointV1 {
                    rest_position: [0.0, 0.0, 0.0],
                    inverse_mass: 0.0,
                },
                newengine_render_api::HairGuidePointV1 {
                    rest_position: [0.0, -0.1, 0.0],
                    inverse_mass: 1.0,
                },
            ],
            guide_strands: vec![newengine_render_api::HairGuideStrandV1 {
                first_point: 0,
                point_count: 2,
                group: 0,
                root_uv: [0.5, 0.5],
                root_joint_index: 0,
            }],
            collision_capsules: Vec::new(),
            follow_strands_per_guide: 0,
        };
        let shaders = HairShaderSetV1::new(
            "shaders/hair/guide_sim.comp",
            "shaders/hair/strand_ribbon.vert",
            "shaders/hair/strand_ribbon.frag",
        );
        bind_compiled_player_groom_v1(
            &mut world,
            player,
            groom,
            HairInstanceDescV1 {
                instance_id: 55,
                ..Default::default()
            },
            shaders,
        )
        .unwrap();
        let scene = world.resource::<HairSceneV1>().unwrap();
        assert_eq!(scene.instances.len(), 1);
        assert_eq!(scene.instances[0].skin_pose_id, Some(player.stable_u64()));
        assert_eq!(
            scene.instances[0].groom.as_str(),
            "characters/test/hair.nehair"
        );
    }

    #[test]
    fn player_palette_is_published_without_animation_types_in_hair_contract() {
        let mut world = World::new();
        let player = world.spawn();
        assert!(world.insert(
            player,
            PlayerSkinPose {
                palette: vec![Mat4::IDENTITY],
                revision: 4,
            },
        ));
        let mut scene = HairSceneV1::new(HairShaderSetV1::new(
            "shaders/hair/guide_sim.comp",
            "shaders/hair/strand_ribbon.vert",
            "shaders/hair/strand_ribbon.frag",
        ));
        scene.instances.push(HairInstanceDescV1 {
            instance_id: 9,
            groom: HairGroomRef::new("characters/test/hair.nehair"),
            skin_pose_id: Some(player.stable_u64()),
            ..Default::default()
        });
        world.insert_resource(scene);

        let root = Mat4::from_translation(Vec3::new(3.0, 2.0, 1.0));
        publish_player_hair_pose(&mut world, player, root);

        let registry = world.resource::<HairSkinPoseRegistryV1>().unwrap();
        let pose = registry.get(player.stable_u64()).unwrap();
        assert_eq!(pose.revision, 4);
        assert_eq!(pose.joint_deforms.len(), 1);
        let scene = world.resource::<HairSceneV1>().unwrap();
        assert_eq!(scene.instances[0].root_transform, root.to_cols_array());
    }
}
