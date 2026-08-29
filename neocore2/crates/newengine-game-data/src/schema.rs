use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameData {
    pub schema: String,
    pub version: u32,
    pub runtime: RuntimeData,
    pub world: WorldData,
    pub player: PlayerData,
    pub gameplay: GameplayData,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeData {
    pub fixed_dt_ms: u32,
    pub app_name: String,
    pub app_dir_name: String,
    pub window_title: String,
    pub default_profile_asset: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldData {
    pub title: String,
    pub objective: String,
    pub terrain: TerrainData,
    pub sky: SkyData,
    pub palette: PaletteData,
    pub material: MaterialDefaultsData,
    pub lighting: LightingData,
    pub shadows: ShadowData,
    pub day_night: DayNightData,
    pub foliage: FoliageData,
    pub mission: MissionDefaultsData,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerData {
    pub spawn: [f32; 3],
    pub yaw: f32,
    #[serde(default = "default_player_move_speed")]
    pub move_speed: f32,
    pub look_sensitivity: f32,
    #[serde(default = "default_player_character_ref")]
    pub character_ref: String,
    #[serde(default)]
    pub model: PlayerModelData,
    #[serde(default)]
    pub tuning: PlayerTuningData,
}

fn default_player_character_ref() -> String {
    crate::DEFAULT_PLAYER_CHARACTER_REF.to_owned()
}

fn default_player_move_speed() -> f32 {
    3.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerModelData {
    pub enabled: bool,
    pub source: String,
    pub target_height: f32,
    pub eye_height_ratio: f32,
    pub local_offset: [f32; 3],
    pub yaw_offset: f32,
    pub hide_in_first_person: bool,
}

/// Optional authored spring/K locomotion response parameters.
///
/// Absence means the character definition does not provide an original response model. The
/// runtime must not synthesize TLOU2-derived constants for another character/profile.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerMotionResponseData {
    pub velocity_spring_const: f32,
    pub velocity_spring_const_decel: f32,
    pub velocity_spring_dampen_ratio: f32,
    pub speed_spring_const: f32,
    pub max_accel: f32,
    pub trans_clamp_dist: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerTuningData {
    #[serde(default)]
    pub motion_response: Option<PlayerMotionResponseData>,
    pub body_radius: f32,
    pub body_half_height: f32,
    pub crouched_body_half_height: f32,
    pub visual_radius: f32,
    pub visual_half_height: f32,
    pub camera_eye_height: f32,
    pub crouched_camera_eye_height: f32,
    pub crouch_camera_speed: f32,
    pub sprint_multiplier: f32,
    pub jump_speed: f32,
    pub gravity: f32,
    pub contact_skin: f32,
    pub ground_probe_distance: f32,
    pub max_slope_degrees: f32,
    pub footstep_stride: f32,
    pub landing_speed_threshold: f32,
    pub locomotion_min_horizontal_speed: f32,
    pub ground_probe_max_upward_velocity: f32,
    pub landing_min_airborne_seconds: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameplayData {
    pub status: GameplayStatusData,
    pub projectile: ProjectileData,
    pub inventory: InventoryDefaultsData,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameplayStatusData {
    pub default_status: String,
    pub pickup_status: String,
    pub target_status: String,
    pub hazard_status: String,
    pub goal_locked_status: String,
    pub goal_complete_status: String,
    pub failed_progress_label: String,
    pub completed_progress_label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectileData {
    pub radius: f32,
    pub speed: f32,
    pub lifetime_seconds: f32,
    pub spawn_clearance: f32,
    pub restitution: f32,
    pub friction: f32,
    pub density: f32,
    pub angular_velocity: [f32; 3],
    pub color: [f32; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InventoryDefaultsData {
    pub rifle_item: String,
    pub rifle_ammo: String,
    pub medkit_item: String,
    pub loadout: String,
    pub package_asset: String,
    pub hud_slots: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainData {
    pub enabled: bool,
    pub seed: u64,
    pub cells: u32,
    pub size: f32,
    pub base_height: f32,
    pub height_scale: f32,
    pub generator: TerrainGeneratorData,
    pub surface: TerrainSurfaceData,
    pub heightmap: TerrainHeightmapData,
    pub streaming: TerrainStreamingData,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainGeneratorData {
    pub id: String,
    pub ridged_seed_xor: u64,
    pub ridged_frequency: f32,
    pub ridged_amplitude: f32,
    pub ridged_shape_edge0: f32,
    pub ridged_shape_edge1: f32,
    pub veins_seed_xor: u64,
    pub veins_frequency: f32,
    pub veins_amplitude: f32,
    pub smoothing_passes: u32,
    pub smoothing_strength: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainSurfaceData {
    pub forest_texture: String,
    pub sand_texture: String,
    pub rock_texture: String,
    pub patch_scale: f32,
    pub blend_softness: f32,
    pub layer_weight: f32,
    pub layer_uv_scale: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainHeightmapData {
    pub enabled: bool,
    pub source: String,
    pub mode: String,
    pub strength: f32,
    pub min_height: f32,
    pub max_height: f32,
    pub tile_scale: [f32; 2],
    pub tile_offset: [f32; 2],
    pub invert: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainStreamingData {
    pub enabled: bool,
    pub chunk_radius: i32,
    pub unload_radius: i32,
    pub max_chunks_per_frame: usize,
    pub launch_warm_radius: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkyData {
    pub definition_ref: String,
    pub radius: f32,
    pub mesh: String,
    pub follow_camera: bool,
    /// Environment provider profile. Empty keeps the provider default.
    #[serde(default)]
    pub environment_profile: String,
    /// Optional world-environment routing metadata.
    #[serde(default)]
    pub environment_region: String,
    #[serde(default)]
    pub environment_biome: String,
    pub cloud_dictionary: String,
    pub cloud_profile: String,
    pub sun_radius: f32,
    pub moon_radius: f32,
    pub moon_texture: String,
    pub atmosphere: SkyAtmosphereData,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkyAtmosphereData {
    pub day_zenith: [f32; 3],
    pub day_horizon: [f32; 3],
    pub dusk_zenith: [f32; 3],
    pub dusk_horizon: [f32; 3],
    pub night_zenith: [f32; 3],
    pub night_horizon: [f32; 3],
    pub cloud_day: [f32; 3],
    pub cloud_night: [f32; 3],
    pub night_sky_strength: f32,
    pub cloud_coverage: f32,
    pub cloud_softness: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaletteData {
    pub terrain: [f32; 4],
    pub sky: [f32; 4],
    pub sky_emissive: [f32; 3],
    pub tree_bark: [f32; 4],
    pub tree_leaf: [f32; 4],
    pub tree_branch: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaterialDefaultsData {
    pub uv_scale: [f32; 2],
    pub uv_offset: [f32; 2],
    pub roughness: f32,
    pub normal_scale: f32,
    pub occlusion_strength: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LightingData {
    pub ambient_color: [f32; 3],
    pub ambient_intensity: f32,
    pub sun_direction: [f32; 3],
    pub sun_color: [f32; 3],
    pub sun_intensity: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShadowData {
    pub enabled: bool,
    pub resolution: u32,
    pub cascade_count: u32,
    pub max_distance: f32,
    pub softness: f32,
    pub bias: f32,
    pub normal_bias: f32,
    pub contact_strength: f32,
    pub filter: String,
    pub pcss_light_angular_radius_degrees: f32,
    pub pcss_blocker_search_radius_texels: f32,
    pub pcss_max_filter_radius_texels: f32,
    pub pcss_blocker_samples: u32,
    pub pcss_filter_samples: u32,
    pub pcss_min_filter_radius_texels: f32,
    pub pcss_stable_kernel_cell_texels: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DayNightData {
    pub enabled: bool,
    pub time_of_day_hours: f32,
    pub day_length_seconds: f32,
    pub day_of_year: u32,
    pub latitude_degrees: f32,
    pub axial_tilt_degrees: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FoliageData {
    pub enabled: bool,
    pub prefab: String,
    pub seed: u64,
    pub grid_min: i32,
    pub grid_max: i32,
    pub spacing: f32,
    pub jitter: f32,
    pub gate_threshold: f32,
    pub max_count: u32,
    pub min_scale: f32,
    pub max_scale: f32,
    pub min_player_distance: f32,
    pub edge_margin: f32,
    pub surface_offset: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MissionDefaultsData {
    pub pickup_radius: f32,
    pub pickup_scale: [f32; 3],
    pub target_health: f32,
    pub target_scale: [f32; 3],
    pub hazard_radius: f32,
    pub hazard_scale: [f32; 3],
    pub goal_radius: f32,
    pub goal_scale: [f32; 3],
}
