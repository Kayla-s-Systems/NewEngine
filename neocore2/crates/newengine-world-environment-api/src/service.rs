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
/// Read-only compact diagnostics for the current physical atmosphere. No request payload.
pub const WORLD_ENVIRONMENT_SERVICE_METHOD_INSPECT_TEXT_V1: &str = "environment.inspect_text_v1";
pub const WORLD_ENVIRONMENT_SERVICE_METHOD_INSPECT_CELL_TEXT_V1: &str =
    "environment.inspect_cell_text_v1";
pub const WORLD_ENVIRONMENT_SERVICE_METHOD_OBJECTS_TEXT_V1: &str = "environment.objects_text_v1";
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
                .copied()
                .chain([
                    WORLD_ENVIRONMENT_SERVICE_METHOD_INSPECT_TEXT_V1,
                    WORLD_ENVIRONMENT_SERVICE_METHOD_INSPECT_CELL_TEXT_V1,
                    WORLD_ENVIRONMENT_SERVICE_METHOD_OBJECTS_TEXT_V1,
                ])
                .map(str::to_owned)
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
