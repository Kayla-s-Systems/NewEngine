
use newengine_math::Vec3;
use serde::Deserialize;

mod raw_payload_defaults;


pub(super) use self::profile::*;
use self::paths::profile_asset_candidates;


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
    definitions: Vec<RawDefinitionInstanceSpec>,
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
    #[serde(default)]
    model: RawPlayerModelSpec,
}


#[derive(Debug, Deserialize)]
struct RawPlayerModelSpec {
    #[serde(default = "default_player_model_enabled")]
    enabled: bool,
    #[serde(default = "default_player_model_source")]
    source: String,
    #[serde(default = "default_player_texture_dictionary")]
    texture_dictionary: Option<String>,
    #[serde(default = "default_player_skeleton")]
    skeleton: Option<String>,
    #[serde(default = "default_player_model_height")]
    target_height: f32,
    #[serde(default = "default_player_model_eye_height_ratio")]
    eye_height_ratio: f32,
    #[serde(default = "default_player_model_offset")]
    local_offset: [f32; 3],
    #[serde(default = "default_player_model_yaw_offset")]
    yaw_offset: f32,
    #[serde(default = "default_player_model_hide_in_first_person")]
    hide_in_first_person: bool,
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
    #[serde(default)]
    generator: RawTerrainGeneratorSpec,
    #[serde(default)]
    surface: RawTerrainSurfaceSpec,
    #[serde(default)]
    streaming: RawTerrainStreamingSpec,
}

#[derive(Debug, Deserialize)]
struct RawTerrainSurfaceSpec {
    #[serde(default = "default_terrain_surface_forest")]
    forest_base_texture: String,
    #[serde(default = "default_terrain_surface_sand")]
    sand_base_texture: String,
    #[serde(default = "default_terrain_surface_rock")]
    rock_base_texture: String,
    #[serde(default = "default_terrain_patch_scale")]
    patch_scale: f32,
    #[serde(default = "default_terrain_blend_softness")]
    blend_softness: f32,
}


#[derive(Debug, Deserialize)]
struct RawTerrainStreamingSpec {
    #[serde(default = "default_terrain_streaming_enabled")]
    enabled: bool,
    #[serde(default = "default_terrain_chunk_radius")]
    chunk_radius: i32,
    #[serde(default = "default_terrain_unload_radius")]
    unload_radius: i32,
    #[serde(default = "default_terrain_max_chunks_per_frame")]
    max_chunks_per_frame: usize,
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


#[derive(Debug, Deserialize)]
struct RawSkySpec {
    #[serde(default = "default_sky_radius")]
    radius: f32,
    #[serde(default = "default_skydome_mesh")]
    mesh: String,
    #[serde(default = "default_sky_follow_camera")]
    follow_camera: bool,
    #[serde(default = "default_cloud_dictionary")]
    cloud_dictionary: String,
    #[serde(default = "default_cloud_profile")]
    cloud_profile: String,
    #[serde(default = "default_sky_sun_radius")]
    sun_radius: f32,
    #[serde(default = "default_sky_moon_radius")]
    moon_radius: f32,
    #[serde(default = "default_moon_texture")]
    moon_texture: String,
    #[serde(default)]
    atmosphere: RawSkyAtmosphereSpec,
}


#[derive(Debug, Deserialize)]
struct RawSkyAtmosphereSpec {
    #[serde(default = "default_sky_day_zenith")]
    day_zenith: ColorRgb,
    #[serde(default = "default_sky_day_horizon")]
    day_horizon: ColorRgb,
    #[serde(default = "default_sky_dusk_zenith")]
    dusk_zenith: ColorRgb,
    #[serde(default = "default_sky_dusk_horizon")]
    dusk_horizon: ColorRgb,
    #[serde(default = "default_sky_night_zenith")]
    night_zenith: ColorRgb,
    #[serde(default = "default_sky_night_horizon")]
    night_horizon: ColorRgb,
    #[serde(default = "default_sky_cloud_day")]
    cloud_day: ColorRgb,
    #[serde(default = "default_sky_cloud_night")]
    cloud_night: ColorRgb,
    #[serde(default = "default_sky_night_strength")]
    night_sky_strength: f32,
    #[serde(default = "default_sky_cloud_coverage")]
    cloud_coverage: f32,
    #[serde(default = "default_sky_cloud_softness")]
    cloud_softness: f32,
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


#[derive(Debug, Deserialize)]
struct RawPlayerCollisionSpec {
    #[serde(default = "default_player_body_radius")]
    radius: f32,
    #[serde(default = "default_player_body_half_height")]
    half_height: f32,
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


#[derive(Debug, Deserialize)]
struct RawPhysicsSpec {
    #[serde(default = "default_gravity")]
    gravity: f32,
    #[serde(default = "default_contact_skin")]
    contact_skin: f32,
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


#[derive(Debug, Deserialize)]
struct RawMaterialSetSpec {
    #[serde(default = "default_terrain_material")]
    terrain: RawMaterialSpec,
    #[serde(default = "default_sky_material")]
    sky: RawMaterialSpec,
    #[serde(default = "default_sun_material")]
    sun: RawMaterialSpec,
    #[serde(default = "default_moon_material")]
    moon: RawMaterialSpec,
    #[serde(default = "default_tree_bark_material")]
    tree_bark: RawMaterialSpec,
    #[serde(default = "default_tree_leaf_material")]
    tree_leaf: RawMaterialSpec,
    #[serde(default = "default_tree_branch_material")]
    tree_branch: RawMaterialSpec,
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
    #[serde(default)]
    day_night: RawDayNightSpec,
}


#[derive(Debug, Deserialize)]
struct RawDayNightSpec {
    #[serde(default = "default_day_night_enabled")]
    enabled: bool,
    #[serde(default = "default_time_of_day_hours")]
    time_of_day_hours: f32,
    #[serde(default = "default_day_length_seconds")]
    day_length_seconds: f32,
    #[serde(default = "default_sun_latitude_degrees")]
    latitude_degrees: f32,
    #[serde(default = "default_axial_tilt_degrees")]
    axial_tilt_degrees: f32,
}

