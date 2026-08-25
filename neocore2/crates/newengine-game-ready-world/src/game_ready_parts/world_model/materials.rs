use super::super::*;
use newengine_materials::api::MaterialRegistryApi;

const WORLD_MATERIAL_LIBRARY: &str = newengine_game_data::WORLD_MATERIAL_LIBRARY;

#[derive(Clone, Copy, Debug)]
pub(super) struct ForestRoadMaterials {
    pub(super) road: MaterialId,
    pub(super) terrain: MaterialId,
    pub(super) props: MaterialId,
}

fn material_spec(entry: &str, roughness: f32) -> GameReadyMaterialSpec {
    GameReadyMaterialSpec {
        asset: Some(format!("{WORLD_MATERIAL_LIBRARY}@{entry}")),
        base_color_texture: None,
        normal_texture: None,
        roughness_texture: None,
        uv_scale: [1.0, 1.0],
        uv_offset: [0.0, 0.0],
        roughness,
        normal_scale: 0.0,
        occlusion_strength: 1.0,
    }
}

pub(super) fn register_forest_road_materials(mats: &MaterialRegistry) -> ForestRoadMaterials {
    let opaque_world = MaterialFlags::DOUBLE_SIDED
        .union(MaterialFlags::CAST_SHADOWS)
        .union(MaterialFlags::RECEIVE_SHADOWS);
    let road = material_spec("forest_road_road", 0.92);
    let terrain = material_spec("forest_road_terrain", 0.96);
    let props = material_spec("forest_road_props", 0.82);

    ForestRoadMaterials {
        road: register_material(
            mats,
            "World/ForestRoad/Road",
            [0.24, 0.16, 0.08, 1.0],
            [0.0, 0.0, 0.0],
            1.0,
            opaque_world,
            &road,
        ),
        terrain: register_material(
            mats,
            "World/ForestRoad/Terrain",
            [0.10, 0.18, 0.08, 1.0],
            [0.0, 0.0, 0.0],
            1.0,
            opaque_world,
            &terrain,
        ),
        props: register_material(
            mats,
            "World/ForestRoad/Props",
            [0.24, 0.13, 0.055, 1.0],
            [0.0, 0.0, 0.0],
            1.0,
            opaque_world,
            &props,
        ),
    }
}

pub(super) fn register_authored_prefab_material(
    mats: &MaterialRegistry,
    prefab: &GameReadyPrefabSpec,
) -> Option<MaterialId> {
    let raw_asset = prefab.material.trim();
    if raw_asset.is_empty() {
        return None;
    }
    let asset = match newengine_assets_api::require_asset_reference_extension(
        raw_asset,
        &["nemat"],
        true,
    ) {
        Ok(reference) => reference.canonical,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "static world prefab id='{}' material='{}' ignored: {}",
                prefab.id,
                raw_asset,
                error,
            );
            return None;
        }
    };
    let spec = GameReadyMaterialSpec {
        asset: Some(asset.clone()),
        base_color_texture: None,
        normal_texture: None,
        roughness_texture: None,
        uv_scale: [1.0, 1.0],
        uv_offset: [0.0, 0.0],
        roughness: 0.82,
        normal_scale: 0.72,
        occlusion_strength: 1.0,
    };
    // Authored `.nemat` owns the double-sided policy. Runtime contributes only
    // the static-world shadow requirements so terrain can explicitly cull its
    // underside while foliage/props remain two-sided when their asset says so.
    let flags = MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS);
    Some(register_material(
        mats,
        &format!("World/Static/{}/Material", prefab.id),
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0, 0.0],
        1.0,
        flags,
        &spec,
    ))
}

#[inline]
pub(super) fn static_world_decal_slot(slot: &str) -> bool {
    let slot = slot.trim().to_ascii_lowercase();
    if slot.ends_with("dirt_road") && !slot.ends_with("dirt_road_bare") {
        return true;
    }
    [
        "dirt_road_trails",
        "puddle_streaks",
        "road_edge_gravel",
        "rock_decal",
        "fallen_generic_leaves",
        "fallen_maple_leaves",
    ]
    .iter()
    .any(|tag| slot.contains(tag))
}

#[inline]
pub(super) fn static_world_receive_only_shadow_slot(slot: &str) -> bool {
    let slot = slot.trim().to_ascii_lowercase();
    [
        "dirt_road_bare",
        "ground_dirt",
        "terrain_far",
        "aerial_grass",
        "grass_close",
        "cobblestone",
        // Thin overlay/decal-like meshes must never cast a second nearly
        // coplanar shadow over their supporting road/terrain surface. Their
        // visible color can receive the world shadow, but suppressing them as
        // casters removes camera-dependent shadow acne/flicker.
        "dirt_road_trails",
        "puddle_streaks",
        "road_edge_gravel",
        "rock_decal",
        "fallen_generic_leaves",
        "fallen_maple_leaves",
    ]
    .iter()
    .any(|tag| slot.contains(tag))
}

#[inline]
pub(super) fn material_for_slot(materials: ForestRoadMaterials, slot: &str) -> MaterialId {
    let slot = slot.trim().to_ascii_lowercase();
    if slot.contains("terrain") || slot.contains("ground") {
        materials.terrain
    } else if slot.contains("props") || slot.contains("wood") || slot.contains("rock") {
        materials.props
    } else {
        materials.road
    }
}

#[inline]
pub(super) fn resolve_prefab_part_material(
    mats: &MaterialRegistry,
    authored_material: Option<MaterialId>,
    profile_materials: ForestRoadMaterials,
    material_slot: &str,
) -> (MaterialId, newengine_model_domain_api::MeshRenderOptions) {
    // The profile owns the no-authored-material policy explicitly. This is not an
    // asset-loading fallback: ForestRoad registers the complete slot material set
    // before prefab admission and selects one deterministically here.
    let material_id = match authored_material {
        Some(material_id) => material_id,
        None => material_for_slot(profile_materials, material_slot),
    };
    let render_options = match mats.resolve(material_id) {
        Some(material) if material.desc.flags.contains(MaterialFlags::ALPHA_TEST) => {
            newengine_model_domain_api::MeshRenderOptions::world_masked()
        }
        Some(_) => newengine_model_domain_api::MeshRenderOptions::world_opaque(),
        None => {
            newengine_ulog_api::ulog::error!(
                "static world material registry invariant failed for slot='{}'; using explicit opaque recovery policy",
                material_slot,
            );
            newengine_model_domain_api::MeshRenderOptions::world_opaque()
        }
    };
    (material_id, render_options)
}
