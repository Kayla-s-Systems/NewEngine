use newengine_math::Vec3;
use newengine_model_domain_api::MeshRenderOptions;

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
    pub(in crate::scene_bridge::game_ready) definitions: Vec<GameReadyDefinitionInstanceSpec>,
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
    pub(in crate::scene_bridge::game_ready) properties_ref: Option<String>,
    pub(in crate::scene_bridge::game_ready) texture_dictionary: Option<String>,
    pub(in crate::scene_bridge::game_ready) skeleton: Option<String>,
    pub(in crate::scene_bridge::game_ready) target_height: f32,
    pub(in crate::scene_bridge::game_ready) eye_height_ratio: f32,
    pub(in crate::scene_bridge::game_ready) local_offset: Vec3,
    pub(in crate::scene_bridge::game_ready) yaw_offset: f32,
    pub(in crate::scene_bridge::game_ready) hide_in_first_person: bool,
    pub(in crate::scene_bridge::game_ready) render_options: MeshRenderOptions,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyTerrainSpec {
    /// Whether the procedural terrain mesh/heightfield is instantiated. Maps with
    /// authored world meshes can disable it and use static mesh collision instead.
    pub(in crate::scene_bridge::game_ready) enabled: bool,
    pub(in crate::scene_bridge::game_ready) seed: u64,
    pub(in crate::scene_bridge::game_ready) cells_x: u32,
    pub(in crate::scene_bridge::game_ready) cells_z: u32,
    pub(in crate::scene_bridge::game_ready) size_x: f32,
    pub(in crate::scene_bridge::game_ready) size_z: f32,
    pub(in crate::scene_bridge::game_ready) base_height: f32,
    pub(in crate::scene_bridge::game_ready) height_scale: f32,
    pub(in crate::scene_bridge::game_ready) render_options: MeshRenderOptions,
    pub(in crate::scene_bridge::game_ready) generator: GameReadyTerrainGeneratorSpec,
    pub(in crate::scene_bridge::game_ready) surface: GameReadyTerrainSurfaceSpec,
    pub(in crate::scene_bridge::game_ready) heightmap: GameReadyTerrainHeightmapSpec,
    pub(in crate::scene_bridge::game_ready) streaming: GameReadyTerrainStreamingSpec,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyTerrainSurfaceSpec {
    pub(in crate::scene_bridge::game_ready) forest_base_texture: String,
    pub(in crate::scene_bridge::game_ready) sand_base_texture: String,
    pub(in crate::scene_bridge::game_ready) rock_base_texture: String,
    pub(in crate::scene_bridge::game_ready) patch_scale: f32,
    pub(in crate::scene_bridge::game_ready) blend_softness: f32,
    /// Declarative authoring projection for the terrain surface package.
    /// Runtime currently maps these roles onto the stable 3-channel terrain shader
    /// contract: forest/base, sand/path and rock/slope. The fixed fields above are
    /// the compatibility projection consumed by the renderer.
    pub(in crate::scene_bridge::game_ready) layers: Vec<GameReadyTerrainSurfaceLayerSpec>,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyTerrainSurfaceLayerSpec {
    pub(in crate::scene_bridge::game_ready) role: String,
    pub(in crate::scene_bridge::game_ready) base_texture: String,
    pub(in crate::scene_bridge::game_ready) weight: f32,
    pub(in crate::scene_bridge::game_ready) uv_scale: f32,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyTerrainHeightmapSpec {
    pub(in crate::scene_bridge::game_ready) enabled: bool,
    pub(in crate::scene_bridge::game_ready) source: String,
    pub(in crate::scene_bridge::game_ready) mode: String,
    pub(in crate::scene_bridge::game_ready) strength: f32,
    pub(in crate::scene_bridge::game_ready) min_height: f32,
    pub(in crate::scene_bridge::game_ready) max_height: f32,
    pub(in crate::scene_bridge::game_ready) tile_scale: [f32; 2],
    pub(in crate::scene_bridge::game_ready) tile_offset: [f32; 2],
    pub(in crate::scene_bridge::game_ready) invert: bool,
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
pub(in crate::scene_bridge) struct GameReadySkySpec {
    pub(in crate::scene_bridge::game_ready) definition_ref: String,
    pub(in crate::scene_bridge::game_ready) render_options: MeshRenderOptions,
    pub(in crate::scene_bridge::game_ready) radius: f32,
    pub(in crate::scene_bridge::game_ready) mesh: String,
    pub(in crate::scene_bridge::game_ready) follow_camera: bool,
    pub(in crate::scene_bridge::game_ready) cloud_dictionary: String,
    pub(in crate::scene_bridge::game_ready) cloud_profile: String,
    pub(in crate::scene_bridge::game_ready) sun_radius: f32,
    pub(in crate::scene_bridge::game_ready) moon_radius: f32,
    pub(in crate::scene_bridge::game_ready) moon_texture: String,
    pub(in crate::scene_bridge::game_ready) atmosphere: GameReadySkyAtmosphereSpec,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge) struct GameReadySkyAtmosphereSpec {
    pub(in crate::scene_bridge::game_ready) day_zenith: ColorRgb,
    pub(in crate::scene_bridge::game_ready) day_horizon: ColorRgb,
    pub(in crate::scene_bridge::game_ready) dusk_zenith: ColorRgb,
    pub(in crate::scene_bridge::game_ready) dusk_horizon: ColorRgb,
    pub(in crate::scene_bridge::game_ready) night_zenith: ColorRgb,
    pub(in crate::scene_bridge::game_ready) night_horizon: ColorRgb,
    pub(in crate::scene_bridge::game_ready) cloud_day: ColorRgb,
    pub(in crate::scene_bridge::game_ready) cloud_night: ColorRgb,
    pub(in crate::scene_bridge::game_ready) night_sky_strength: f32,
    pub(in crate::scene_bridge::game_ready) cloud_coverage: f32,
    pub(in crate::scene_bridge::game_ready) cloud_softness: f32,
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
    pub(in crate::scene_bridge::game_ready) sun: GameReadyMaterialSpec,
    pub(in crate::scene_bridge::game_ready) moon: GameReadyMaterialSpec,
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
    /// Seasonal day in the tropical year, 1..=366. It drives solar declination
    /// independently from the time-of-day clock.
    pub(in crate::scene_bridge::game_ready) day_of_year: u32,
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
    pub(in crate::scene_bridge::game_ready) render_options: MeshRenderOptions,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyPrefabSpec {
    pub(in crate::scene_bridge::game_ready) id: String,
    pub(in crate::scene_bridge::game_ready) source: String,
    pub(in crate::scene_bridge::game_ready) proxy: String,
    pub(in crate::scene_bridge::game_ready) material: String,
    pub(in crate::scene_bridge::game_ready) enabled: bool,
    pub(in crate::scene_bridge::game_ready) position: Vec3,
    pub(in crate::scene_bridge::game_ready) rotation_ypr: Vec3,
    pub(in crate::scene_bridge::game_ready) scale: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(in crate::scene_bridge::game_ready) enum GameReadyDefinitionApplyMode {
    /// Resolve metadata/dependency graph only. No ECS marker and no render packet
    /// are produced by the generic definition instantiator. Domain systems such
    /// as sky, terrain, foliage or player avatar consume the metadata explicitly.
    #[default]
    MetadataOnly,
    /// Spawn a lightweight diagnostic marker entity carrying DefinitionInstance
    /// and DefinitionRuntimeTrace. This is explicit because `.ytyp` dependencies
    /// are not render/spawn commands.
    InstantiateMarker,
}
impl GameReadyDefinitionApplyMode {
    pub(in crate::scene_bridge::game_ready) fn from_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "instantiate" | "instantiate_marker" | "marker" | "diagnostic_marker" => {
                Self::InstantiateMarker
            }
            _ => Self::MetadataOnly,
        }
    }

    pub(in crate::scene_bridge::game_ready) const fn as_str(self) -> &'static str {
        match self {
            Self::MetadataOnly => "metadata_only",
            Self::InstantiateMarker => "instantiate_marker",
        }
    }
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyDefinitionInstanceSpec {
    pub(in crate::scene_bridge::game_ready) definition_ref: String,
    pub(in crate::scene_bridge::game_ready) position: Vec3,
    pub(in crate::scene_bridge::game_ready) rotation_ypr: [f32; 3],
    pub(in crate::scene_bridge::game_ready) scale: Vec3,
    pub(in crate::scene_bridge::game_ready) apply_mode: GameReadyDefinitionApplyMode,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyGameplaySpec {
    pub(in crate::scene_bridge::game_ready) default_status: String,
    pub(in crate::scene_bridge::game_ready) pickup_status: String,
    pub(in crate::scene_bridge::game_ready) target_status: String,
    pub(in crate::scene_bridge::game_ready) hazard_status: String,
    pub(in crate::scene_bridge::game_ready) goal_locked_status: String,
    pub(in crate::scene_bridge::game_ready) goal_complete_status: String,
    pub(in crate::scene_bridge::game_ready) failed_progress_label: String,
    pub(in crate::scene_bridge::game_ready) completed_progress_label: String,
    pub(in crate::scene_bridge::game_ready) player_collision: GameReadyPlayerCollisionSpec,
    pub(in crate::scene_bridge::game_ready) player_visual: GameReadyPlayerVisualSpec,
    pub(in crate::scene_bridge::game_ready) physics: GameReadyPhysicsSpec,
    pub(in crate::scene_bridge::game_ready) mission: GameReadyMissionSpec,
}

#[derive(Clone, Debug, Default)]
pub(in crate::scene_bridge::game_ready) struct GameReadyMissionSpec {
    pub(in crate::scene_bridge::game_ready) pickups: Vec<GameReadyMissionPickupSpec>,
    pub(in crate::scene_bridge::game_ready) targets: Vec<GameReadyMissionTargetSpec>,
    pub(in crate::scene_bridge::game_ready) hazards: Vec<GameReadyMissionHazardSpec>,
    pub(in crate::scene_bridge::game_ready) goals: Vec<GameReadyMissionGoalSpec>,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyMissionPickupSpec {
    pub(in crate::scene_bridge::game_ready) id: String,
    pub(in crate::scene_bridge::game_ready) position: Vec3,
    pub(in crate::scene_bridge::game_ready) radius: f32,
    pub(in crate::scene_bridge::game_ready) scale: Vec3,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyMissionTargetSpec {
    pub(in crate::scene_bridge::game_ready) id: String,
    pub(in crate::scene_bridge::game_ready) position: Vec3,
    pub(in crate::scene_bridge::game_ready) health: f32,
    pub(in crate::scene_bridge::game_ready) scale: Vec3,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyMissionHazardSpec {
    pub(in crate::scene_bridge::game_ready) id: String,
    pub(in crate::scene_bridge::game_ready) position: Vec3,
    pub(in crate::scene_bridge::game_ready) radius: f32,
    pub(in crate::scene_bridge::game_ready) scale: Vec3,
}

#[derive(Clone, Debug)]
pub(in crate::scene_bridge::game_ready) struct GameReadyMissionGoalSpec {
    pub(in crate::scene_bridge::game_ready) id: String,
    pub(in crate::scene_bridge::game_ready) position: Vec3,
    pub(in crate::scene_bridge::game_ready) radius: f32,
    pub(in crate::scene_bridge::game_ready) scale: Vec3,
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
