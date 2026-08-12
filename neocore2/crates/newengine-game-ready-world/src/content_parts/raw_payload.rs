use newengine_math::Vec3;
use serde::Deserialize;

#[path = "profile_parse.rs"]
mod profile_parse;
#[path = "raw_payload_defaults.rs"]
mod raw_payload_defaults;
#[path = "sanitize_defaults.rs"]
mod sanitize_defaults;
#[path = "ymap_read_diagnostics.rs"]
mod ymap_read_diagnostics;

pub(crate) use self::profile_parse::load_game_ready_map_profile;

use self::profile_parse::*;
use self::sanitize_defaults::*;
use super::profile::*;

#[derive(Debug, Deserialize)]
struct RawGameReadyPayload {
    #[serde(default = "default_title")]
    pub(super) title: String,
    #[serde(default = "default_objective")]
    pub(super) objective: String,
    #[serde(default)]
    pub(super) player: RawPlayerSpec,
    #[serde(default)]
    pub(super) terrain: RawTerrainSpec,
    #[serde(default)]
    pub(super) sky: RawSkySpec,
    #[serde(default)]
    pub(super) materials: RawMaterialSetSpec,
    #[serde(default)]
    pub(super) lighting: RawLightingSpec,
    #[serde(default)]
    pub(super) foliage: RawFoliageSpec,
    #[serde(default)]
    pub(super) prefabs: Vec<RawPrefabSpec>,
    #[serde(default)]
    pub(super) definitions: Vec<RawDefinitionInstanceSpec>,
    #[serde(default)]
    pub(super) gameplay: RawGameplaySpec,
    #[serde(default)]
    pub(super) palette: RawPaletteSpec,
}

#[derive(Debug, Deserialize)]
struct RawPlayerSpec {
    #[serde(default = "default_player_start")]
    pub(super) start: [f32; 3],
    #[serde(default = "default_player_yaw")]
    pub(super) yaw: f32,
    #[serde(default = "default_move_speed")]
    pub(super) move_speed: f32,
    #[serde(default = "default_look_sens")]
    pub(super) look_sens: f32,
    #[serde(default)]
    pub(super) model: RawPlayerModelSpec,
}

#[derive(Debug, Deserialize)]
struct RawPlayerModelSpec {
    #[serde(default = "default_player_model_enabled")]
    pub(super) enabled: bool,
    #[serde(default = "default_player_model_source")]
    pub(super) source: String,
    #[serde(default = "default_player_model_properties_ref")]
    pub(super) properties_ref: Option<String>,
    #[serde(default = "default_player_texture_dictionary")]
    pub(super) texture_dictionary: Option<String>,
    #[serde(default = "default_player_skeleton")]
    pub(super) skeleton: Option<String>,
    #[serde(default = "default_player_model_height")]
    pub(super) target_height: f32,
    #[serde(default = "default_player_model_eye_height_ratio")]
    pub(super) eye_height_ratio: f32,
    #[serde(default = "default_player_model_offset")]
    pub(super) local_offset: [f32; 3],
    #[serde(default = "default_player_model_yaw_offset")]
    pub(super) yaw_offset: f32,
    #[serde(default = "default_player_model_hide_in_first_person")]
    pub(super) hide_in_first_person: bool,
}

#[derive(Debug, Deserialize)]
struct RawTerrainSpec {
    #[serde(default = "default_terrain_enabled")]
    pub(super) enabled: bool,
    #[serde(default = "default_terrain_seed")]
    pub(super) seed: u64,
    #[serde(default = "default_terrain_cells")]
    pub(super) cells_x: u32,
    #[serde(default = "default_terrain_cells")]
    pub(super) cells_z: u32,
    #[serde(default = "default_terrain_size")]
    pub(super) size_x: f32,
    #[serde(default = "default_terrain_size")]
    pub(super) size_z: f32,
    #[serde(default = "default_base_height")]
    pub(super) base_height: f32,
    #[serde(default = "default_height_scale")]
    pub(super) height_scale: f32,
    #[serde(default)]
    pub(super) generator: RawTerrainGeneratorSpec,
    #[serde(default)]
    pub(super) surface: RawTerrainSurfaceSpec,
    #[serde(default)]
    pub(super) heightmap: RawTerrainHeightmapSpec,
    #[serde(default)]
    pub(super) streaming: RawTerrainStreamingSpec,
}

#[derive(Debug, Deserialize)]
struct RawTerrainSurfaceSpec {
    #[serde(default = "default_terrain_surface_forest")]
    pub(super) forest_base_texture: String,
    #[serde(default = "default_terrain_surface_sand")]
    pub(super) sand_base_texture: String,
    #[serde(default = "default_terrain_surface_rock")]
    pub(super) rock_base_texture: String,
    #[serde(default = "default_terrain_patch_scale")]
    pub(super) patch_scale: f32,
    #[serde(default = "default_terrain_blend_softness")]
    pub(super) blend_softness: f32,
    #[serde(default)]
    pub(super) layers: Vec<RawTerrainSurfaceLayerSpec>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawTerrainSurfaceLayerSpec {
    #[serde(default)]
    pub(super) role: String,
    #[serde(default)]
    pub(super) base_texture: String,
    #[serde(default)]
    pub(super) texture: String,
    #[serde(default = "default_terrain_surface_layer_weight")]
    pub(super) weight: f32,
    #[serde(default = "default_terrain_surface_layer_uv_scale")]
    pub(super) uv_scale: f32,
}

#[derive(Debug, Deserialize)]
struct RawTerrainHeightmapSpec {
    #[serde(default)]
    pub(super) enabled: bool,
    #[serde(default)]
    pub(super) source: String,
    #[serde(default = "default_terrain_heightmap_mode")]
    pub(super) mode: String,
    #[serde(default = "default_terrain_heightmap_strength")]
    pub(super) strength: f32,
    #[serde(default = "default_terrain_heightmap_min_height")]
    pub(super) min_height: f32,
    #[serde(default = "default_terrain_heightmap_max_height")]
    pub(super) max_height: f32,
    #[serde(default = "default_terrain_heightmap_tile_scale")]
    pub(super) tile_scale: [f32; 2],
    #[serde(default = "default_terrain_heightmap_tile_offset")]
    pub(super) tile_offset: [f32; 2],
    #[serde(default)]
    pub(super) invert: bool,
}

#[derive(Debug, Deserialize)]
struct RawTerrainStreamingSpec {
    #[serde(default = "default_terrain_streaming_enabled")]
    pub(super) enabled: bool,
    #[serde(default = "default_terrain_chunk_radius")]
    pub(super) chunk_radius: i32,
    #[serde(default = "default_terrain_unload_radius")]
    pub(super) unload_radius: i32,
    #[serde(default = "default_terrain_max_chunks_per_frame")]
    pub(super) max_chunks_per_frame: usize,
}

#[derive(Debug, Deserialize)]
struct RawTerrainGeneratorSpec {
    #[serde(default = "default_terrain_generator_id")]
    pub(super) id: String,
    #[serde(default = "default_ridged_seed_xor")]
    pub(super) ridged_seed_xor: u64,
    #[serde(default = "default_ridged_frequency")]
    pub(super) ridged_frequency: f32,
    #[serde(default = "default_ridged_amplitude")]
    pub(super) ridged_amplitude: f32,
    #[serde(default = "default_ridged_shape_edge0")]
    pub(super) ridged_shape_edge0: f32,
    #[serde(default = "default_ridged_shape_edge1")]
    pub(super) ridged_shape_edge1: f32,
    #[serde(default = "default_veins_seed_xor")]
    pub(super) veins_seed_xor: u64,
    #[serde(default = "default_veins_frequency")]
    pub(super) veins_frequency: f32,
    #[serde(default = "default_veins_amplitude")]
    pub(super) veins_amplitude: f32,
    #[serde(default = "default_smoothing_passes")]
    pub(super) smoothing_passes: u32,
    #[serde(default = "default_smoothing_strength")]
    pub(super) smoothing_strength: f32,
}

#[derive(Debug, Deserialize)]
struct RawSkySpec {
    #[serde(default = "default_sky_definition_ref")]
    pub(super) definition_ref: String,
    #[serde(default = "default_sky_radius")]
    pub(super) radius: f32,
    #[serde(default = "default_skydome_mesh")]
    pub(super) mesh: String,
    #[serde(default = "default_sky_follow_camera")]
    pub(super) follow_camera: bool,
    #[serde(default = "default_cloud_dictionary")]
    pub(super) cloud_dictionary: String,
    #[serde(default = "default_cloud_profile")]
    pub(super) cloud_profile: String,
    #[serde(default = "default_sky_sun_radius")]
    pub(super) sun_radius: f32,
    #[serde(default = "default_sky_moon_radius")]
    pub(super) moon_radius: f32,
    #[serde(default = "default_moon_texture")]
    pub(super) moon_texture: String,
    #[serde(default)]
    pub(super) atmosphere: RawSkyAtmosphereSpec,
}

#[derive(Debug, Deserialize)]
struct RawSkyAtmosphereSpec {
    #[serde(default = "default_sky_day_zenith")]
    pub(super) day_zenith: ColorRgb,
    #[serde(default = "default_sky_day_horizon")]
    pub(super) day_horizon: ColorRgb,
    #[serde(default = "default_sky_dusk_zenith")]
    pub(super) dusk_zenith: ColorRgb,
    #[serde(default = "default_sky_dusk_horizon")]
    pub(super) dusk_horizon: ColorRgb,
    #[serde(default = "default_sky_night_zenith")]
    pub(super) night_zenith: ColorRgb,
    #[serde(default = "default_sky_night_horizon")]
    pub(super) night_horizon: ColorRgb,
    #[serde(default = "default_sky_cloud_day")]
    pub(super) cloud_day: ColorRgb,
    #[serde(default = "default_sky_cloud_night")]
    pub(super) cloud_night: ColorRgb,
    #[serde(default = "default_sky_night_strength")]
    pub(super) night_sky_strength: f32,
    #[serde(default = "default_sky_cloud_coverage")]
    pub(super) cloud_coverage: f32,
    #[serde(default = "default_sky_cloud_softness")]
    pub(super) cloud_softness: f32,
}

#[derive(Debug, Deserialize)]
struct RawGameplaySpec {
    #[serde(default = "default_status_text")]
    pub(super) default_status: String,
    #[serde(default = "default_pickup_status")]
    pub(super) pickup_status: String,
    #[serde(default = "default_target_status")]
    pub(super) target_status: String,
    #[serde(default = "default_hazard_status")]
    pub(super) hazard_status: String,
    #[serde(default = "default_goal_locked_status")]
    pub(super) goal_locked_status: String,
    #[serde(default = "default_goal_complete_status")]
    pub(super) goal_complete_status: String,
    #[serde(default = "default_failed_progress_label")]
    pub(super) failed_progress_label: String,
    #[serde(default = "default_completed_progress_label")]
    pub(super) completed_progress_label: String,
    #[serde(default)]
    pub(super) player_collision: RawPlayerCollisionSpec,
    #[serde(default)]
    pub(super) player_visual: RawPlayerVisualSpec,
    #[serde(default)]
    pub(super) physics: RawPhysicsSpec,
    #[serde(default)]
    pub(super) mission: RawMissionSpec,
}

#[derive(Debug, Default, Deserialize)]
struct RawMissionSpec {
    #[serde(default)]
    pub(super) pickups: Vec<RawMissionPickupSpec>,
    #[serde(default)]
    pub(super) targets: Vec<RawMissionTargetSpec>,
    #[serde(default)]
    pub(super) hazards: Vec<RawMissionHazardSpec>,
    #[serde(default)]
    pub(super) goals: Vec<RawMissionGoalSpec>,
}

#[derive(Debug, Deserialize)]
struct RawMissionPickupSpec {
    #[serde(default)]
    pub(super) id: String,
    #[serde(default)]
    pub(super) position: [f32; 3],
    #[serde(default)]
    pub(super) radius: f32,
    #[serde(default)]
    pub(super) scale: [f32; 3],
}

#[derive(Debug, Deserialize)]
struct RawMissionTargetSpec {
    #[serde(default)]
    pub(super) id: String,
    #[serde(default)]
    pub(super) position: [f32; 3],
    #[serde(default)]
    pub(super) health: f32,
    #[serde(default)]
    pub(super) scale: [f32; 3],
}

#[derive(Debug, Deserialize)]
struct RawMissionHazardSpec {
    #[serde(default)]
    pub(super) id: String,
    #[serde(default)]
    pub(super) position: [f32; 3],
    #[serde(default)]
    pub(super) radius: f32,
    #[serde(default)]
    pub(super) scale: [f32; 3],
}

#[derive(Debug, Deserialize)]
struct RawMissionGoalSpec {
    #[serde(default)]
    pub(super) id: String,
    #[serde(default)]
    pub(super) position: [f32; 3],
    #[serde(default)]
    pub(super) radius: f32,
    #[serde(default)]
    pub(super) scale: [f32; 3],
}

#[derive(Debug, Deserialize)]
struct RawPlayerCollisionSpec {
    #[serde(default = "default_player_body_radius")]
    pub(super) radius: f32,
    #[serde(default = "default_player_body_half_height")]
    pub(super) half_height: f32,
}

#[derive(Debug, Deserialize)]
struct RawPlayerVisualSpec {
    #[serde(default = "default_player_visual_radius")]
    pub(super) radius: f32,
    #[serde(default = "default_player_visual_half_height")]
    pub(super) half_height: f32,
    #[serde(default = "default_camera_eye_height")]
    pub(super) camera_eye_height: f32,
    #[serde(default = "default_sprint_multiplier")]
    pub(super) sprint_multiplier: f32,
}

#[derive(Debug, Deserialize)]
struct RawPhysicsSpec {
    #[serde(default = "default_gravity")]
    pub(super) gravity: f32,
    #[serde(default = "default_contact_skin")]
    pub(super) contact_skin: f32,
}

#[derive(Debug, Deserialize)]
struct RawPaletteSpec {
    #[serde(default = "default_terrain_color")]
    pub(super) terrain: ColorRgba,
    #[serde(default = "default_sky_color")]
    pub(super) sky: ColorRgba,
    #[serde(default = "default_sky_emissive")]
    pub(super) sky_emissive: ColorRgb,
    #[serde(default = "default_tree_bark_color")]
    pub(super) tree_bark: ColorRgba,
    #[serde(default = "default_tree_leaf_color")]
    pub(super) tree_leaf: ColorRgba,
    #[serde(default = "default_tree_branch_color")]
    pub(super) tree_branch: ColorRgba,
}

#[derive(Debug, Deserialize)]
struct RawMaterialSetSpec {
    #[serde(default = "default_terrain_material")]
    pub(super) terrain: RawMaterialSpec,
    #[serde(default = "default_sky_material")]
    pub(super) sky: RawMaterialSpec,
    #[serde(default = "default_sun_material")]
    pub(super) sun: RawMaterialSpec,
    #[serde(default = "default_moon_material")]
    pub(super) moon: RawMaterialSpec,
    #[serde(default = "default_tree_bark_material")]
    pub(super) tree_bark: RawMaterialSpec,
    #[serde(default = "default_tree_leaf_material")]
    pub(super) tree_leaf: RawMaterialSpec,
    #[serde(default = "default_tree_branch_material")]
    pub(super) tree_branch: RawMaterialSpec,
}

#[derive(Debug, Clone, Deserialize)]
struct RawMaterialSpec {
    #[serde(default)]
    pub(super) asset: Option<String>,
    #[serde(default)]
    pub(super) base_color_texture: Option<String>,
    #[serde(default)]
    pub(super) normal_texture: Option<String>,
    #[serde(default)]
    pub(super) roughness_texture: Option<String>,
    #[serde(default = "default_uv_scale")]
    pub(super) uv_scale: [f32; 2],
    #[serde(default = "default_uv_offset")]
    pub(super) uv_offset: [f32; 2],
    #[serde(default = "default_material_roughness")]
    pub(super) roughness: f32,
    #[serde(default = "default_material_normal_scale")]
    pub(super) normal_scale: f32,
    #[serde(default = "default_material_occlusion_strength")]
    pub(super) occlusion_strength: f32,
}

#[derive(Debug, Deserialize)]
struct RawLightingSpec {
    #[serde(default = "default_ambient_color")]
    pub(super) ambient_color: ColorRgb,
    #[serde(default = "default_ambient_intensity")]
    pub(super) ambient_intensity: f32,
    #[serde(default = "default_sun_direction")]
    pub(super) sun_direction: ColorRgb,
    #[serde(default = "default_sun_color")]
    pub(super) sun_color: ColorRgb,
    #[serde(default = "default_sun_intensity")]
    pub(super) sun_intensity: f32,
    #[serde(default)]
    pub(super) shadows: RawShadowSpec,
    #[serde(default)]
    pub(super) day_night: RawDayNightSpec,
}

#[derive(Debug, Deserialize)]
struct RawDayNightSpec {
    #[serde(default = "default_day_night_enabled")]
    pub(super) enabled: bool,
    #[serde(default = "default_time_of_day_hours")]
    pub(super) time_of_day_hours: f32,
    #[serde(default = "default_day_length_seconds")]
    pub(super) day_length_seconds: f32,
    #[serde(default = "default_day_of_year")]
    pub(super) day_of_year: u32,
    #[serde(default = "default_sun_latitude_degrees")]
    pub(super) latitude_degrees: f32,
    #[serde(default = "default_axial_tilt_degrees")]
    pub(super) axial_tilt_degrees: f32,
}
