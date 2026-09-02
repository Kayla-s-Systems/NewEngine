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
    pub(crate) initial_render_cells: Vec<newengine_assets_api::MapCellCoordV1>,
    pub(crate) initial_simulation_cells: Vec<newengine_assets_api::MapCellCoordV1>,
    pub(crate) initial_placement_ids:
        std::collections::BTreeMap<newengine_assets_api::MapCellCoordV1, Vec<String>>,
    pub(crate) render_radius: i32,
    pub(crate) simulation_radius: i32,
    pub(crate) render_unload_radius: i32,
    pub(crate) simulation_unload_radius: i32,
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
    pub(crate) combat_team: Option<u32>,
    pub(crate) health_maximum: f32,
    pub(crate) stamina_maximum: f32,
    pub(crate) stamina_sprint_drain_per_second: f32,
    pub(crate) stamina_regen_per_second: f32,
    pub(crate) stamina_regen_delay_seconds: f32,
    pub(crate) stamina_exhausted_resume_fraction: f32,
    pub(crate) damage_response_tuning:
        newengine_engine_runtime::gameplay::CharacterDamageResponseTuning,
    pub(crate) death_policy: newengine_engine_runtime::gameplay::CharacterDeathPolicy,
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
    pub(crate) animation_slots: std::collections::BTreeMap<String, String>,
    pub(crate) animation_event_bindings: std::collections::BTreeMap<String, String>,
    pub(crate) idle_animation: Option<String>,
    pub(crate) walk_animation: Option<String>,
    pub(crate) run_animation: Option<String>,
    pub(crate) sprint_animation: Option<String>,
    pub(crate) crouch_idle_animation: Option<String>,
    pub(crate) crouch_walk_animation: Option<String>,
    pub(crate) jump_animation: Option<String>,
    pub(crate) fall_animation: Option<String>,
    pub(crate) fall_low_animation: Option<String>,
    pub(crate) fall_medium_animation: Option<String>,
    pub(crate) fall_high_animation: Option<String>,
    pub(crate) landing_soft_animation: Option<String>,
    pub(crate) landing_medium_animation: Option<String>,
    pub(crate) landing_hard_animation: Option<String>,
    pub(crate) landing_hard_run_animation: Option<String>,
    pub(crate) fall_medium_min_distance: f32,
    pub(crate) fall_high_min_distance: f32,
    pub(crate) detached_head_follow: bool,
    pub(crate) detached_head_follow_rule:
        Option<newengine_engine_runtime::gameplay::PlayerPaletteFollowRule>,
    pub(crate) eye_parent_follow: bool,
    pub(crate) eye_parent_follow_rule:
        Option<newengine_engine_runtime::gameplay::PlayerEyeParentFollowRule>,
    pub(crate) helper_pose_copies: Vec<newengine_engine_runtime::gameplay::PlayerJointCopyRule>,
    pub(crate) skin_sidecar:
        Option<newengine_engine_runtime::gameplay::PlayerSkinSidecarDefinition>,
    pub(crate) braid_secondary_motion:
        Option<newengine_engine_runtime::gameplay::PlayerBraidSecondaryMotionRig>,
    pub(crate) equipment_ready_animation: Option<String>,
    pub(crate) equipment_aim_animation: Option<String>,
    pub(crate) equipment_reload_animation: Option<String>,
    pub(crate) unarmed_ready_animation: Option<String>,
    pub(crate) unarmed_attack_animation: Option<String>,
    /// Optional authored turn-in-place clips. These are full-body steps; stationary mouse yaw never
    /// rotates the world root directly. Runtime selects the nearest signed angle.
    pub(crate) turn_45_left_animation: Option<String>,
    pub(crate) turn_45_right_animation: Option<String>,
    pub(crate) turn_90_left_animation: Option<String>,
    pub(crate) turn_90_right_animation: Option<String>,
    pub(crate) turn_135_left_animation: Option<String>,
    pub(crate) turn_135_right_animation: Option<String>,
    pub(crate) turn_180_left_animation: Option<String>,
    pub(crate) turn_180_right_animation: Option<String>,
    pub(crate) equipment_ready_sample_phase: f32,
    pub(crate) equipment_ready_sample_phases: std::collections::BTreeMap<String, f32>,
    pub(crate) equipment_ready_rotation_weights:
        Vec<newengine_engine_runtime::gameplay::PlayerJointRotationWeight>,
    pub(crate) equipment_aim_rotation_weights:
        Vec<newengine_engine_runtime::gameplay::PlayerJointRotationWeight>,
    pub(crate) equipment_reload_rotation_weights:
        Vec<newengine_engine_runtime::gameplay::PlayerJointRotationWeight>,
    pub(crate) equipment_arm_ik: bool,
    pub(crate) equipment_arm_ik_rig:
        Option<newengine_engine_runtime::gameplay::PlayerWeaponArmIkRigDefinition>,
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
    /// Project-authored physics surface identity and generic semantic event bindings.
    /// Keys are capability/signal ids; values are arbitrary project gameplay event ids.
    pub(crate) surface_id: String,
    pub(crate) surface_events: std::collections::BTreeMap<String, String>,
    pub(crate) ballistic_material:
        Option<newengine_engine_runtime::gameplay::BallisticMaterialResponse>,
    pub(crate) ground_placement_surface: bool,
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
    pub(crate) camera: GameReadyCameraSpec,
    pub(crate) physics: GameReadyPhysicsSpec,
    pub(crate) mission: GameReadyMissionSpec,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GameReadyMissionSpec {
    pub(crate) core_material: Option<String>,
    pub(crate) target_material: Option<String>,
    pub(crate) hazard_material: Option<String>,
    pub(crate) goal_material: Option<String>,
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
    pub(crate) character_ref: Option<String>,
    pub(crate) position: Vec3,
    pub(crate) health: f32,
    pub(crate) scale: Vec3,
    pub(crate) ai: Option<GameReadyEnemyAiSpec>,
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyEnemyAiSpec {
    pub(crate) combat_team: u32,
    pub(crate) sight_range: f32,
    pub(crate) field_of_view_degrees: f32,
    pub(crate) memory_seconds: f32,
    pub(crate) decision_interval_seconds: f32,
    pub(crate) navigation: newengine_engine_runtime::gameplay::AINavigationTuning,
    pub(crate) patrol_route: Vec<Vec3>,
    pub(crate) patrol_looping: bool,
    pub(crate) combat: newengine_gameplay_fps_api::FpsAiCombatTuning,
    pub(crate) weapon_mount: newengine_gameplay_fps_api::FpsActorWeaponMountTuning,
    pub(crate) loadout: String,
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
pub(crate) struct GameReadyCameraSpec {
    pub(crate) definition_ref: String,
    pub(crate) declared: bool,
    pub(crate) instance_id: String,
    pub(crate) initial_view: newengine_engine_runtime::gameplay::PlayerCameraViewMode,
    pub(crate) position: Vec3,
    pub(crate) rotation_ypr: Vec3,
    pub(crate) first_person_fov_y_radians: f32,
    pub(crate) first_person_ads_fov_y_radians: f32,
    pub(crate) first_person_near: f32,
    pub(crate) first_person_forward_clearance: f32,
    pub(crate) first_person_body_yaw_limit_radians: f32,
    pub(crate) first_person_down_pitch_limit_radians: f32,
    pub(crate) first_person_collision_enabled: bool,
    pub(crate) first_person_collision_probe_radius: f32,
    pub(crate) first_person_collision_padding: f32,
    pub(crate) first_person_grounded_eye_deadband_m: f32,
    pub(crate) first_person_grounded_eye_time_constant_seconds: f32,
    pub(crate) first_person_camera_recoil_share: f32,
    pub(crate) first_person_aim_response_hz: f32,
    pub(crate) near_clip_enabled: bool,
    pub(crate) near_clip_first_person_max_distance: f32,
    pub(crate) near_clip_third_person_min_distance: f32,
    pub(crate) near_clip_third_person_max_distance: f32,
    pub(crate) near_clip_pull_in_distance: f32,
    pub(crate) near_clip_probe_radius: f32,
    pub(crate) near_clip_release_time_seconds: f32,
    pub(crate) near_clip_hysteresis_m: f32,
    pub(crate) third_person_follow_fov_y_radians: f32,
    pub(crate) third_person_follow_offset_ls: Vec3,
    pub(crate) third_person_follow_focus_offset_ls: Vec3,
    pub(crate) third_person_follow_smooth_time: f32,
    pub(crate) third_person_follow_max_speed: f32,
    pub(crate) third_person_follow_zoom_min: f32,
    pub(crate) third_person_follow_zoom_max: f32,
    pub(crate) third_person_aim_fov_y_radians: f32,
    pub(crate) third_person_aim_offset_ls: Vec3,
    pub(crate) third_person_aim_focus_offset_ls: Vec3,
    pub(crate) third_person_aim_smooth_time: f32,
    pub(crate) third_person_aim_max_speed: f32,
    pub(crate) third_person_aim_zoom_min: f32,
    pub(crate) third_person_aim_zoom_max: f32,
    pub(crate) third_person_orbit_fov_y_radians: f32,
    pub(crate) third_person_orbit_offset_ls: Vec3,
    pub(crate) third_person_orbit_focus_offset_ls: Vec3,
    pub(crate) third_person_orbit_smooth_time: f32,
    pub(crate) third_person_orbit_max_speed: f32,
    pub(crate) third_person_orbit_zoom_min: f32,
    pub(crate) third_person_orbit_zoom_max: f32,
    pub(crate) third_person_orbit_look_sensitivity_radians_per_pixel: f32,
    pub(crate) third_person_orbit_pitch_min_radians: f32,
    pub(crate) third_person_orbit_pitch_max_radians: f32,
    pub(crate) third_person_collision_enabled: bool,
    pub(crate) third_person_collision_probe_radius: f32,
    pub(crate) third_person_collision_padding: f32,
    pub(crate) third_person_collision_min_distance: f32,
    pub(crate) third_person_collision_release_frequency_hz: f32,
    pub(crate) third_person_collision_release_damping_ratio: f32,
    pub(crate) third_person_collision_distance_hysteresis: f32,
    pub(crate) third_person_look_at_collision_blend: f32,
    pub(crate) third_person_look_at_response_hz: f32,
    pub(crate) third_person_look_at_max_error_fov_fraction: f32,
    pub(crate) third_person_catch_up_enabled: bool,
    pub(crate) third_person_catch_up_frequency_hz: f32,
    pub(crate) third_person_catch_up_damping_ratio: f32,
    pub(crate) third_person_catch_up_max_distance_m: f32,
    pub(crate) third_person_catch_up_settle_distance_m: f32,
    pub(crate) zoom_wheel_exponent_per_step: f32,
    pub(crate) orbit_drag_zoom_exponent_per_pixel: f32,
    pub(crate) zoom_smooth_time_seconds: f32,
    pub(crate) gameplay_blend_in_seconds: f32,
    pub(crate) gameplay_blend_out_seconds: f32,
    pub(crate) gameplay_blend_lock_input: bool,
    pub(crate) hide_local_model_in_first_person: bool,
}

impl Default for GameReadyCameraSpec {
    fn default() -> Self {
        let d = newengine_engine_runtime::gameplay::PlayerCameraProfile::default();
        Self {
            definition_ref: String::new(),
            declared: false,
            instance_id: String::new(),
            initial_view: d.initial_view,
            position: Vec3::ZERO,
            rotation_ypr: Vec3::ZERO,
            first_person_fov_y_radians: d.first_person_fov_y_radians,
            first_person_ads_fov_y_radians: d.first_person_ads_fov_y_radians,
            first_person_near: d.first_person_near,
            first_person_forward_clearance: d.first_person_forward_clearance,
            first_person_body_yaw_limit_radians: d.first_person_body_yaw_limit_radians,
            first_person_down_pitch_limit_radians: d.first_person_down_pitch_limit_radians,
            first_person_collision_enabled: d.first_person_collision_enabled,
            first_person_collision_probe_radius: d.first_person_collision_probe_radius,
            first_person_collision_padding: d.first_person_collision_padding,
            first_person_grounded_eye_deadband_m: d.first_person_grounded_eye_deadband_m,
            first_person_grounded_eye_time_constant_seconds: d
                .first_person_grounded_eye_time_constant_seconds,
            first_person_camera_recoil_share: d.first_person_camera_recoil_share,
            first_person_aim_response_hz: d.first_person_aim_response_hz,
            near_clip_enabled: d.near_clip_enabled,
            near_clip_first_person_max_distance: d.near_clip_first_person_max_distance,
            near_clip_third_person_min_distance: d.near_clip_third_person_min_distance,
            near_clip_third_person_max_distance: d.near_clip_third_person_max_distance,
            near_clip_pull_in_distance: d.near_clip_pull_in_distance,
            near_clip_probe_radius: d.near_clip_probe_radius,
            near_clip_release_time_seconds: d.near_clip_release_time_seconds,
            near_clip_hysteresis_m: d.near_clip_hysteresis_m,
            third_person_follow_fov_y_radians: d.third_person_follow_fov_y_radians,
            third_person_follow_offset_ls: d.third_person_follow_offset_ls,
            third_person_follow_focus_offset_ls: d.third_person_follow_focus_offset_ls,
            third_person_follow_smooth_time: d.third_person_follow_smooth_time,
            third_person_follow_max_speed: d.third_person_follow_max_speed,
            third_person_follow_zoom_min: d.third_person_follow_zoom_min,
            third_person_follow_zoom_max: d.third_person_follow_zoom_max,
            third_person_aim_fov_y_radians: d.third_person_aim_fov_y_radians,
            third_person_aim_offset_ls: d.third_person_aim_offset_ls,
            third_person_aim_focus_offset_ls: d.third_person_aim_focus_offset_ls,
            third_person_aim_smooth_time: d.third_person_aim_smooth_time,
            third_person_aim_max_speed: d.third_person_aim_max_speed,
            third_person_aim_zoom_min: d.third_person_aim_zoom_min,
            third_person_aim_zoom_max: d.third_person_aim_zoom_max,
            third_person_orbit_fov_y_radians: d.third_person_orbit_fov_y_radians,
            third_person_orbit_offset_ls: d.third_person_orbit_offset_ls,
            third_person_orbit_focus_offset_ls: d.third_person_orbit_focus_offset_ls,
            third_person_orbit_smooth_time: d.third_person_orbit_smooth_time,
            third_person_orbit_max_speed: d.third_person_orbit_max_speed,
            third_person_orbit_zoom_min: d.third_person_orbit_zoom_min,
            third_person_orbit_zoom_max: d.third_person_orbit_zoom_max,
            third_person_orbit_look_sensitivity_radians_per_pixel: d
                .third_person_orbit_look_sensitivity_radians_per_pixel,
            third_person_orbit_pitch_min_radians: d.third_person_orbit_pitch_min_radians,
            third_person_orbit_pitch_max_radians: d.third_person_orbit_pitch_max_radians,
            third_person_collision_enabled: d.third_person_collision_enabled,
            third_person_collision_probe_radius: d.third_person_collision_probe_radius,
            third_person_collision_padding: d.third_person_collision_padding,
            third_person_collision_min_distance: d.third_person_collision_min_distance,
            third_person_collision_release_frequency_hz: d
                .third_person_collision_release_frequency_hz,
            third_person_collision_release_damping_ratio: d
                .third_person_collision_release_damping_ratio,
            third_person_collision_distance_hysteresis: d
                .third_person_collision_distance_hysteresis,
            third_person_look_at_collision_blend: d.third_person_look_at_collision_blend,
            third_person_look_at_response_hz: d.third_person_look_at_response_hz,
            third_person_look_at_max_error_fov_fraction: d
                .third_person_look_at_max_error_fov_fraction,
            third_person_catch_up_enabled: d.third_person_catch_up_enabled,
            third_person_catch_up_frequency_hz: d.third_person_catch_up_frequency_hz,
            third_person_catch_up_damping_ratio: d.third_person_catch_up_damping_ratio,
            third_person_catch_up_max_distance_m: d.third_person_catch_up_max_distance_m,
            third_person_catch_up_settle_distance_m: d.third_person_catch_up_settle_distance_m,
            zoom_wheel_exponent_per_step: d.zoom_wheel_exponent_per_step,
            orbit_drag_zoom_exponent_per_pixel: d.orbit_drag_zoom_exponent_per_pixel,
            zoom_smooth_time_seconds: d.zoom_smooth_time_seconds,
            gameplay_blend_in_seconds: d.gameplay_blend_in_seconds,
            gameplay_blend_out_seconds: d.gameplay_blend_out_seconds,
            gameplay_blend_lock_input: d.gameplay_blend_lock_input,
            hide_local_model_in_first_person: d.hide_local_model_in_first_person,
        }
    }
}

impl GameReadyCameraSpec {
    pub(crate) fn player_profile(&self) -> newengine_engine_runtime::gameplay::PlayerCameraProfile {
        newengine_engine_runtime::gameplay::PlayerCameraProfile {
            initial_view: self.initial_view,
            first_person_fov_y_radians: self.first_person_fov_y_radians,
            first_person_ads_fov_y_radians: self.first_person_ads_fov_y_radians,
            first_person_near: self.first_person_near,
            first_person_forward_clearance: self.first_person_forward_clearance,
            first_person_body_yaw_limit_radians: self.first_person_body_yaw_limit_radians,
            first_person_down_pitch_limit_radians: self.first_person_down_pitch_limit_radians,
            first_person_collision_enabled: self.first_person_collision_enabled,
            first_person_collision_probe_radius: self.first_person_collision_probe_radius,
            first_person_collision_padding: self.first_person_collision_padding,
            first_person_grounded_eye_deadband_m: self.first_person_grounded_eye_deadband_m,
            first_person_grounded_eye_time_constant_seconds: self
                .first_person_grounded_eye_time_constant_seconds,
            first_person_camera_recoil_share: self.first_person_camera_recoil_share,
            first_person_aim_response_hz: self.first_person_aim_response_hz,
            near_clip_enabled: self.near_clip_enabled,
            near_clip_first_person_max_distance: self.near_clip_first_person_max_distance,
            near_clip_third_person_min_distance: self.near_clip_third_person_min_distance,
            near_clip_third_person_max_distance: self.near_clip_third_person_max_distance,
            near_clip_pull_in_distance: self.near_clip_pull_in_distance,
            near_clip_probe_radius: self.near_clip_probe_radius,
            near_clip_release_time_seconds: self.near_clip_release_time_seconds,
            near_clip_hysteresis_m: self.near_clip_hysteresis_m,
            third_person_follow_fov_y_radians: self.third_person_follow_fov_y_radians,
            third_person_follow_offset_ls: self.third_person_follow_offset_ls,
            third_person_follow_focus_offset_ls: self.third_person_follow_focus_offset_ls,
            third_person_follow_smooth_time: self.third_person_follow_smooth_time,
            third_person_follow_max_speed: self.third_person_follow_max_speed,
            third_person_follow_zoom_min: self.third_person_follow_zoom_min,
            third_person_follow_zoom_max: self.third_person_follow_zoom_max,
            third_person_aim_fov_y_radians: self.third_person_aim_fov_y_radians,
            third_person_aim_offset_ls: self.third_person_aim_offset_ls,
            third_person_aim_focus_offset_ls: self.third_person_aim_focus_offset_ls,
            third_person_aim_smooth_time: self.third_person_aim_smooth_time,
            third_person_aim_max_speed: self.third_person_aim_max_speed,
            third_person_aim_zoom_min: self.third_person_aim_zoom_min,
            third_person_aim_zoom_max: self.third_person_aim_zoom_max,
            third_person_orbit_fov_y_radians: self.third_person_orbit_fov_y_radians,
            third_person_orbit_offset_ls: self.third_person_orbit_offset_ls,
            third_person_orbit_focus_offset_ls: self.third_person_orbit_focus_offset_ls,
            third_person_orbit_smooth_time: self.third_person_orbit_smooth_time,
            third_person_orbit_max_speed: self.third_person_orbit_max_speed,
            third_person_orbit_zoom_min: self.third_person_orbit_zoom_min,
            third_person_orbit_zoom_max: self.third_person_orbit_zoom_max,
            third_person_orbit_look_sensitivity_radians_per_pixel: self
                .third_person_orbit_look_sensitivity_radians_per_pixel,
            third_person_orbit_pitch_min_radians: self.third_person_orbit_pitch_min_radians,
            third_person_orbit_pitch_max_radians: self.third_person_orbit_pitch_max_radians,
            third_person_collision_enabled: self.third_person_collision_enabled,
            third_person_collision_probe_radius: self.third_person_collision_probe_radius,
            third_person_collision_padding: self.third_person_collision_padding,
            third_person_collision_min_distance: self.third_person_collision_min_distance,
            third_person_collision_release_frequency_hz: self
                .third_person_collision_release_frequency_hz,
            third_person_collision_release_damping_ratio: self
                .third_person_collision_release_damping_ratio,
            third_person_collision_distance_hysteresis: self
                .third_person_collision_distance_hysteresis,
            third_person_look_at_collision_blend: self.third_person_look_at_collision_blend,
            third_person_look_at_response_hz: self.third_person_look_at_response_hz,
            third_person_look_at_max_error_fov_fraction: self
                .third_person_look_at_max_error_fov_fraction,
            third_person_catch_up_enabled: self.third_person_catch_up_enabled,
            third_person_catch_up_frequency_hz: self.third_person_catch_up_frequency_hz,
            third_person_catch_up_damping_ratio: self.third_person_catch_up_damping_ratio,
            third_person_catch_up_max_distance_m: self.third_person_catch_up_max_distance_m,
            third_person_catch_up_settle_distance_m: self.third_person_catch_up_settle_distance_m,
            zoom_wheel_exponent_per_step: self.zoom_wheel_exponent_per_step,
            orbit_drag_zoom_exponent_per_pixel: self.orbit_drag_zoom_exponent_per_pixel,
            zoom_smooth_time_seconds: self.zoom_smooth_time_seconds,
            gameplay_blend_in_seconds: self.gameplay_blend_in_seconds,
            gameplay_blend_out_seconds: self.gameplay_blend_out_seconds,
            gameplay_blend_lock_input: self.gameplay_blend_lock_input,
            hide_local_model_in_first_person: self.hide_local_model_in_first_person,
        }
        .sanitized()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyPhysicsSpec {
    pub(crate) gravity: f32,
    pub(crate) contact_skin: f32,
}
