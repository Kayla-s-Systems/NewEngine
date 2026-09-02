pub use newengine_authored_world_runtime::{AuthoredMapStreamingSpec, AuthoredWorldPlacementSpec};
use newengine_math::Vec3;

#[derive(Clone, Debug)]
pub struct AuthoredWorldProfile {
    pub title: String,
    pub objective: String,
    pub authored_map_streaming: Option<AuthoredMapStreamingSpec>,
    pub player: GameReadyPlayerSpec,
    pub terrain: GameReadyTerrainSpec,
    pub sky: GameReadySkySpec,
    pub materials: GameReadyMaterialSetSpec,
    pub lighting: GameReadyLightingSpec,
    pub foliage: GameReadyFoliageSpec,
    pub prefabs: Vec<AuthoredWorldPlacementSpec>,
    pub definitions: Vec<GameReadyDefinitionInstanceSpec>,
    pub audio_emitters: Vec<GameReadyAudioEmitterSpec>,
    pub acoustic_materials: newengine_audio_api::AcousticMaterialLibrary,
    pub gameplay: GameReadyGameplaySpec,
    pub palette: GameReadyPaletteSpec,
}

#[derive(Clone, Debug)]
pub struct GameReadyPlayerSpec {
    pub start: Vec3,
    pub yaw: f32,
    /// Legacy base movement scalar. Kept for old map payload compatibility; `run_speed`
    /// is the authoritative authored locomotion target after YTYP hydration.
    pub move_speed: f32,
    pub walk_speed: f32,
    pub run_speed: f32,
    pub sprint_speed: f32,
    pub crouch_speed: f32,
    pub combat_team: Option<u32>,
    pub health_maximum: f32,
    pub stamina_maximum: f32,
    pub stamina_sprint_drain_per_second: f32,
    pub stamina_regen_per_second: f32,
    pub stamina_regen_delay_seconds: f32,
    pub stamina_exhausted_resume_fraction: f32,
    pub damage_response_tuning: newengine_engine_runtime::gameplay::CharacterDamageResponseTuning,
    pub death_policy: newengine_engine_runtime::gameplay::CharacterDeathPolicy,
    pub look_sens: f32,
    pub model: GameReadyPlayerModelSpec,
}

pub use newengine_fps_character_runtime::AuthoredPlayerModelSpec as GameReadyPlayerModelSpec;

pub type AuthoredFpsPlayerSpec = GameReadyPlayerSpec;
pub type AuthoredFpsPlayerModelSpec = GameReadyPlayerModelSpec;
pub type AuthoredFpsAudioEmitterSpec = GameReadyAudioEmitterSpec;
pub type AuthoredFpsDefinitionApplyMode = GameReadyDefinitionApplyMode;
pub type AuthoredFpsDefinitionInstanceSpec = GameReadyDefinitionInstanceSpec;
pub type AuthoredFpsGameplaySpec = GameReadyGameplaySpec;
pub type AuthoredFpsDayNightSpec = GameReadyDayNightSpec;
pub type AuthoredFpsLightingSpec = GameReadyLightingSpec;
pub type AuthoredFpsShadowSpec = GameReadyShadowSpec;
pub type AuthoredFpsSkyAtmosphereSpec = GameReadySkyAtmosphereSpec;
pub type AuthoredFpsSkySpec = GameReadySkySpec;
pub type AuthoredFpsTerrainSpec = GameReadyTerrainSpec;

pub use newengine_material_domain_api::AuthoredMaterialSpec as GameReadyMaterialSpec;
pub use newengine_world_environment_api::authored_profile::{
    AuthoredDayNightSpec as GameReadyDayNightSpec,
    AuthoredEnvironmentMaterialSetSpec as GameReadyMaterialSetSpec,
    AuthoredEnvironmentPaletteSpec as GameReadyPaletteSpec,
    AuthoredFoliageSpec as GameReadyFoliageSpec, AuthoredLightingSpec as GameReadyLightingSpec,
    AuthoredShadowSpec as GameReadyShadowSpec,
    AuthoredSkyAtmosphereSpec as GameReadySkyAtmosphereSpec, AuthoredSkySpec as GameReadySkySpec,
    AuthoredTerrainGeneratorSpec as GameReadyTerrainGeneratorSpec,
    AuthoredTerrainHeightmapSpec as GameReadyTerrainHeightmapSpec,
    AuthoredTerrainSpec as GameReadyTerrainSpec,
    AuthoredTerrainStreamingSpec as GameReadyTerrainStreamingSpec,
    AuthoredTerrainSurfaceLayerSpec as GameReadyTerrainSurfaceLayerSpec,
    AuthoredTerrainSurfaceSpec as GameReadyTerrainSurfaceSpec, ColorRgb, ColorRgba,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GameReadyDefinitionApplyMode {
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
    pub fn from_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "instantiate" | "instantiate_marker" | "marker" | "diagnostic_marker" => {
                Self::InstantiateMarker
            }
            _ => Self::MetadataOnly,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MetadataOnly => "metadata_only",
            Self::InstantiateMarker => "instantiate_marker",
        }
    }
}

#[derive(Clone, Debug)]
pub struct GameReadyAudioEmitterSpec {
    pub id: String,
    pub position: Vec3,
    pub emitter: newengine_audio_api::AudioEmitter,
}

#[derive(Clone, Debug)]
pub struct GameReadyDefinitionInstanceSpec {
    pub definition_ref: String,
    pub position: Vec3,
    pub rotation_ypr: [f32; 3],
    pub scale: Vec3,
    pub apply_mode: GameReadyDefinitionApplyMode,
}

#[derive(Clone, Debug)]
pub struct GameReadyGameplaySpec {
    pub default_status: String,
    pub pickup_status: String,
    pub target_status: String,
    pub hazard_status: String,
    pub goal_locked_status: String,
    pub goal_complete_status: String,
    pub failed_progress_label: String,
    pub completed_progress_label: String,
    pub player_collision: GameReadyPlayerCollisionSpec,
    pub player_visual: GameReadyPlayerVisualSpec,
    pub camera: GameReadyCameraSpec,
    pub physics: GameReadyPhysicsSpec,
    pub mission: AuthoredMissionSpec,
}

#[derive(Clone, Debug, Default)]
pub struct AuthoredMissionSpec {
    pub core_material: Option<String>,
    pub target_material: Option<String>,
    pub hazard_material: Option<String>,
    pub goal_material: Option<String>,
    pub pickups: Vec<AuthoredMissionPickupSpec>,
    pub targets: Vec<AuthoredMissionTargetSpec>,
    pub hazards: Vec<AuthoredMissionHazardSpec>,
    pub goals: Vec<AuthoredMissionGoalSpec>,
}

#[derive(Clone, Debug)]
pub struct AuthoredMissionPickupSpec {
    pub id: String,
    pub item: Option<String>,
    pub quantity: u32,
    pub auto_equip: bool,
    pub position: Vec3,
    pub rotation_ypr: Vec3,
    pub radius: f32,
    pub scale: Vec3,
}

#[derive(Clone, Debug)]
pub struct AuthoredMissionTargetSpec {
    pub id: String,
    pub character_ref: Option<String>,
    pub position: Vec3,
    pub health: f32,
    pub scale: Vec3,
    pub ai: Option<GameReadyEnemyAiSpec>,
}

#[derive(Clone, Debug)]
pub struct GameReadyEnemyAiSpec {
    pub combat_team: u32,
    pub sight_range: f32,
    pub field_of_view_degrees: f32,
    pub memory_seconds: f32,
    pub decision_interval_seconds: f32,
    pub navigation: newengine_engine_runtime::gameplay::AINavigationTuning,
    pub patrol_route: Vec<Vec3>,
    pub patrol_looping: bool,
    pub combat: newengine_gameplay_fps_api::FpsAiCombatTuning,
    pub weapon_mount: newengine_gameplay_fps_api::FpsActorWeaponMountTuning,
    pub loadout: String,
}

#[derive(Clone, Debug)]
pub struct AuthoredMissionHazardSpec {
    pub id: String,
    pub position: Vec3,
    pub radius: f32,
    pub scale: Vec3,
}

#[derive(Clone, Debug)]
pub struct AuthoredMissionGoalSpec {
    pub id: String,
    pub position: Vec3,
    pub radius: f32,
    pub scale: Vec3,
}

#[derive(Clone, Debug)]
pub struct GameReadyPlayerCollisionSpec {
    pub radius: f32,
    pub half_height: f32,
}

#[derive(Clone, Debug)]
pub struct GameReadyPlayerVisualSpec {
    pub radius: f32,
    pub half_height: f32,
    pub camera_eye_height: f32,
    pub sprint_multiplier: f32,
}

#[derive(Clone, Debug)]
pub struct GameReadyCameraSpec {
    pub definition_ref: String,
    pub declared: bool,
    pub instance_id: String,
    pub initial_view: newengine_engine_runtime::gameplay::PlayerCameraViewMode,
    pub position: Vec3,
    pub rotation_ypr: Vec3,
    pub first_person_fov_y_radians: f32,
    pub first_person_ads_fov_y_radians: f32,
    pub first_person_near: f32,
    pub first_person_forward_clearance: f32,
    pub first_person_body_yaw_limit_radians: f32,
    pub first_person_down_pitch_limit_radians: f32,
    pub first_person_collision_enabled: bool,
    pub first_person_collision_probe_radius: f32,
    pub first_person_collision_padding: f32,
    pub first_person_grounded_eye_deadband_m: f32,
    pub first_person_grounded_eye_time_constant_seconds: f32,
    pub first_person_camera_recoil_share: f32,
    pub first_person_aim_response_hz: f32,
    pub near_clip_enabled: bool,
    pub near_clip_first_person_max_distance: f32,
    pub near_clip_third_person_min_distance: f32,
    pub near_clip_third_person_max_distance: f32,
    pub near_clip_pull_in_distance: f32,
    pub near_clip_probe_radius: f32,
    pub near_clip_release_time_seconds: f32,
    pub near_clip_hysteresis_m: f32,
    pub third_person_follow_fov_y_radians: f32,
    pub third_person_follow_offset_ls: Vec3,
    pub third_person_follow_focus_offset_ls: Vec3,
    pub third_person_follow_smooth_time: f32,
    pub third_person_follow_max_speed: f32,
    pub third_person_follow_zoom_min: f32,
    pub third_person_follow_zoom_max: f32,
    pub third_person_aim_fov_y_radians: f32,
    pub third_person_aim_offset_ls: Vec3,
    pub third_person_aim_focus_offset_ls: Vec3,
    pub third_person_aim_smooth_time: f32,
    pub third_person_aim_max_speed: f32,
    pub third_person_aim_zoom_min: f32,
    pub third_person_aim_zoom_max: f32,
    pub third_person_orbit_fov_y_radians: f32,
    pub third_person_orbit_offset_ls: Vec3,
    pub third_person_orbit_focus_offset_ls: Vec3,
    pub third_person_orbit_smooth_time: f32,
    pub third_person_orbit_max_speed: f32,
    pub third_person_orbit_zoom_min: f32,
    pub third_person_orbit_zoom_max: f32,
    pub third_person_orbit_look_sensitivity_radians_per_pixel: f32,
    pub third_person_orbit_pitch_min_radians: f32,
    pub third_person_orbit_pitch_max_radians: f32,
    pub third_person_collision_enabled: bool,
    pub third_person_collision_probe_radius: f32,
    pub third_person_collision_padding: f32,
    pub third_person_collision_min_distance: f32,
    pub third_person_collision_release_frequency_hz: f32,
    pub third_person_collision_release_damping_ratio: f32,
    pub third_person_collision_distance_hysteresis: f32,
    pub third_person_look_at_collision_blend: f32,
    pub third_person_look_at_response_hz: f32,
    pub third_person_look_at_max_error_fov_fraction: f32,
    pub third_person_catch_up_enabled: bool,
    pub third_person_catch_up_frequency_hz: f32,
    pub third_person_catch_up_damping_ratio: f32,
    pub third_person_catch_up_max_distance_m: f32,
    pub third_person_catch_up_settle_distance_m: f32,
    pub zoom_wheel_exponent_per_step: f32,
    pub orbit_drag_zoom_exponent_per_pixel: f32,
    pub zoom_smooth_time_seconds: f32,
    pub gameplay_blend_in_seconds: f32,
    pub gameplay_blend_out_seconds: f32,
    pub gameplay_blend_lock_input: bool,
    pub hide_local_model_in_first_person: bool,
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
    pub fn player_profile(&self) -> newengine_engine_runtime::gameplay::PlayerCameraProfile {
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
pub struct GameReadyPhysicsSpec {
    pub gravity: f32,
    pub contact_skin: f32,
}
