use newengine_math::Vec3;
use newengine_model_domain_api::MeshRenderOptions;

pub(crate) type ColorRgba = [f32; 4];
pub(crate) type ColorRgb = [f32; 3];

#[derive(Clone, Debug)]
pub(crate) struct GameReadyMapProfile {
    pub(crate) title: String,
    pub(crate) objective: String,
    pub(crate) authored_map_streaming: Option<GameReadyAuthoredMapStreamingSpec>,
    pub(crate) player: GameReadyPlayerSpec,
    pub(crate) terrain: GameReadyTerrainSpec,
    pub(crate) sky: GameReadySkySpec,
    pub(crate) materials: GameReadyMaterialSetSpec,
    pub(crate) lighting: GameReadyLightingSpec,
    pub(crate) foliage: GameReadyFoliageSpec,
    pub(crate) prefabs: Vec<GameReadyPrefabSpec>,
    pub(crate) definitions: Vec<GameReadyDefinitionInstanceSpec>,
    pub(crate) acoustic_materials: newengine_audio_api::AcousticMaterialLibrary,
    pub(crate) gameplay: GameReadyGameplaySpec,
    pub(crate) palette: GameReadyPaletteSpec,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyAuthoredMapStreamingSpec {
    pub(crate) map_ref: String,
    pub(crate) index: newengine_assets_api::MapIndexV1,
    pub(crate) initial_cells: Vec<newengine_assets_api::MapCellCoordV1>,
    pub(crate) initial_placement_ids:
        std::collections::BTreeMap<newengine_assets_api::MapCellCoordV1, Vec<String>>,
    pub(crate) resident_radius: i32,
    pub(crate) unload_radius: i32,
    pub(crate) max_cells_per_tick: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyPlayerSpec {
    pub(crate) start: Vec3,
    pub(crate) yaw: f32,
    /// Legacy base movement scalar. Kept for old map payload compatibility; `run_speed`
    /// is the authoritative authored locomotion target after YTYP hydration.
    pub(crate) move_speed: f32,
    pub(crate) walk_speed: f32,
    pub(crate) run_speed: f32,
    pub(crate) sprint_speed: f32,
    pub(crate) crouch_speed: f32,
    pub(crate) look_sens: f32,
    pub(crate) model: GameReadyPlayerModelSpec,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyPlayerModelSpec {
    pub(crate) enabled: bool,
    pub(crate) source: String,
    pub(crate) properties_ref: Option<String>,
    pub(crate) texture_dictionary: Option<String>,
    pub(crate) skeleton: Option<String>,
    pub(crate) idle_animation: Option<String>,
    pub(crate) walk_animation: Option<String>,
    pub(crate) run_animation: Option<String>,
    pub(crate) sprint_animation: Option<String>,
    pub(crate) crouch_idle_animation: Option<String>,
    pub(crate) crouch_walk_animation: Option<String>,
    pub(crate) jump_animation: Option<String>,
    pub(crate) fall_animation: Option<String>,
    pub(crate) equipment_ready_animation: Option<String>,
    pub(crate) equipment_aim_animation: Option<String>,
    pub(crate) equipment_reload_animation: Option<String>,
    pub(crate) unarmed_ready_animation: Option<String>,
    pub(crate) unarmed_attack_animation: Option<String>,
    pub(crate) equipment_ready_sample_phase: f32,
    pub(crate) equipment_ready_rotation_weights:
        Vec<newengine_engine_runtime::gameplay::PlayerJointRotationWeight>,
    pub(crate) equipment_aim_rotation_weights:
        Vec<newengine_engine_runtime::gameplay::PlayerJointRotationWeight>,
    pub(crate) equipment_reload_rotation_weights:
        Vec<newengine_engine_runtime::gameplay::PlayerJointRotationWeight>,
    pub(crate) equipment_arm_ik: bool,
    pub(crate) target_height: f32,
    pub(crate) eye_height_ratio: f32,
    pub(crate) local_offset: Vec3,
    pub(crate) yaw_offset: f32,
    pub(crate) hide_in_first_person: bool,
    pub(crate) render_options: MeshRenderOptions,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyTerrainSpec {
    /// Whether the procedural terrain mesh/heightfield is instantiated. Maps with
    /// authored world meshes can disable it and use static mesh collision instead.
    pub(crate) enabled: bool,
    pub(crate) seed: u64,
    pub(crate) cells_x: u32,
    pub(crate) cells_z: u32,
    pub(crate) size_x: f32,
    pub(crate) size_z: f32,
    pub(crate) base_height: f32,
    pub(crate) height_scale: f32,
    pub(crate) render_options: MeshRenderOptions,
    pub(crate) generator: GameReadyTerrainGeneratorSpec,
    pub(crate) surface: GameReadyTerrainSurfaceSpec,
    pub(crate) heightmap: GameReadyTerrainHeightmapSpec,
    pub(crate) streaming: GameReadyTerrainStreamingSpec,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyTerrainSurfaceSpec {
    pub(crate) forest_base_texture: String,
    pub(crate) sand_base_texture: String,
    pub(crate) rock_base_texture: String,
    pub(crate) patch_scale: f32,
    pub(crate) blend_softness: f32,
    /// Declarative authoring projection for the terrain surface package.
    /// Runtime currently maps these roles onto the stable 3-channel terrain shader
    /// contract: forest/base, sand/path and rock/slope. The fixed fields above are
    /// the compatibility projection consumed by the renderer.
    pub(crate) layers: Vec<GameReadyTerrainSurfaceLayerSpec>,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyTerrainSurfaceLayerSpec {
    pub(crate) role: String,
    pub(crate) base_texture: String,
    pub(crate) weight: f32,
    pub(crate) uv_scale: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyTerrainHeightmapSpec {
    pub(crate) enabled: bool,
    pub(crate) source: String,
    pub(crate) mode: String,
    pub(crate) strength: f32,
    pub(crate) min_height: f32,
    pub(crate) max_height: f32,
    pub(crate) tile_scale: [f32; 2],
    pub(crate) tile_offset: [f32; 2],
    pub(crate) invert: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyTerrainStreamingSpec {
    pub(crate) enabled: bool,
    pub(crate) chunk_radius: i32,
    pub(crate) unload_radius: i32,
    pub(crate) max_chunks_per_frame: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyTerrainGeneratorSpec {
    pub(crate) id: String,
    pub(crate) ridged_seed_xor: u64,
    pub(crate) ridged_frequency: f32,
    pub(crate) ridged_amplitude: f32,
    pub(crate) ridged_shape_edge0: f32,
    pub(crate) ridged_shape_edge1: f32,
    pub(crate) veins_seed_xor: u64,
    pub(crate) veins_frequency: f32,
    pub(crate) veins_amplitude: f32,
    pub(crate) smoothing_passes: u32,
    pub(crate) smoothing_strength: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadySkySpec {
    pub(crate) definition_ref: String,
    pub(crate) render_options: MeshRenderOptions,
    pub(crate) radius: f32,
    pub(crate) mesh: String,
    pub(crate) follow_camera: bool,
    pub(crate) environment_profile: String,
    pub(crate) environment_region: String,
    pub(crate) environment_biome: String,
    pub(crate) cloud_dictionary: String,
    pub(crate) cloud_profile: String,
    pub(crate) sun_radius: f32,
    pub(crate) moon_radius: f32,
    pub(crate) moon_texture: String,
    pub(crate) atmosphere: GameReadySkyAtmosphereSpec,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadySkyAtmosphereSpec {
    pub(crate) day_zenith: ColorRgb,
    pub(crate) day_horizon: ColorRgb,
    pub(crate) dusk_zenith: ColorRgb,
    pub(crate) dusk_horizon: ColorRgb,
    pub(crate) night_zenith: ColorRgb,
    pub(crate) night_horizon: ColorRgb,
    pub(crate) cloud_day: ColorRgb,
    pub(crate) cloud_night: ColorRgb,
    pub(crate) night_sky_strength: f32,
    pub(crate) cloud_coverage: f32,
    pub(crate) cloud_softness: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyPaletteSpec {
    pub(crate) terrain: ColorRgba,
    pub(crate) sky: ColorRgba,
    pub(crate) sky_emissive: ColorRgb,
    pub(crate) tree_bark: ColorRgba,
    pub(crate) tree_leaf: ColorRgba,
    pub(crate) tree_branch: ColorRgba,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyMaterialSetSpec {
    pub(crate) terrain: GameReadyMaterialSpec,
    pub(crate) sky: GameReadyMaterialSpec,
    pub(crate) sun: GameReadyMaterialSpec,
    pub(crate) moon: GameReadyMaterialSpec,
    pub(crate) tree_bark: GameReadyMaterialSpec,
    pub(crate) tree_leaf: GameReadyMaterialSpec,
    pub(crate) tree_branch: GameReadyMaterialSpec,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyMaterialSpec {
    pub(crate) asset: Option<String>,
    pub(crate) base_color_texture: Option<String>,
    pub(crate) normal_texture: Option<String>,
    pub(crate) roughness_texture: Option<String>,
    pub(crate) uv_scale: [f32; 2],
    pub(crate) uv_offset: [f32; 2],
    pub(crate) roughness: f32,
    pub(crate) normal_scale: f32,
    pub(crate) occlusion_strength: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyLightingSpec {
    pub(crate) ambient_color: ColorRgb,
    pub(crate) ambient_intensity: f32,
    pub(crate) sun_direction: ColorRgb,
    pub(crate) sun_color: ColorRgb,
    pub(crate) sun_intensity: f32,
    pub(crate) shadows: GameReadyShadowSpec,
    pub(crate) day_night: GameReadyDayNightSpec,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyDayNightSpec {
    pub(crate) enabled: bool,
    pub(crate) time_of_day_hours: f32,
    pub(crate) day_length_seconds: f32,
    /// Seasonal day in the tropical year, 1..=366. It drives solar declination
    /// independently from the time-of-day clock.
    pub(crate) day_of_year: u32,
    pub(crate) latitude_degrees: f32,
    pub(crate) axial_tilt_degrees: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyShadowSpec {
    pub(crate) enabled: bool,
    pub(crate) resolution: u32,
    pub(crate) cascade_count: u32,
    pub(crate) max_distance: f32,
    pub(crate) softness: f32,
    pub(crate) bias: f32,
    pub(crate) normal_bias: f32,
    pub(crate) contact_strength: f32,
    pub(crate) filter: newengine_lighting::ShadowFilter,
    pub(crate) pcss: newengine_lighting::ShadowPcssSettings,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyFoliageSpec {
    pub(crate) enabled: bool,
    pub(crate) settings: newengine_model_domain_api::FoliageSettings,
    pub(crate) prefab: String,
    pub(crate) alternate_prefab: String,
    pub(crate) alternate_canonical_path: String,
    pub(crate) alternate_weight: f32,
    pub(crate) alternate_collision_radius: f32,
    pub(crate) alternate_collision_half_height: f32,
    pub(crate) alternate_collision_center: Vec3,
    pub(crate) seed: u64,
    pub(crate) grid_min: i32,
    pub(crate) grid_max: i32,
    pub(crate) spacing: f32,
    pub(crate) jitter: f32,
    pub(crate) gate_threshold: f32,
    pub(crate) max_count: u32,
    pub(crate) min_scale: f32,
    pub(crate) max_scale: f32,
    pub(crate) min_player_distance: f32,
    pub(crate) edge_margin: f32,
    pub(crate) surface_offset: f32,
    pub(crate) collision_enabled: bool,
    pub(crate) collision_radius: f32,
    pub(crate) collision_half_height: f32,
    pub(crate) collision_center: Vec3,
    pub(crate) render_options: MeshRenderOptions,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyPrefabSpec {
    pub(crate) id: String,
    pub(crate) authored_map_ref: String,
    pub(crate) authored_placement_id: String,
    /// Discrete YMAP cell ownership. None for legacy/profile-authored prefabs.
    pub(crate) authored_cell: Option<newengine_assets_api::MapCellCoordV1>,
    pub(crate) authored_discrete_placement: bool,
    pub(crate) authored_primary: bool,
    pub(crate) source: String,
    pub(crate) proxy: String,
    pub(crate) material: String,
    pub(crate) enabled: bool,
    pub(crate) position: Vec3,
    pub(crate) rotation_ypr: Vec3,
    pub(crate) scale: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum GameReadyDefinitionApplyMode {
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
    pub(crate) fn from_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "instantiate" | "instantiate_marker" | "marker" | "diagnostic_marker" => {
                Self::InstantiateMarker
            }
            _ => Self::MetadataOnly,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::MetadataOnly => "metadata_only",
            Self::InstantiateMarker => "instantiate_marker",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyDefinitionInstanceSpec {
    pub(crate) definition_ref: String,
    pub(crate) position: Vec3,
    pub(crate) rotation_ypr: [f32; 3],
    pub(crate) scale: Vec3,
    pub(crate) apply_mode: GameReadyDefinitionApplyMode,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyGameplaySpec {
    pub(crate) default_status: String,
    pub(crate) pickup_status: String,
    pub(crate) target_status: String,
    pub(crate) hazard_status: String,
    pub(crate) goal_locked_status: String,
    pub(crate) goal_complete_status: String,
    pub(crate) failed_progress_label: String,
    pub(crate) completed_progress_label: String,
    pub(crate) player_collision: GameReadyPlayerCollisionSpec,
    pub(crate) player_visual: GameReadyPlayerVisualSpec,
    pub(crate) physics: GameReadyPhysicsSpec,
    pub(crate) mission: GameReadyMissionSpec,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GameReadyMissionSpec {
    pub(crate) pickups: Vec<GameReadyMissionPickupSpec>,
    pub(crate) targets: Vec<GameReadyMissionTargetSpec>,
    pub(crate) hazards: Vec<GameReadyMissionHazardSpec>,
    pub(crate) goals: Vec<GameReadyMissionGoalSpec>,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyMissionPickupSpec {
    pub(crate) id: String,
    pub(crate) item: Option<String>,
    pub(crate) quantity: u32,
    pub(crate) auto_equip: bool,
    pub(crate) position: Vec3,
    pub(crate) rotation_ypr: Vec3,
    pub(crate) radius: f32,
    pub(crate) scale: Vec3,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyMissionTargetSpec {
    pub(crate) id: String,
    pub(crate) position: Vec3,
    pub(crate) health: f32,
    pub(crate) scale: Vec3,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyMissionHazardSpec {
    pub(crate) id: String,
    pub(crate) position: Vec3,
    pub(crate) radius: f32,
    pub(crate) scale: Vec3,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyMissionGoalSpec {
    pub(crate) id: String,
    pub(crate) position: Vec3,
    pub(crate) radius: f32,
    pub(crate) scale: Vec3,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyPlayerCollisionSpec {
    pub(crate) radius: f32,
    pub(crate) half_height: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyPlayerVisualSpec {
    pub(crate) radius: f32,
    pub(crate) half_height: f32,
    pub(crate) camera_eye_height: f32,
    pub(crate) sprint_multiplier: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyPhysicsSpec {
    pub(crate) gravity: f32,
    pub(crate) contact_skin: f32,
}
