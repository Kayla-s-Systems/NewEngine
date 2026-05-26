#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

/// Engine-facing audio gateway id. Runtime consumers call this facade; the host
/// resolves it to the active audio provider by descriptor metadata.
pub const ENGINE_AUDIO_SERVICE_ID: &str = "engine.audio";

/// First-party/default provider service id for audio backends.
pub const AUDIO_SERVICE_ID: &str = "audio.api";
pub const AUDIO_BACKEND_CAPABILITY_ID: &str = "audio.backend";

pub const AUDIO_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const AUDIO_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const AUDIO_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1: &str = "play_event_json_v1";
pub const AUDIO_SERVICE_METHOD_DRAIN_EVENTS_JSON_V1: &str = "drain_events_json_v1";

pub const AUDIO_REQUIRED_METHODS_V1: &[&str] = &[
    AUDIO_SERVICE_METHOD_INFO,
    AUDIO_SERVICE_METHOD_INVOKE,
    AUDIO_SERVICE_METHOD_SHUTDOWN_V1,
    AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1,
];

pub const AUDIO_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "audio",
        ENGINE_AUDIO_SERVICE_ID,
        AUDIO_SERVICE_ID,
        AUDIO_BACKEND_CAPABILITY_ID,
    );

pub const AUDIO_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_AUDIO_SERVICE_ID,
        "newengine.audio-api >= 0.1.x",
        AUDIO_REQUIRED_METHODS_V1,
    );

/// Audio is optional for headless/dev runs unless a strict profile explicitly
/// requires it. The engine-owned queue provider gives UI/gameplay systems a
/// stable semantic event sink before a real mixer plugin exists.
pub const AUDIO_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        AUDIO_RUNTIME_CONTRACT_SPEC,
        Some(AUDIO_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_AUDIO_BACKEND"),
    );

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl Default for AudioServiceInfo {
    #[inline]
    fn default() -> Self {
        Self {
            protocol: "newengine.audio-api/v1".to_owned(),
            features: vec![
                "semantic-feedback-events".to_owned(),
                "ui-feedback".to_owned(),
                "engine-owned-event-queue".to_owned(),
                "plugin-override-ready".to_owned(),
            ],
            methods: AUDIO_REQUIRED_METHODS_V1
                .iter()
                .map(|it| (*it).to_owned())
                .chain(std::iter::once(AUDIO_SERVICE_METHOD_DRAIN_EVENTS_JSON_V1.to_owned()))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioFeedbackKind {
    UiOpen,
    UiClose,
    UiNavigate,
    UiConfirm,
    UiBack,
    UiRebind,
    UiError,
}

impl AudioFeedbackKind {
    #[inline]
    pub const fn event_id(self) -> &'static str {
        match self {
            Self::UiOpen => "ui.open",
            Self::UiClose => "ui.close",
            Self::UiNavigate => "ui.navigate",
            Self::UiConfirm => "ui.confirm",
            Self::UiBack => "ui.back",
            Self::UiRebind => "ui.rebind",
            Self::UiError => "ui.error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFeedbackEvent {
    pub version: u32,
    pub id: String,
    pub source: String,
    #[serde(default = "default_intensity")]
    pub intensity: f32,
    #[serde(default)]
    pub frame_index: u64,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl AudioFeedbackEvent {
    #[inline]
    pub fn ui(kind: AudioFeedbackKind, frame_index: u64) -> Self {
        Self {
            version: 1,
            id: kind.event_id().to_owned(),
            source: "engine.ui.primary".to_owned(),
            intensity: default_intensity(),
            frame_index,
            metadata: serde_json::Value::Null,
        }
    }

    #[inline]
    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity.clamp(0.0, 1.0);
        self
    }

    #[inline]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFeedbackAck {
    pub accepted: bool,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub queued_events: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFeedbackDrain {
    pub events: Vec<AudioFeedbackEvent>,
}

#[inline]
fn default_intensity() -> f32 { 1.0 }
