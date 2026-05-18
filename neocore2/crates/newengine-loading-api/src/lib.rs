#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

/// Engine-facing loading-screen gateway id.
///
/// Runtime/platform code calls this stable facade. The current implementation is
/// an engine-owned native loading shell bridge, while future profile/plugin
/// providers may expose their own `loading.api` backend behind the same gateway.
pub const ENGINE_LOADING_SERVICE_ID: &str = "engine.loading";

/// First-party provider service id for loading shell providers.
pub const LOADING_SERVICE_ID: &str = "loading.api";
pub const LOADING_BACKEND_CAPABILITY_ID: &str = "loading.backend";

pub const LOADING_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const LOADING_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const LOADING_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const LOADING_SERVICE_METHOD_SNAPSHOT_JSON_V1: &str = "snapshot_json_v1";
pub const LOADING_SERVICE_METHOD_PUBLISH_JSON_V1: &str = "publish_json_v1";

pub const LOADING_REQUIRED_METHODS_V1: &[&str] = &[
    LOADING_SERVICE_METHOD_INFO,
    LOADING_SERVICE_METHOD_INVOKE,
    LOADING_SERVICE_METHOD_SHUTDOWN_V1,
    LOADING_SERVICE_METHOD_SNAPSHOT_JSON_V1,
];

pub const LOADING_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "loading",
        ENGINE_LOADING_SERVICE_ID,
        LOADING_SERVICE_ID,
        LOADING_BACKEND_CAPABILITY_ID,
    );

pub const LOADING_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_LOADING_SERVICE_ID,
        "newengine.loading-api >= 0.1.x",
        LOADING_REQUIRED_METHODS_V1,
    );

/// Loading is an always-helpful diagnostic domain, but it must not make headless
/// or test runs fatal unless a strict profile explicitly requires it.
pub const LOADING_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        LOADING_RUNTIME_CONTRACT_SPEC,
        Some(LOADING_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_LOADING_BACKEND"),
    );

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadingServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl Default for LoadingServiceInfo {
    #[inline]
    fn default() -> Self {
        Self {
            protocol: "newengine.loading-api/v1".to_owned(),
            features: vec![
                "engine-owned-native-shell".to_owned(),
                "shared-snapshot".to_owned(),
                "independent-visual-clock".to_owned(),
                "subsystem-stage-projection".to_owned(),
            ],
            methods: LOADING_REQUIRED_METHODS_V1
                .iter()
                .map(|it| (*it).to_owned())
                .chain(std::iter::once(LOADING_SERVICE_METHOD_PUBLISH_JSON_V1.to_owned()))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadingSubsystemPhase {
    Waiting,
    Running,
    Ready,
    Degraded,
    Failed,
}

impl Default for LoadingSubsystemPhase {
    #[inline]
    fn default() -> Self {
        Self::Waiting
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadingSubsystemSnapshot {
    pub id: String,
    pub label: String,
    pub phase: LoadingSubsystemPhase,
    pub state_label: String,
    pub detail: String,
    pub progress_01: Option<f32>,
}

impl LoadingSubsystemSnapshot {
    #[inline]
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        phase: LoadingSubsystemPhase,
        state_label: impl Into<String>,
        detail: impl Into<String>,
        progress_01: Option<f32>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            phase,
            state_label: state_label.into(),
            detail: detail.into(),
            progress_01: progress_01.map(|v| v.clamp(0.0, 1.0)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadingScreenSnapshot {
    pub active: bool,
    pub title: String,
    pub status: String,
    pub detail: String,
    pub progress_01: f32,
    pub spinner_phase: u32,
    pub source: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub view_json: String,
    #[serde(default)]
    pub subsystems: Vec<LoadingSubsystemSnapshot>,
}

impl Default for LoadingScreenSnapshot {
    #[inline]
    fn default() -> Self {
        Self {
            active: false,
            title: "NEWENGINE // BOOTSTRAP".to_owned(),
            status: "Preparing runtime...".to_owned(),
            detail: "The native loading shell is waiting for startup telemetry.".to_owned(),
            progress_01: 0.0,
            spinner_phase: 0,
            source: "engine.loading".to_owned(),
            provider: "native-shell".to_owned(),
            view_json: String::new(),
            subsystems: Vec::new(),
        }
    }
}

impl LoadingScreenSnapshot {
    #[inline]
    pub fn inactive() -> Self {
        Self::default()
    }

    #[inline]
    pub fn normalize(mut self) -> Self {
        self.progress_01 = self.progress_01.clamp(0.0, 1.0);
        for subsystem in &mut self.subsystems {
            subsystem.progress_01 = subsystem.progress_01.map(|v| v.clamp(0.0, 1.0));
        }
        self
    }
}
