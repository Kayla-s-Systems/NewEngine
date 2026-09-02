#![forbid(unsafe_op_in_unsafe_fn)]

//! Project-authored world-environment DTOs shared by parsers and runtime providers.
//! These contracts contain no runtime-profile or product identity.

pub type ColorRgba = [f32; 4];
pub type ColorRgb = [f32; 3];

#[derive(Clone, Debug)]
pub struct AuthoredTerrainSpec {
    pub enabled: bool,
    pub seed: u64,
    pub cells_x: u32,
    pub cells_z: u32,
    pub size_x: f32,
    pub size_z: f32,
    pub base_height: f32,
    pub height_scale: f32,
    pub render_options: newengine_model_domain_api::MeshRenderOptions,
    pub generator: AuthoredTerrainGeneratorSpec,
    pub surface: AuthoredTerrainSurfaceSpec,
    pub heightmap: AuthoredTerrainHeightmapSpec,
    pub streaming: AuthoredTerrainStreamingSpec,
}

#[derive(Clone, Debug)]
pub struct AuthoredTerrainSurfaceSpec {
    pub forest_base_texture: String,
    pub sand_base_texture: String,
    pub rock_base_texture: String,
    pub patch_scale: f32,
    pub blend_softness: f32,
    pub layers: Vec<AuthoredTerrainSurfaceLayerSpec>,
}

#[derive(Clone, Debug)]
pub struct AuthoredTerrainSurfaceLayerSpec {
    pub role: String,
    pub base_texture: String,
    pub weight: f32,
    pub uv_scale: f32,
}

#[derive(Clone, Debug)]
pub struct AuthoredTerrainHeightmapSpec {
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

#[derive(Clone, Debug)]
pub struct AuthoredTerrainStreamingSpec {
    pub enabled: bool,
    pub chunk_radius: i32,
    pub unload_radius: i32,
    pub max_chunks_per_frame: usize,
}

#[derive(Clone, Debug)]
pub struct AuthoredTerrainGeneratorSpec {
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

#[derive(Clone, Debug)]
pub struct AuthoredSkySpec {
    pub definition_ref: String,
    pub render_options: newengine_model_domain_api::MeshRenderOptions,
    pub radius: f32,
    pub mesh: String,
    pub follow_camera: bool,
    pub environment_profile: String,
    pub environment_region: String,
    pub environment_biome: String,
    pub cloud_dictionary: String,
    pub cloud_profile: String,
    pub sun_radius: f32,
    pub moon_radius: f32,
    pub moon_texture: String,
    pub atmosphere: AuthoredSkyAtmosphereSpec,
}

#[derive(Clone, Debug)]
pub struct AuthoredSkyAtmosphereSpec {
    pub day_zenith: ColorRgb,
    pub day_horizon: ColorRgb,
    pub dusk_zenith: ColorRgb,
    pub dusk_horizon: ColorRgb,
    pub night_zenith: ColorRgb,
    pub night_horizon: ColorRgb,
    pub cloud_day: ColorRgb,
    pub cloud_night: ColorRgb,
    pub night_sky_strength: f32,
    pub cloud_coverage: f32,
    pub cloud_softness: f32,
}

#[derive(Clone, Debug)]
pub struct AuthoredEnvironmentPaletteSpec {
    pub terrain: ColorRgba,
    pub sky: ColorRgba,
    pub sky_emissive: ColorRgb,
    pub tree_bark: ColorRgba,
    pub tree_leaf: ColorRgba,
    pub tree_branch: ColorRgba,
}

#[derive(Clone, Debug)]
pub struct AuthoredEnvironmentMaterialSetSpec {
    pub terrain: newengine_material_domain_api::AuthoredMaterialSpec,
    pub sky: newengine_material_domain_api::AuthoredMaterialSpec,
    pub sun: newengine_material_domain_api::AuthoredMaterialSpec,
    pub moon: newengine_material_domain_api::AuthoredMaterialSpec,
    pub tree_bark: newengine_material_domain_api::AuthoredMaterialSpec,
    pub tree_leaf: newengine_material_domain_api::AuthoredMaterialSpec,
    pub tree_branch: newengine_material_domain_api::AuthoredMaterialSpec,
}

#[derive(Clone, Debug)]
pub struct AuthoredLightingSpec {
    pub ambient_color: ColorRgb,
    pub ambient_intensity: f32,
    pub sun_direction: ColorRgb,
    pub sun_color: ColorRgb,
    pub sun_intensity: f32,
    pub shadows: AuthoredShadowSpec,
    pub day_night: AuthoredDayNightSpec,
}

#[derive(Clone, Debug)]
pub struct AuthoredDayNightSpec {
    pub enabled: bool,
    pub time_of_day_hours: f32,
    pub day_length_seconds: f32,
    pub day_of_year: u32,
    pub latitude_degrees: f32,
    pub axial_tilt_degrees: f32,
}

#[derive(Clone, Debug)]
pub struct AuthoredShadowSpec {
    pub enabled: bool,
    pub resolution: u32,
    pub cascade_count: u32,
    pub max_distance: f32,
    pub softness: f32,
    pub bias: f32,
    pub normal_bias: f32,
    pub contact_strength: f32,
    pub filter: newengine_lighting::ShadowFilter,
    pub pcss: newengine_lighting::ShadowPcssSettings,
}

#[derive(Clone, Debug)]
pub struct AuthoredFoliageSpec {
    pub enabled: bool,
    pub settings: newengine_model_domain_api::FoliageSettings,
    pub prefab: String,
    pub alternate_prefab: String,
    pub alternate_canonical_path: String,
    pub alternate_weight: f32,
    pub alternate_collision_radius: f32,
    pub alternate_collision_half_height: f32,
    pub alternate_collision_center: newengine_math::Vec3,
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
    pub collision_enabled: bool,
    pub collision_radius: f32,
    pub collision_half_height: f32,
    pub collision_center: newengine_math::Vec3,
    pub render_options: newengine_model_domain_api::MeshRenderOptions,
}
