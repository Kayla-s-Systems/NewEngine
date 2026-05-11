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
    pub(in crate::scene_bridge::game_ready) gameplay: GameReadyGameplaySpec,
    pub(in crate::scene_bridge::game_ready) palette: GameReadyPaletteSpec,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyPlayerSpec {
    pub(in crate::scene_bridge::game_ready) start: Vec3,
    pub(in crate::scene_bridge::game_ready) yaw: f32,
    pub(in crate::scene_bridge::game_ready) move_speed: f32,
    pub(in crate::scene_bridge::game_ready) look_sens: f32,
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
    pub(in crate::scene_bridge::game_ready) collision_tile_cells: u32,
    pub(in crate::scene_bridge::game_ready) collision_floor_depth: f32,
    pub(in crate::scene_bridge::game_ready) collision_horizontal_skin: f32,
    pub(in crate::scene_bridge::game_ready) generator: GameReadyTerrainGeneratorSpec,
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
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadySkySpec {
    pub(in crate::scene_bridge::game_ready) radius: f32,
    pub(in crate::scene_bridge::game_ready) mesh: String,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyPaletteSpec {
    pub(in crate::scene_bridge::game_ready) terrain: ColorRgba,
    pub(in crate::scene_bridge::game_ready) sky: ColorRgba,
    pub(in crate::scene_bridge::game_ready) sky_emissive: ColorRgb,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyMaterialSetSpec {
    pub(in crate::scene_bridge::game_ready) terrain: GameReadyMaterialSpec,
    pub(in crate::scene_bridge::game_ready) sky: GameReadyMaterialSpec,
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

