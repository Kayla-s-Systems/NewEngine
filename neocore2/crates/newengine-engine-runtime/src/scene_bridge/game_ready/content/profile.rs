use newengine_math::Vec3;

pub(in crate::scene_bridge::game_ready) type ColorRgba = [f32; 4];
pub(in crate::scene_bridge::game_ready) type ColorRgb = [f32; 3];

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyMapProfile {
    pub(in crate::scene_bridge::game_ready) title: String,
    pub(in crate::scene_bridge::game_ready) objective: String,
    pub(in crate::scene_bridge::game_ready) player: GameReadyPlayerSpec,
    pub(in crate::scene_bridge::game_ready) terrain: GameReadyTerrainSpec,
    pub(in crate::scene_bridge::game_ready) sky: GameReadySkySpec,
    pub(in crate::scene_bridge::game_ready) materials: GameReadyMaterialSetSpec,
    pub(in crate::scene_bridge::game_ready) lighting: GameReadyLightingSpec,
    pub(in crate::scene_bridge::game_ready) foliage: GameReadyFoliageSpec,
    pub(in crate::scene_bridge::game_ready) prefabs: Vec<GameReadyPrefabSpec>,
    pub(in crate::scene_bridge::game_ready) gameplay: GameReadyGameplaySpec,
    pub(in crate::scene_bridge::game_ready) palette: GameReadyPaletteSpec,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyPlayerSpec {
    pub(in crate::scene_bridge::game_ready) start: Vec3,
    pub(in crate::scene_bridge::game_ready) yaw: f32,
    pub(in crate::scene_bridge::game_ready) move_speed: f32,
    pub(in crate::scene_bridge::game_ready) look_sens: f32,
    pub(in crate::scene_bridge::game_ready) model: GameReadyPlayerModelSpec,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyPlayerModelSpec {
    pub(in crate::scene_bridge::game_ready) enabled: bool,
    pub(in crate::scene_bridge::game_ready) source: String,
    pub(in crate::scene_bridge::game_ready) texture_dictionary: Option<String>,
    pub(in crate::scene_bridge::game_ready) skeleton: Option<String>,
    pub(in crate::scene_bridge::game_ready) target_height: f32,
    pub(in crate::scene_bridge::game_ready) eye_height_ratio: f32,
    pub(in crate::scene_bridge::game_ready) local_offset: Vec3,
    pub(in crate::scene_bridge::game_ready) yaw_offset: f32,
    pub(in crate::scene_bridge::game_ready) hide_in_first_person: bool,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyTerrainSpec {
    pub(in crate::scene_bridge::game_ready) seed: u64,
    pub(in crate::scene_bridge::game_ready) cells_x: u32,
    pub(in crate::scene_bridge::game_ready) cells_z: u32,
    pub(in crate::scene_bridge::game_ready) size_x: f32,
    pub(in crate::scene_bridge::game_ready) size_z: f32,
    pub(in crate::scene_bridge::game_ready) base_height: f32,
    pub(in crate::scene_bridge::game_ready) height_scale: f32,
    pub(in crate::scene_bridge::game_ready) generator: GameReadyTerrainGeneratorSpec,
    pub(in crate::scene_bridge::game_ready) surface: GameReadyTerrainSurfaceSpec,
    pub(in crate::scene_bridge::game_ready) streaming: GameReadyTerrainStreamingSpec,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyTerrainSurfaceSpec {
    pub(in crate::scene_bridge::game_ready) forest_base_texture: String,
    pub(in crate::scene_bridge::game_ready) sand_base_texture: String,
    pub(in crate::scene_bridge::game_ready) rock_base_texture: String,
    pub(in crate::scene_bridge::game_ready) patch_scale: f32,
    pub(in crate::scene_bridge::game_ready) blend_softness: f32,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyTerrainStreamingSpec {
    pub(in crate::scene_bridge::game_ready) enabled: bool,
    pub(in crate::scene_bridge::game_ready) chunk_radius: i32,
    pub(in crate::scene_bridge::game_ready) unload_radius: i32,
    pub(in crate::scene_bridge::game_ready) max_chunks_per_frame: usize,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyTerrainGeneratorSpec {
    pub(in crate::scene_bridge::game_ready) id: String,
    pub(in crate::scene_bridge::game_ready) ridged_seed_xor: u64,
    pub(in crate::scene_bridge::game_ready) ridged_frequency: f32,
    pub(in crate::scene_bridge::game_ready) ridged_amplitude: f32,
    pub(in crate::scene_bridge::game_ready) ridged_shape_edge0: f32,
    pub(in crate::scene_bridge::game_ready) ridged_shape_edge1: f32,
    pub(in crate::scene_bridge::game_ready) veins_seed_xor: u64,
    pub(in crate::scene_bridge::game_ready) veins_frequency: f32,
    pub(in crate::scene_bridge::game_ready) veins_amplitude: f32,
    pub(in crate::scene_bridge::game_ready) smoothing_passes: u32,
    pub(in crate::scene_bridge::game_ready) smoothing_strength: f32,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadySkySpec {
    pub(in crate::scene_bridge::game_ready) radius: f32,
    pub(in crate::scene_bridge::game_ready) mesh: String,
    pub(in crate::scene_bridge::game_ready) follow_camera: bool,
    pub(in crate::scene_bridge::game_ready) cloud_dictionary: String,
    pub(in crate::scene_bridge::game_ready) cloud_profile: String,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyPaletteSpec {
    pub(in crate::scene_bridge::game_ready) terrain: ColorRgba,
    pub(in crate::scene_bridge::game_ready) sky: ColorRgba,
    pub(in crate::scene_bridge::game_ready) sky_emissive: ColorRgb,
    pub(in crate::scene_bridge::game_ready) tree_bark: ColorRgba,
    pub(in crate::scene_bridge::game_ready) tree_leaf: ColorRgba,
    pub(in crate::scene_bridge::game_ready) tree_branch: ColorRgba,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyMaterialSetSpec {
    pub(in crate::scene_bridge::game_ready) terrain: GameReadyMaterialSpec,
    pub(in crate::scene_bridge::game_ready) sky: GameReadyMaterialSpec,
    pub(in crate::scene_bridge::game_ready) tree_bark: GameReadyMaterialSpec,
    pub(in crate::scene_bridge::game_ready) tree_leaf: GameReadyMaterialSpec,
    pub(in crate::scene_bridge::game_ready) tree_branch: GameReadyMaterialSpec,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyMaterialSpec {
    pub(in crate::scene_bridge::game_ready) asset: Option<String>,
    pub(in crate::scene_bridge::game_ready) base_color_texture: Option<String>,
    pub(in crate::scene_bridge::game_ready) normal_texture: Option<String>,
    pub(in crate::scene_bridge::game_ready) roughness_texture: Option<String>,
    pub(in crate::scene_bridge::game_ready) uv_scale: [f32; 2],
    pub(in crate::scene_bridge::game_ready) uv_offset: [f32; 2],
    pub(in crate::scene_bridge::game_ready) roughness: f32,
    pub(in crate::scene_bridge::game_ready) normal_scale: f32,
    pub(in crate::scene_bridge::game_ready) occlusion_strength: f32,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyLightingSpec {
    pub(in crate::scene_bridge::game_ready) ambient_color: ColorRgb,
    pub(in crate::scene_bridge::game_ready) ambient_intensity: f32,
    pub(in crate::scene_bridge::game_ready) sun_direction: ColorRgb,
    pub(in crate::scene_bridge::game_ready) sun_color: ColorRgb,
    pub(in crate::scene_bridge::game_ready) sun_intensity: f32,
    pub(in crate::scene_bridge::game_ready) shadows: GameReadyShadowSpec,
    pub(in crate::scene_bridge::game_ready) day_night: GameReadyDayNightSpec,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyDayNightSpec {
    pub(in crate::scene_bridge::game_ready) enabled: bool,
    pub(in crate::scene_bridge::game_ready) time_of_day_hours: f32,
    pub(in crate::scene_bridge::game_ready) day_length_seconds: f32,
    pub(in crate::scene_bridge::game_ready) latitude_degrees: f32,
    pub(in crate::scene_bridge::game_ready) axial_tilt_degrees: f32,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyShadowSpec {
    pub(in crate::scene_bridge::game_ready) enabled: bool,
    pub(in crate::scene_bridge::game_ready) resolution: u32,
    pub(in crate::scene_bridge::game_ready) cascade_count: u32,
    pub(in crate::scene_bridge::game_ready) max_distance: f32,
    pub(in crate::scene_bridge::game_ready) softness: f32,
    pub(in crate::scene_bridge::game_ready) bias: f32,
    pub(in crate::scene_bridge::game_ready) normal_bias: f32,
    pub(in crate::scene_bridge::game_ready) contact_strength: f32,
}


#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyFoliageSpec {
    pub(in crate::scene_bridge::game_ready) enabled: bool,
    pub(in crate::scene_bridge::game_ready) prefab: String,
    pub(in crate::scene_bridge::game_ready) seed: u64,
    pub(in crate::scene_bridge::game_ready) grid_min: i32,
    pub(in crate::scene_bridge::game_ready) grid_max: i32,
    pub(in crate::scene_bridge::game_ready) spacing: f32,
    pub(in crate::scene_bridge::game_ready) jitter: f32,
    pub(in crate::scene_bridge::game_ready) gate_threshold: f32,
    pub(in crate::scene_bridge::game_ready) max_count: u32,
    pub(in crate::scene_bridge::game_ready) min_scale: f32,
    pub(in crate::scene_bridge::game_ready) max_scale: f32,
    pub(in crate::scene_bridge::game_ready) min_player_distance: f32,
    pub(in crate::scene_bridge::game_ready) edge_margin: f32,
    pub(in crate::scene_bridge::game_ready) surface_offset: f32,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyPrefabSpec {
    pub(in crate::scene_bridge::game_ready) id: String,
    pub(in crate::scene_bridge::game_ready) source: String,
    pub(in crate::scene_bridge::game_ready) proxy: String,
    pub(in crate::scene_bridge::game_ready) enabled: bool,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyGameplaySpec {
    pub(in crate::scene_bridge::game_ready) default_status: String,
    pub(in crate::scene_bridge::game_ready) pickup_status: String,
    pub(in crate::scene_bridge::game_ready) hazard_status: String,
    pub(in crate::scene_bridge::game_ready) goal_locked_status: String,
    pub(in crate::scene_bridge::game_ready) goal_complete_status: String,
    pub(in crate::scene_bridge::game_ready) failed_progress_label: String,
    pub(in crate::scene_bridge::game_ready) completed_progress_label: String,
    pub(in crate::scene_bridge::game_ready) player_collision: GameReadyPlayerCollisionSpec,
    pub(in crate::scene_bridge::game_ready) player_visual: GameReadyPlayerVisualSpec,
    pub(in crate::scene_bridge::game_ready) physics: GameReadyPhysicsSpec,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyPlayerCollisionSpec {
    pub(in crate::scene_bridge::game_ready) radius: f32,
    pub(in crate::scene_bridge::game_ready) half_height: f32,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyPlayerVisualSpec {
    pub(in crate::scene_bridge::game_ready) radius: f32,
    pub(in crate::scene_bridge::game_ready) half_height: f32,
    pub(in crate::scene_bridge::game_ready) camera_eye_height: f32,
    pub(in crate::scene_bridge::game_ready) sprint_multiplier: f32,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyPhysicsSpec {
    pub(in crate::scene_bridge::game_ready) gravity: f32,
    pub(in crate::scene_bridge::game_ready) contact_skin: f32,
}

