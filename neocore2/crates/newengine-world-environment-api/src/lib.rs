#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable service contract for the `engine.world.environment` gateway.
//!
//! Environment is world state. Render is only one consumer. This API exposes
//! sky, celestial, atmosphere, weather, clouds, wind, lighting and gameplay
//! modifiers as DTOs. Providers must not receive native ECS ids, `&mut World`,
//! renderer handles or GPU history buffers.
use abi_stable::std_types::RString;
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_time_api::TimeSnapshotV1;
use newengine_world_api::WorldCellCoord;
use serde::{Deserialize, Serialize};

pub const ENGINE_WORLD_ENVIRONMENT_SERVICE_ID: &str = "engine.world.environment";
pub const WORLD_ENVIRONMENT_DEFAULT_SERVICE_ID: &str = "world.environment.default.api";
pub const WORLD_ENVIRONMENT_NULL_SERVICE_ID: &str = "world.environment.null.api";
pub const WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID: &str = "world.environment.backend";
pub const WORLD_ENVIRONMENT_CELESTIAL_CAPABILITY_ID: &str = "world.environment.celestial";
pub const WORLD_ENVIRONMENT_DAY_NIGHT_CAPABILITY_ID: &str = "world.environment.day_night";
pub const WORLD_ENVIRONMENT_SKY_CAPABILITY_ID: &str = "world.environment.sky";
pub const WORLD_ENVIRONMENT_ATMOSPHERE_CAPABILITY_ID: &str = "world.environment.atmosphere";
pub const WORLD_ENVIRONMENT_CLOUDS_CAPABILITY_ID: &str = "world.environment.clouds";
pub const WORLD_ENVIRONMENT_WEATHER_CAPABILITY_ID: &str = "world.environment.weather";
pub const WORLD_ENVIRONMENT_WIND_CAPABILITY_ID: &str = "world.environment.wind";
pub const WORLD_ENVIRONMENT_SNAPSHOT_CAPABILITY_ID: &str = "world.environment.snapshot";
pub const WORLD_ENVIRONMENT_STREAMING_CAPABILITY_ID: &str = "world.environment.streaming";
pub const WORLD_ENVIRONMENT_DETERMINISTIC_REPLAY_CAPABILITY_ID: &str =
    "world.environment.deterministic_replay";

pub const WORLD_ENVIRONMENT_SERVICE_METHOD_INFO: &str =
    newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const WORLD_ENVIRONMENT_SERVICE_METHOD_INVOKE: &str =
    newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const WORLD_ENVIRONMENT_SERVICE_METHOD_SHUTDOWN_V1: &str =
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const WORLD_ENVIRONMENT_SERVICE_METHOD_FRAME_JSON_V1: &str = "environment.frame_json_v1";
pub const WORLD_ENVIRONMENT_SERVICE_METHOD_SAMPLE_AT_POSITION_JSON_V1: &str =
    "environment.sample_at_position_json_v1";
pub const WORLD_ENVIRONMENT_SERVICE_METHOD_SNAPSHOT_JSON_V1: &str = "environment.snapshot_json_v1";
pub const WORLD_ENVIRONMENT_SERVICE_METHOD_RESTORE_JSON_V1: &str = "environment.restore_json_v1";
pub const WORLD_ENVIRONMENT_SERVICE_METHOD_PREVIEW_TIME_JSON_V1: &str =
    "environment.preview_time_json_v1";

pub const WORLD_ENVIRONMENT_REQUIRED_METHODS_V1: &[&str] = &[
    WORLD_ENVIRONMENT_SERVICE_METHOD_INFO,
    WORLD_ENVIRONMENT_SERVICE_METHOD_INVOKE,
    WORLD_ENVIRONMENT_SERVICE_METHOD_SHUTDOWN_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_FRAME_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_SAMPLE_AT_POSITION_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_SNAPSHOT_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_RESTORE_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_PREVIEW_TIME_JSON_V1,
];

pub const WORLD_ENVIRONMENT_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "world.environment",
        ENGINE_WORLD_ENVIRONMENT_SERVICE_ID,
        WORLD_ENVIRONMENT_DEFAULT_SERVICE_ID,
        WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID,
    );

pub const WORLD_ENVIRONMENT_RUNTIME_CONTRACT_SPEC:
    newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_WORLD_ENVIRONMENT_SERVICE_ID,
        "newengine.world.environment-api >= 0.1.x",
        WORLD_ENVIRONMENT_REQUIRED_METHODS_V1,
    );

/// Missing `engine.world.environment` degrades by profile policy; the default
/// runtime also registers a visible NullEnvironment route for explicit degraded mode.
pub const WORLD_ENVIRONMENT_RUNTIME_REQUIREMENT_SPEC:
    newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        WORLD_ENVIRONMENT_RUNTIME_CONTRACT_SPEC,
        Some(WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_WORLD_ENVIRONMENT_BACKEND"),
    );

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub degraded: bool,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl EnvironmentServiceInfo {
    #[inline]
    pub fn default_provider(provider: impl Into<String>) -> Self {
        Self {
            protocol: "newengine.world.environment-api/v1".to_owned(),
            provider: provider.into(),
            degraded: false,
            features: vec![
                "dto-only-environment-frame".to_owned(),
                "deterministic-day-night-baseline".to_owned(),
                "time-of-day-phase".to_owned(),
                "celestial-state".to_owned(),
                "sky-state".to_owned(),
                "atmosphere-state".to_owned(),
                "weather-state".to_owned(),
                "cloud-state".to_owned(),
                "environment-objects".to_owned(),
                "consumer-packets".to_owned(),
                "wind-state".to_owned(),
                "visual-asset-refs".to_owned(),
                "weather-profile-catalog".to_owned(),
                "lighting-intent".to_owned(),
                "gameplay-modifiers".to_owned(),
            ],
            methods: WORLD_ENVIRONMENT_REQUIRED_METHODS_V1
                .iter()
                .map(|it| (*it).to_owned())
                .collect(),
        }
    }

    #[inline]
    pub fn null_provider(provider: impl Into<String>) -> Self {
        let mut info = Self::default_provider(provider);
        info.degraded = true;
        info.features = vec![
            "dto-only-environment-frame".to_owned(),
            "stable-degraded-neutral-environment".to_owned(),
        ];
        info
    }
}

impl Default for EnvironmentServiceInfo {
    #[inline]
    fn default() -> Self {
        Self::default_provider("environment.default")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec3Dto {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3Dto {
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub const fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    #[inline]
    pub const fn up() -> Self {
        Self {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        }
    }
}

impl Default for Vec3Dto {
    #[inline]
    fn default() -> Self {
        Self::zero()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color3Dto {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color3Dto {
    #[inline]
    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    #[inline]
    pub const fn black() -> Self {
        Self {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        }
    }

    #[inline]
    pub const fn white() -> Self {
        Self {
            r: 1.0,
            g: 1.0,
            b: 1.0,
        }
    }
}

impl Default for Color3Dto {
    #[inline]
    fn default() -> Self {
        Self::black()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AabbDto {
    pub min: Vec3Dto,
    pub max: Vec3Dto,
}

impl Default for AabbDto {
    #[inline]
    fn default() -> Self {
        Self {
            min: Vec3Dto::zero(),
            max: Vec3Dto::zero(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TransformDto {
    pub translation: Vec3Dto,
    pub rotation_quat_xyzw: [f32; 4],
    pub scale: Vec3Dto,
}

impl Default for TransformDto {
    #[inline]
    fn default() -> Self {
        Self {
            translation: Vec3Dto::zero(),
            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
            scale: Vec3Dto::new(1.0, 1.0, 1.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct EnvironmentProfileRefDto {
    #[serde(default)]
    pub profile_id: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct EnvironmentObjectId {
    pub stable_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeatherKind {
    Clear,
    Cloudy,
    Overcast,
    Rain,
    Storm,
    Snow,
    Fog,
    DustStorm,
    HeatHaze,
}

impl Default for WeatherKind {
    #[inline]
    fn default() -> Self {
        Self::Clear
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrecipitationKind {
    None,
    Rain,
    Snow,
    Dust,
}

impl Default for PrecipitationKind {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnvironmentObjectKind {
    CloudField,
    CloudVolume,
    StormCell,
    FogBank,
    WeatherFront,
    DustWall,
    SnowBand,
    HeatHazeZone,
}

impl Default for EnvironmentObjectKind {
    #[inline]
    fn default() -> Self {
        Self::CloudField
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeOfDayPhase {
    Night,
    Dawn,
    Day,
    Dusk,
}

impl Default for TimeOfDayPhase {
    #[inline]
    fn default() -> Self {
        Self::Night
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimeOfDayStateDto {
    pub normalized_day: f32,
    pub hours: f32,
    pub phase: TimeOfDayPhase,
    pub dawn_dusk_blend: f32,
    pub day_blend: f32,
    pub night_blend: f32,
}

impl Default for TimeOfDayStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            normalized_day: 0.0,
            hours: 0.0,
            phase: TimeOfDayPhase::Night,
            dawn_dusk_blend: 0.0,
            day_blend: 0.0,
            night_blend: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentFrameRequest {
    pub frame_id: u64,
    pub world_instance_id: String,
    pub time: TimeSnapshotV1,
    pub observer_position: Vec3Dto,
    pub observer_cell: Option<WorldCellCoord>,
    pub active_region: Option<String>,
    pub active_biome: Option<String>,
    pub resident_cells: Vec<WorldCellCoord>,
    pub environment_profile: EnvironmentProfileRefDto,
    pub seed: u64,
}

impl Default for EnvironmentFrameRequest {
    #[inline]
    fn default() -> Self {
        Self {
            frame_id: 0,
            world_instance_id: "world.runtime.default".to_owned(),
            time: TimeSnapshotV1::default(),
            observer_position: Vec3Dto::zero(),
            observer_cell: None,
            active_region: None,
            active_biome: None,
            resident_cells: Vec::new(),
            environment_profile: EnvironmentProfileRefDto {
                profile_id: "environment.default".to_owned(),
            },
            seed: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentVisualAssetRefsDto {
    pub visual_group_id: String,
    pub texture_dictionary_ref: String,
    pub sky_texture_ref: String,
    pub starfield_texture_ref: String,
    pub cloud_field_ref: String,
    pub cloud_density_texture_ref: String,
    pub cloud_detail_texture_ref: String,
    pub cloud_dither_texture_ref: String,
    pub sun_disk_texture_ref: String,
    pub moon_disk_texture_ref: String,
    pub weather_table_ref: String,
    pub weather_visual_ref: String,
}

impl Default for EnvironmentVisualAssetRefsDto {
    #[inline]
    fn default() -> Self {
        Self {
            visual_group_id: String::new(),
            texture_dictionary_ref: String::new(),
            sky_texture_ref: String::new(),
            starfield_texture_ref: String::new(),
            cloud_field_ref: String::new(),
            cloud_density_texture_ref: String::new(),
            cloud_detail_texture_ref: String::new(),
            cloud_dither_texture_ref: String::new(),
            sun_disk_texture_ref: String::new(),
            moon_disk_texture_ref: String::new(),
            weather_table_ref: String::new(),
            weather_visual_ref: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentFrameDto {
    pub frame_id: u64,
    pub world_instance_id: String,
    pub world_time_seconds: f64,
    pub time_of_day_normalized: f32,
    pub day_index: u32,
    #[serde(default)]
    pub time_of_day_state: TimeOfDayStateDto,
    pub global: EnvironmentGlobalStateDto,
    #[serde(default)]
    pub visual_assets: EnvironmentVisualAssetRefsDto,
    pub celestial: CelestialStateDto,
    pub sky: SkyStateDto,
    pub atmosphere: AtmosphereStateDto,
    pub weather: WeatherStateDto,
    pub clouds: CloudStateDto,
    pub wind: WindStateDto,
    pub lighting_intent: EnvironmentLightingIntentDto,
    pub gameplay_modifiers: EnvironmentGameplayModifiersDto,
    pub exposure_intent: ExposureIntentDto,
    #[serde(default)]
    pub environment_objects: Vec<EnvironmentObjectDto>,
    #[serde(default)]
    pub consumer_packets: EnvironmentConsumerPacketsDto,
    pub diagnostics: EnvironmentDiagnosticsDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentGlobalStateDto {
    pub active_region: Option<String>,
    pub active_biome: Option<String>,
    pub active_weather_profile: String,
    pub active_environment_profile: String,
    #[serde(default)]
    pub weather_table_ref: String,
    #[serde(default)]
    pub sky_profile_ref: String,
    #[serde(default)]
    pub cloud_profile_ref: String,
    #[serde(default)]
    pub wind_profile_ref: String,
    pub environment_seed: u64,
}

impl Default for EnvironmentGlobalStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            active_region: None,
            active_biome: None,
            active_weather_profile: "weather.clear".to_owned(),
            active_environment_profile: "environment.default".to_owned(),
            weather_table_ref: String::new(),
            sky_profile_ref: String::new(),
            cloud_profile_ref: String::new(),
            wind_profile_ref: String::new(),
            environment_seed: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CelestialBodyDto {
    pub direction_world: Vec3Dto,
    pub altitude_radians: f32,
    pub azimuth_radians: f32,
    pub angular_radius_radians: f32,
    pub color_linear: Color3Dto,
    pub intensity_lux_hint: f32,
    pub visible: bool,
}

impl Default for CelestialBodyDto {
    #[inline]
    fn default() -> Self {
        Self {
            direction_world: Vec3Dto::up(),
            altitude_radians: 0.0,
            azimuth_radians: 0.0,
            angular_radius_radians: 0.00465,
            color_linear: Color3Dto::white(),
            intensity_lux_hint: 0.0,
            visible: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CelestialStateDto {
    pub sun: CelestialBodyDto,
    pub moon: CelestialBodyDto,
    pub moon_phase: f32,
    pub stars_visibility: f32,
    pub night_sky_visibility: f32,
}

impl Default for CelestialStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            sun: CelestialBodyDto::default(),
            moon: CelestialBodyDto::default(),
            moon_phase: 0.5,
            stars_visibility: 0.0,
            night_sky_visibility: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SkyStateDto {
    pub zenith_color_linear: Color3Dto,
    pub horizon_color_linear: Color3Dto,
    pub sun_horizon_color_linear: Color3Dto,
    pub opposite_horizon_color_linear: Color3Dto,
    pub dusk_dawn_blend: f32,
    pub night_blend: f32,
    pub overcast_blend: f32,
    pub light_pollution: f32,
}

impl Default for SkyStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            zenith_color_linear: Color3Dto::new(0.10, 0.15, 0.24),
            horizon_color_linear: Color3Dto::new(0.12, 0.14, 0.18),
            sun_horizon_color_linear: Color3Dto::new(0.20, 0.16, 0.12),
            opposite_horizon_color_linear: Color3Dto::new(0.08, 0.10, 0.14),
            dusk_dawn_blend: 0.0,
            night_blend: 1.0,
            overcast_blend: 0.0,
            light_pollution: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AtmosphereStateDto {
    pub fog_density: f32,
    pub fog_height_falloff: f32,
    pub fog_color_linear: Color3Dto,
    pub haze_amount: f32,
    pub humidity: f32,
    pub aerosol_density: f32,
    pub visibility_distance_meters: f32,
}

impl Default for AtmosphereStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            fog_density: 0.0,
            fog_height_falloff: 0.12,
            fog_color_linear: Color3Dto::new(0.45, 0.50, 0.58),
            haze_amount: 0.05,
            humidity: 0.25,
            aerosol_density: 0.10,
            visibility_distance_meters: 20_000.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PrecipitationStateDto {
    pub kind: PrecipitationKind,
    pub intensity: f32,
}

impl Default for PrecipitationStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            kind: PrecipitationKind::None,
            intensity: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ThunderStateDto {
    pub probability: f32,
    pub distance_meters: f32,
}

impl Default for ThunderStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            probability: 0.0,
            distance_meters: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WetnessStateDto {
    pub surface_wetness: f32,
    pub accumulation_rate: f32,
    pub drying_rate: f32,
}

impl Default for WetnessStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            surface_wetness: 0.0,
            accumulation_rate: 0.0,
            drying_rate: 0.02,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SnowStateDto {
    pub surface_snow: f32,
    pub accumulation_rate: f32,
    pub melt_rate: f32,
}

impl Default for SnowStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            surface_snow: 0.0,
            accumulation_rate: 0.0,
            melt_rate: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherStateDto {
    pub weather_id: String,
    pub state: WeatherKind,
    pub intensity: f32,
    pub transition_progress: f32,
    pub precipitation: PrecipitationStateDto,
    pub thunder: ThunderStateDto,
    pub wetness: WetnessStateDto,
    pub snow: SnowStateDto,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Default for WeatherStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            weather_id: "weather.clear".to_owned(),
            state: WeatherKind::Clear,
            intensity: 0.0,
            transition_progress: 1.0,
            precipitation: PrecipitationStateDto::default(),
            thunder: ThunderStateDto::default(),
            wetness: WetnessStateDto::default(),
            snow: SnowStateDto::default(),
            tags: vec!["weather.clear".to_owned(), "visibility.normal".to_owned()],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CloudLayerDto {
    pub altitude_min_meters: f32,
    pub altitude_max_meters: f32,
    pub coverage: f32,
    pub density: f32,
    pub wind_velocity: Vec3Dto,
}

impl Default for CloudLayerDto {
    #[inline]
    fn default() -> Self {
        Self {
            altitude_min_meters: 1800.0,
            altitude_max_meters: 3200.0,
            coverage: 0.15,
            density: 0.20,
            wind_velocity: Vec3Dto::new(1.0, 0.0, 0.35),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CloudStateDto {
    pub coverage: f32,
    pub overcast: f32,
    pub shadow_strength: f32,
    pub light_absorption: f32,
    pub layers: Vec<CloudLayerDto>,
    pub volumes: Vec<EnvironmentObjectDto>,
    pub storm_cells: Vec<EnvironmentObjectDto>,
}

impl Default for CloudStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            coverage: 0.15,
            overcast: 0.0,
            shadow_strength: 0.05,
            light_absorption: 0.04,
            layers: vec![CloudLayerDto::default()],
            volumes: Vec::new(),
            storm_cells: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindStateDto {
    pub global_direction: Vec3Dto,
    pub global_speed_mps: f32,
    pub gust_strength: f32,
    pub cloud_advection: Vec3Dto,
}

impl Default for WindStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            global_direction: Vec3Dto::new(1.0, 0.0, 0.35),
            global_speed_mps: 2.0,
            gust_strength: 0.1,
            cloud_advection: Vec3Dto::new(2.0, 0.0, 0.7),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentLightingIntentDto {
    pub sun_lux_hint: f32,
    pub moon_lux_hint: f32,
    pub ambient_intensity: f32,
    pub sky_light_intensity: f32,
    pub cloud_shadow_strength: f32,
    pub wetness_specular_boost: f32,
}

impl Default for EnvironmentLightingIntentDto {
    #[inline]
    fn default() -> Self {
        Self {
            sun_lux_hint: 0.0,
            moon_lux_hint: 0.0,
            ambient_intensity: 0.05,
            sky_light_intensity: 0.05,
            cloud_shadow_strength: 0.0,
            wetness_specular_boost: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ExposureIntentDto {
    pub night_adaptation_hint: f32,
    pub storm_darkening: f32,
    pub sun_glare_hint: f32,
    pub interior_exterior_bias: f32,
}

impl Default for ExposureIntentDto {
    #[inline]
    fn default() -> Self {
        Self {
            night_adaptation_hint: 1.0,
            storm_darkening: 0.0,
            sun_glare_hint: 0.0,
            interior_exterior_bias: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentGameplayModifiersDto {
    pub visibility_multiplier: f32,
    pub audio_masking_multiplier: f32,
    pub weather_hazard_level: f32,
    pub shelter_score: f32,
    pub surface_slipperiness_hint: f32,
}

impl Default for EnvironmentGameplayModifiersDto {
    #[inline]
    fn default() -> Self {
        Self {
            visibility_multiplier: 1.0,
            audio_masking_multiplier: 0.0,
            weather_hazard_level: 0.0,
            shelter_score: 0.0,
            surface_slipperiness_hint: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentObjectDto {
    pub id: EnvironmentObjectId,
    pub kind: EnvironmentObjectKind,
    pub bounds: AabbDto,
    pub owning_cells: Vec<WorldCellCoord>,
    pub transform: TransformDto,
    pub tags: Vec<String>,
    pub state_json: serde_json::Value,
}

impl Default for EnvironmentObjectDto {
    #[inline]
    fn default() -> Self {
        Self {
            id: EnvironmentObjectId::default(),
            kind: EnvironmentObjectKind::CloudField,
            bounds: AabbDto::default(),
            owning_cells: Vec::new(),
            transform: TransformDto::default(),
            tags: Vec::new(),
            state_json: serde_json::Value::Null,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentDiagnosticsDto {
    pub provider: String,
    pub provider_route: String,
    pub degraded: bool,
    pub deterministic_key: String,
    pub active_profile: String,
    pub reasons: Vec<String>,
    pub warnings: Vec<String>,
}

impl Default for EnvironmentDiagnosticsDto {
    #[inline]
    fn default() -> Self {
        Self {
            provider: "environment.default".to_owned(),
            provider_route: "engine.world.default.environment".to_owned(),
            degraded: false,
            deterministic_key: String::new(),
            active_profile: "environment.default".to_owned(),
            reasons: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentResidencyIntentDto {
    pub object_id: EnvironmentObjectId,
    pub owning_cells: Vec<WorldCellCoord>,
    pub required_assets: Vec<String>,
    pub priority: String,
    pub reason: String,
}

impl Default for EnvironmentResidencyIntentDto {
    #[inline]
    fn default() -> Self {
        Self {
            object_id: EnvironmentObjectId::default(),
            owning_cells: Vec::new(),
            required_assets: Vec::new(),
            priority: "background".to_owned(),
            reason: "environment".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RenderEnvironmentPacketDto {
    pub visual_group_id: String,
    pub texture_dictionary_ref: String,
    pub sky_texture_ref: String,
    pub starfield_texture_ref: String,
    pub cloud_field_ref: String,
    pub cloud_density_texture_ref: String,
    pub cloud_detail_texture_ref: String,
    pub cloud_dither_texture_ref: String,
    pub sun_disk_texture_ref: String,
    pub moon_disk_texture_ref: String,
    pub weather_visual_ref: String,
    pub sun_direction: Vec3Dto,
    pub sun_color_linear: Color3Dto,
    pub sun_intensity_hint: f32,
    pub moon_direction: Vec3Dto,
    pub moon_color_linear: Color3Dto,
    pub moon_intensity_hint: f32,
    pub fog_density: f32,
    pub fog_color_linear: Color3Dto,
    pub cloud_coverage: f32,
    pub cloud_shadow_strength: f32,
    pub exposure: ExposureIntentDto,
}

impl Default for RenderEnvironmentPacketDto {
    #[inline]
    fn default() -> Self {
        Self {
            visual_group_id: String::new(),
            texture_dictionary_ref: String::new(),
            sky_texture_ref: String::new(),
            starfield_texture_ref: String::new(),
            cloud_field_ref: String::new(),
            cloud_density_texture_ref: String::new(),
            cloud_detail_texture_ref: String::new(),
            cloud_dither_texture_ref: String::new(),
            sun_disk_texture_ref: String::new(),
            moon_disk_texture_ref: String::new(),
            weather_visual_ref: String::new(),
            sun_direction: Vec3Dto::up(),
            sun_color_linear: Color3Dto::white(),
            sun_intensity_hint: 0.0,
            moon_direction: Vec3Dto::up(),
            moon_color_linear: Color3Dto::new(0.58, 0.66, 0.86),
            moon_intensity_hint: 0.0,
            fog_density: 0.0,
            fog_color_linear: Color3Dto::new(0.45, 0.50, 0.58),
            cloud_coverage: 0.0,
            cloud_shadow_strength: 0.0,
            exposure: ExposureIntentDto::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiEnvironmentObservationDto {
    pub time_of_day_normalized: f32,
    pub phase: TimeOfDayPhase,
    pub is_night: bool,
    pub visibility_multiplier: f32,
    pub audio_masking_multiplier: f32,
    pub weather_hazard_level: f32,
    pub shelter_score: f32,
    pub tags: Vec<String>,
}

impl Default for AiEnvironmentObservationDto {
    #[inline]
    fn default() -> Self {
        Self {
            time_of_day_normalized: 0.0,
            phase: TimeOfDayPhase::Night,
            is_night: true,
            visibility_multiplier: 1.0,
            audio_masking_multiplier: 0.0,
            weather_hazard_level: 0.0,
            shelter_score: 0.0,
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsEnvironmentIntentDto {
    pub wind_velocity: Vec3Dto,
    pub gust_strength: f32,
    pub precipitation_intensity: f32,
    pub wetness_accumulation_rate: f32,
    pub snow_accumulation_rate: f32,
    pub surface_slipperiness_hint: f32,
}

impl Default for PhysicsEnvironmentIntentDto {
    #[inline]
    fn default() -> Self {
        Self {
            wind_velocity: Vec3Dto::zero(),
            gust_strength: 0.0,
            precipitation_intensity: 0.0,
            wetness_accumulation_rate: 0.0,
            snow_accumulation_rate: 0.0,
            surface_slipperiness_hint: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioEnvironmentPacketDto {
    pub rain_intensity: f32,
    pub wind_speed: f32,
    pub thunder_probability: f32,
    pub storm_distance: f32,
    pub indoor_occlusion_hint: f32,
    pub ambience_tags: Vec<String>,
}

impl Default for AudioEnvironmentPacketDto {
    #[inline]
    fn default() -> Self {
        Self {
            rain_intensity: 0.0,
            wind_speed: 0.0,
            thunder_probability: 0.0,
            storm_distance: 0.0,
            indoor_occlusion_hint: 0.0,
            ambience_tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamingEnvironmentHintsDto {
    pub residency_intents: Vec<EnvironmentResidencyIntentDto>,
    pub affected_cells: Vec<WorldCellCoord>,
    pub tags: Vec<String>,
}

impl Default for StreamingEnvironmentHintsDto {
    #[inline]
    fn default() -> Self {
        Self {
            residency_intents: Vec::new(),
            affected_cells: Vec::new(),
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentConsumerPacketsDto {
    pub render: RenderEnvironmentPacketDto,
    pub ai: AiEnvironmentObservationDto,
    pub physics: PhysicsEnvironmentIntentDto,
    pub audio: AudioEnvironmentPacketDto,
    pub streaming: StreamingEnvironmentHintsDto,
}

impl Default for EnvironmentConsumerPacketsDto {
    #[inline]
    fn default() -> Self {
        Self {
            render: RenderEnvironmentPacketDto::default(),
            ai: AiEnvironmentObservationDto::default(),
            physics: PhysicsEnvironmentIntentDto::default(),
            audio: AudioEnvironmentPacketDto::default(),
            streaming: StreamingEnvironmentHintsDto::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentSampleAtPositionRequest {
    pub frame: EnvironmentFrameDto,
    pub position: Vec3Dto,
    pub cell: Option<WorldCellCoord>,
}

impl Default for EnvironmentSampleAtPositionRequest {
    #[inline]
    fn default() -> Self {
        Self {
            frame: EnvironmentFrameDto::neutral_degraded(
                0,
                "world.runtime.default",
                "environment.sample.default",
            ),
            position: Vec3Dto::zero(),
            cell: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentSampleAtPositionResponse {
    pub position: Vec3Dto,
    pub cell: Option<WorldCellCoord>,
    pub visibility_multiplier: f32,
    pub wind_velocity: Vec3Dto,
    pub weather_tags: Vec<String>,
    pub diagnostics: EnvironmentDiagnosticsDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentSnapshotRequest {
    pub include_objects: bool,
}

impl Default for EnvironmentSnapshotRequest {
    #[inline]
    fn default() -> Self {
        Self {
            include_objects: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentSnapshotResponse {
    pub schema: String,
    pub frame: EnvironmentFrameDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentRestoreRequest {
    pub snapshot: EnvironmentSnapshotResponse,
}

impl Default for EnvironmentRestoreRequest {
    #[inline]
    fn default() -> Self {
        Self {
            snapshot: EnvironmentSnapshotResponse::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentRestoreResponse {
    pub ok: bool,
    pub frame: EnvironmentFrameDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentPreviewTimeRequest {
    pub base_request: EnvironmentFrameRequest,
    pub normalized_time_of_day: f32,
}

impl Default for EnvironmentPreviewTimeRequest {
    #[inline]
    fn default() -> Self {
        Self {
            base_request: EnvironmentFrameRequest::default(),
            normalized_time_of_day: 0.5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentInvokeRequest {
    pub method: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

impl EnvironmentFrameDto {
    #[inline]
    pub fn neutral_degraded(
        frame_id: u64,
        world_instance_id: impl Into<String>,
        key: impl Into<String>,
    ) -> Self {
        let world_instance_id = world_instance_id.into();
        let key = key.into();
        Self {
            frame_id,
            world_instance_id,
            world_time_seconds: 0.0,
            time_of_day_normalized: 0.0,
            day_index: 0,
            time_of_day_state: TimeOfDayStateDto::default(),
            global: EnvironmentGlobalStateDto::default(),
            visual_assets: EnvironmentVisualAssetRefsDto::default(),
            celestial: CelestialStateDto::default(),
            sky: SkyStateDto::default(),
            atmosphere: AtmosphereStateDto::default(),
            weather: WeatherStateDto::default(),
            clouds: CloudStateDto {
                coverage: 0.0,
                overcast: 0.0,
                shadow_strength: 0.0,
                light_absorption: 0.0,
                layers: Vec::new(),
                volumes: Vec::new(),
                storm_cells: Vec::new(),
            },
            wind: WindStateDto {
                global_direction: Vec3Dto::zero(),
                global_speed_mps: 0.0,
                gust_strength: 0.0,
                cloud_advection: Vec3Dto::zero(),
            },
            lighting_intent: EnvironmentLightingIntentDto::default(),
            gameplay_modifiers: EnvironmentGameplayModifiersDto::default(),
            exposure_intent: ExposureIntentDto::default(),
            environment_objects: Vec::new(),
            consumer_packets: EnvironmentConsumerPacketsDto::default(),
            diagnostics: EnvironmentDiagnosticsDto {
                provider: "environment.null".to_owned(),
                provider_route: "engine.world.null.environment".to_owned(),
                degraded: true,
                deterministic_key: key,
                active_profile: "environment.null".to_owned(),
                reasons: vec!["neutral degraded environment frame".to_owned()],
                warnings: Vec::new(),
            },
        }
    }
}

impl Default for EnvironmentFrameDto {
    #[inline]
    fn default() -> Self {
        Self::neutral_degraded(0, "world.runtime.default", "environment.default")
    }
}

impl Default for EnvironmentSnapshotResponse {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.world.environment.snapshot.v1".to_owned(),
            frame: EnvironmentFrameDto::default(),
        }
    }
}

/// Thin host-side client over `engine.world.environment`.
#[derive(Clone)]
pub struct EnvironmentClient {
    host: HostApiV1,
    service_id: RString,
}

impl EnvironmentClient {
    #[inline]
    pub fn new(host: HostApiV1) -> Self {
        Self {
            host,
            service_id: RString::from(ENGINE_WORLD_ENVIRONMENT_SERVICE_ID),
        }
    }

    #[inline]
    fn call_json(
        &self,
        method: &str,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let payload = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
        let res = (self.host.call_service_v1)(
            self.service_id.clone(),
            MethodName::from(method),
            Blob::from(payload),
        );
        let bytes = res.into_result().map_err(|e| e.to_string())?.into_vec();
        serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|e| e.to_string())
    }

    #[inline]
    pub fn frame_json_v1(
        &self,
        request: EnvironmentFrameRequest,
    ) -> Result<EnvironmentFrameDto, String> {
        let value = serde_json::to_value(request).map_err(|e| e.to_string())?;
        let response = self.call_json(WORLD_ENVIRONMENT_SERVICE_METHOD_FRAME_JSON_V1, value)?;
        serde_json::from_value(response).map_err(|e| e.to_string())
    }

    #[inline]
    pub fn snapshot_json_v1(&self) -> Result<EnvironmentSnapshotResponse, String> {
        let response = self.call_json(
            WORLD_ENVIRONMENT_SERVICE_METHOD_SNAPSHOT_JSON_V1,
            serde_json::json!({ "include_objects": true }),
        )?;
        serde_json::from_value(response).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_service_ids_are_world_subdomain_gateway_first() {
        assert_eq!(
            ENGINE_WORLD_ENVIRONMENT_SERVICE_ID,
            "engine.world.environment"
        );
        assert_eq!(
            WORLD_ENVIRONMENT_BACKEND_SERVICE_SPEC.engine_gateway_id,
            ENGINE_WORLD_ENVIRONMENT_SERVICE_ID
        );
        assert_eq!(
            WORLD_ENVIRONMENT_BACKEND_SERVICE_SPEC.provider_service_id,
            WORLD_ENVIRONMENT_DEFAULT_SERVICE_ID
        );
        assert_eq!(
            WORLD_ENVIRONMENT_BACKEND_SERVICE_SPEC.backend_capability_id,
            WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID
        );
    }

    #[test]
    fn environment_request_has_no_renderer_state() {
        let json = serde_json::to_value(EnvironmentFrameRequest::default()).unwrap();
        assert!(json.get("time").is_some());
        assert!(json.get("observer_position").is_some());
        assert!(json.get("vulkan").is_none());
        assert!(json.get("gpu_cloud_history").is_none());
        assert!(json.get("renderer_exposure_buffer").is_none());
    }

    #[test]
    fn environment_frame_has_consumer_packets_without_renderer_ownership() {
        let frame = EnvironmentFrameDto::default();
        assert_eq!(frame.consumer_packets.render.cloud_coverage, 0.0);
        assert!(frame.consumer_packets.ai.is_night);
        assert_eq!(frame.consumer_packets.physics.precipitation_intensity, 0.0);
    }
}
