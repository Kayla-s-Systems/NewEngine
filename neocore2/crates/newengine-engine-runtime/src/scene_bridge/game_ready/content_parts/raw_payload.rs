
use newengine_math::Vec3;
use serde::Deserialize;


pub(super) use self::profile::*;
use self::paths::{profile_asset_candidates, GAME_READY_APP_DIR};


#[derive(Debug, Deserialize)]
struct RawGameReadyPayload {
    #[serde(default = "default_title")]
    title: String,
    #[serde(default = "default_objective")]
    objective: String,
    #[serde(default)]
    player: RawPlayerSpec,
    #[serde(default)]
    terrain: RawTerrainSpec,
    #[serde(default)]
    sky: RawSkySpec,
    #[serde(default)]
    materials: RawMaterialSetSpec,
    #[serde(default)]
    lighting: RawLightingSpec,
    #[serde(default)]
    foliage: RawFoliageSpec,
    #[serde(default)]
    prefabs: Vec<RawPrefabSpec>,
    #[serde(default)]
    gameplay: RawGameplaySpec,
    #[serde(default)]
    palette: RawPaletteSpec,
}

#[derive(Debug, Deserialize)]
struct RawPlayerSpec {
    #[serde(default = "default_player_start")]
    start: [f32; 3],
    #[serde(default = "default_player_yaw")]
    yaw: f32,
    #[serde(default = "default_move_speed")]
    move_speed: f32,
    #[serde(default = "default_look_sens")]
    look_sens: f32,
}

impl Default for RawPlayerSpec {
    fn default() -> Self {
        Self {
            start: default_player_start(),
            yaw: default_player_yaw(),
            move_speed: default_move_speed(),
            look_sens: default_look_sens(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawTerrainSpec {
    #[serde(default = "default_terrain_seed")]
    seed: u64,
    #[serde(default = "default_terrain_cells")]
    cells_x: u32,
    #[serde(default = "default_terrain_cells")]
    cells_z: u32,
    #[serde(default = "default_terrain_size")]
    size_x: f32,
    #[serde(default = "default_terrain_size")]
    size_z: f32,
    #[serde(default = "default_base_height")]
    base_height: f32,
    #[serde(default = "default_height_scale")]
    height_scale: f32,
    #[serde(default = "default_collision_tile_cells")]
    collision_tile_cells: u32,
    #[serde(default = "default_collision_floor_depth")]
    collision_floor_depth: f32,
    #[serde(default = "default_collision_horizontal_skin")]
    collision_horizontal_skin: f32,
    #[serde(default)]
    generator: RawTerrainGeneratorSpec,
}

impl Default for RawTerrainSpec {
    fn default() -> Self {
        Self {
            seed: default_terrain_seed(),
            cells_x: default_terrain_cells(),
            cells_z: default_terrain_cells(),
            size_x: default_terrain_size(),
            size_z: default_terrain_size(),
            base_height: default_base_height(),
            height_scale: default_height_scale(),
            collision_tile_cells: default_collision_tile_cells(),
            collision_floor_depth: default_collision_floor_depth(),
            collision_horizontal_skin: default_collision_horizontal_skin(),
            generator: RawTerrainGeneratorSpec::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawTerrainGeneratorSpec {
    #[serde(default = "default_terrain_generator_id")]
    id: String,
    #[serde(default = "default_ridged_seed_xor")]
    ridged_seed_xor: u64,
    #[serde(default = "default_ridged_frequency")]
    ridged_frequency: f32,
    #[serde(default = "default_ridged_amplitude")]
    ridged_amplitude: f32,
    #[serde(default = "default_ridged_shape_edge0")]
    ridged_shape_edge0: f32,
    #[serde(default = "default_ridged_shape_edge1")]
    ridged_shape_edge1: f32,
    #[serde(default = "default_veins_seed_xor")]
    veins_seed_xor: u64,
    #[serde(default = "default_veins_frequency")]
    veins_frequency: f32,
    #[serde(default = "default_veins_amplitude")]
    veins_amplitude: f32,
    #[serde(default = "default_smoothing_passes")]
    smoothing_passes: u32,
    #[serde(default = "default_smoothing_strength")]
    smoothing_strength: f32,
}

impl Default for RawTerrainGeneratorSpec {
    fn default() -> Self {
        Self {
            id: default_terrain_generator_id(),
            ridged_seed_xor: default_ridged_seed_xor(),
            ridged_frequency: default_ridged_frequency(),
            ridged_amplitude: default_ridged_amplitude(),
            ridged_shape_edge0: default_ridged_shape_edge0(),
            ridged_shape_edge1: default_ridged_shape_edge1(),
            veins_seed_xor: default_veins_seed_xor(),
            veins_frequency: default_veins_frequency(),
            veins_amplitude: default_veins_amplitude(),
            smoothing_passes: default_smoothing_passes(),
            smoothing_strength: default_smoothing_strength(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawSkySpec {
    #[serde(default = "default_sky_radius")]
    radius: f32,
    #[serde(default = "default_skydome_mesh")]
    mesh: String,
}

impl Default for RawSkySpec {
    fn default() -> Self {
        Self {
            radius: default_sky_radius(),
            mesh: default_skydome_mesh(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawGameplaySpec {
    #[serde(default = "default_status_text")]
    default_status: String,
    #[serde(default = "default_pickup_status")]
    pickup_status: String,
    #[serde(default = "default_hazard_status")]
    hazard_status: String,
    #[serde(default = "default_goal_locked_status")]
    goal_locked_status: String,
    #[serde(default = "default_goal_complete_status")]
    goal_complete_status: String,
    #[serde(default = "default_failed_progress_label")]
    failed_progress_label: String,
    #[serde(default = "default_completed_progress_label")]
    completed_progress_label: String,
    #[serde(default)]
    player_collision: RawPlayerCollisionSpec,
    #[serde(default)]
    player_visual: RawPlayerVisualSpec,
    #[serde(default)]
    physics: RawPhysicsSpec,
}

impl Default for RawGameplaySpec {
    fn default() -> Self {
        Self {
            default_status: default_status_text(),
            pickup_status: default_pickup_status(),
            hazard_status: default_hazard_status(),
            goal_locked_status: default_goal_locked_status(),
            goal_complete_status: default_goal_complete_status(),
            failed_progress_label: default_failed_progress_label(),
            completed_progress_label: default_completed_progress_label(),
            player_collision: RawPlayerCollisionSpec::default(),
            player_visual: RawPlayerVisualSpec::default(),
            physics: RawPhysicsSpec::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawPlayerCollisionSpec {
    #[serde(default = "default_player_body_radius")]
    radius: f32,
    #[serde(default = "default_player_body_half_height")]
    half_height: f32,
}

impl Default for RawPlayerCollisionSpec {
    fn default() -> Self {
        Self {
            radius: default_player_body_radius(),
            half_height: default_player_body_half_height(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawPlayerVisualSpec {
    #[serde(default = "default_player_visual_radius")]
    radius: f32,
    #[serde(default = "default_player_visual_half_height")]
    half_height: f32,
    #[serde(default = "default_camera_eye_height")]
    camera_eye_height: f32,
    #[serde(default = "default_sprint_multiplier")]
    sprint_multiplier: f32,
}

impl Default for RawPlayerVisualSpec {
    fn default() -> Self {
        Self {
            radius: default_player_visual_radius(),
            half_height: default_player_visual_half_height(),
            camera_eye_height: default_camera_eye_height(),
            sprint_multiplier: default_sprint_multiplier(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawPhysicsSpec {
    #[serde(default = "default_gravity")]
    gravity: f32,
    #[serde(default = "default_contact_skin")]
    contact_skin: f32,
}

impl Default for RawPhysicsSpec {
    fn default() -> Self {
        Self {
            gravity: default_gravity(),
            contact_skin: default_contact_skin(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawPaletteSpec {
    #[serde(default = "default_terrain_color")]
    terrain: ColorRgba,
    #[serde(default = "default_sky_color")]
    sky: ColorRgba,
    #[serde(default = "default_sky_emissive")]
    sky_emissive: ColorRgb,
    #[serde(default = "default_tree_bark_color")]
    tree_bark: ColorRgba,
    #[serde(default = "default_tree_leaf_color")]
    tree_leaf: ColorRgba,
    #[serde(default = "default_tree_branch_color")]
    tree_branch: ColorRgba,
}

impl Default for RawPaletteSpec {
    fn default() -> Self {
        Self {
            terrain: default_terrain_color(),
            sky: default_sky_color(),
            sky_emissive: default_sky_emissive(),
            tree_bark: default_tree_bark_color(),
            tree_leaf: default_tree_leaf_color(),
            tree_branch: default_tree_branch_color(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawMaterialSetSpec {
    #[serde(default = "default_terrain_material")]
    terrain: RawMaterialSpec,
    #[serde(default = "default_sky_material")]
    sky: RawMaterialSpec,
    #[serde(default = "default_tree_bark_material")]
    tree_bark: RawMaterialSpec,
    #[serde(default = "default_tree_leaf_material")]
    tree_leaf: RawMaterialSpec,
    #[serde(default = "default_tree_branch_material")]
    tree_branch: RawMaterialSpec,
}

impl Default for RawMaterialSetSpec {
    fn default() -> Self {
        Self {
            terrain: default_terrain_material(),
            sky: default_sky_material(),
            tree_bark: default_tree_bark_material(),
            tree_leaf: default_tree_leaf_material(),
            tree_branch: default_tree_branch_material(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RawMaterialSpec {
    #[serde(default)]
    asset: Option<String>,
    #[serde(default)]
    base_color_texture: Option<String>,
    #[serde(default)]
    normal_texture: Option<String>,
    #[serde(default)]
    roughness_texture: Option<String>,
    #[serde(default = "default_uv_scale")]
    uv_scale: [f32; 2],
    #[serde(default = "default_uv_offset")]
    uv_offset: [f32; 2],
    #[serde(default = "default_material_roughness")]
    roughness: f32,
    #[serde(default = "default_material_normal_scale")]
    normal_scale: f32,
    #[serde(default = "default_material_occlusion_strength")]
    occlusion_strength: f32,
}

impl Default for RawMaterialSpec {
    fn default() -> Self {
        Self {
            asset: None,
            base_color_texture: None,
            normal_texture: None,
            roughness_texture: None,
            uv_scale: default_uv_scale(),
            uv_offset: default_uv_offset(),
            roughness: default_material_roughness(),
            normal_scale: default_material_normal_scale(),
            occlusion_strength: default_material_occlusion_strength(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawLightingSpec {
    #[serde(default = "default_ambient_color")]
    ambient_color: ColorRgb,
    #[serde(default = "default_ambient_intensity")]
    ambient_intensity: f32,
    #[serde(default = "default_sun_direction")]
    sun_direction: ColorRgb,
    #[serde(default = "default_sun_color")]
    sun_color: ColorRgb,
    #[serde(default = "default_sun_intensity")]
    sun_intensity: f32,
    #[serde(default)]
    shadows: RawShadowSpec,
}

impl Default for RawLightingSpec {
    fn default() -> Self {
        Self {
            ambient_color: default_ambient_color(),
            ambient_intensity: default_ambient_intensity(),
            sun_direction: default_sun_direction(),
            sun_color: default_sun_color(),
            sun_intensity: default_sun_intensity(),
            shadows: RawShadowSpec::default(),
        }
    }
}
