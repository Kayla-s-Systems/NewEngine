use serde::{Deserialize, Serialize};

/// Engine-facing platform service gateway id. Runtime/plugin consumers call
/// this facade; the host resolves it to the active platform provider or route.
pub const ENGINE_PLATFORM_SERVICE_ID: &str = "engine.platform";

/// Default/first-party provider service id for platform backends.
pub const PLATFORM_SERVICE_ID: &str = "platform.api";
pub const PLATFORM_BACKEND_CAPABILITY_ID: &str = "platform.backend";

pub const PLATFORM_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const PLATFORM_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const PLATFORM_SERVICE_METHOD_SHUTDOWN_V1: &str =
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const PLATFORM_SERVICE_METHOD_WINDOW_SNAPSHOT_JSON_V1: &str = "window_snapshot_json_v1";

pub const PLATFORM_REQUIRED_METHODS_V1: &[&str] = &[
    PLATFORM_SERVICE_METHOD_INFO,
    PLATFORM_SERVICE_METHOD_INVOKE,
    PLATFORM_SERVICE_METHOD_SHUTDOWN_V1,
    PLATFORM_SERVICE_METHOD_WINDOW_SNAPSHOT_JSON_V1,
];

pub const PLATFORM_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "platform",
        ENGINE_PLATFORM_SERVICE_ID,
        PLATFORM_SERVICE_ID,
        PLATFORM_BACKEND_CAPABILITY_ID,
    );

pub const PLATFORM_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_PLATFORM_SERVICE_ID,
        "newengine.platform-api >= 0.1.x",
        PLATFORM_REQUIRED_METHODS_V1,
    );

pub const PLATFORM_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        PLATFORM_RUNTIME_CONTRACT_SPEC,
        Some(PLATFORM_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_PLATFORM_BACKEND"),
    );

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl Default for PlatformServiceInfo {
    #[inline]
    fn default() -> Self {
        Self {
            protocol: "newengine.platform-api/v1".to_owned(),
            features: vec![
                "host-owned-window-snapshot".to_owned(),
                "native-window-handles".to_owned(),
                "surface-metrics".to_owned(),
            ],
            methods: PLATFORM_REQUIRED_METHODS_V1
                .iter()
                .map(|method| (*method).to_owned())
                .collect(),
        }
    }
}
