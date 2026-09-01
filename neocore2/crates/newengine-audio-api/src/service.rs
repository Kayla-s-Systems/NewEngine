use super::*;

/// Engine-facing audio gateway id. Runtime consumers call this facade; the host
/// resolves it to the active audio provider by descriptor metadata.
pub const ENGINE_AUDIO_SERVICE_ID: &str = "engine.audio";

/// First-party/default provider service id for audio backends.
pub const AUDIO_SERVICE_ID: &str = "audio.api";
pub const AUDIO_BACKEND_CAPABILITY_ID: &str = "audio.backend";
pub const AUDIO_PROVIDER_ABI_VERSION: u16 = 1;
pub const AUDIO_PROVIDER_ABI_ID: &str = "newengine.audio.provider.v1";
pub const AUDIO_PROVIDER_ABI_CONTRACT_SPEC: newengine_contract_api::ContractSpec =
    newengine_contract_api::ContractSpec::new(
        "audio.provider.abi",
        newengine_contract_api::ContractKind::Abi,
        newengine_contract_api::ContractVersion::major(AUDIO_PROVIDER_ABI_VERSION),
        newengine_contract_api::ContractCompatibility::Exact,
        "newengine-audio-api",
        Some(AUDIO_PROVIDER_ABI_ID),
    );
pub const AUDIO_EMITTER_COMPONENT_TYPE: &str = "audio.emitter";
pub const AUDIO_ACOUSTIC_SURFACE_COMPONENT_TYPE: &str = "audio.acoustic_surface";

pub const AUDIO_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const AUDIO_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const AUDIO_SERVICE_METHOD_SHUTDOWN_V1: &str =
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1: &str = "play_event_json_v1";
pub const AUDIO_SERVICE_METHOD_DRAIN_EVENTS_JSON_V1: &str = "drain_events_json_v1";
pub const AUDIO_SERVICE_METHOD_PRELOAD_CLIP_JSON_V1: &str = "preload_clip_json_v1";
pub const AUDIO_SERVICE_METHOD_PRELOAD_CUE_JSON_V1: &str = "preload_cue_json_v1";
pub const AUDIO_SERVICE_METHOD_PLAY_CUE_JSON_V1: &str = "play_cue_json_v1";
pub const AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1: &str = "play_clip_json_v1";
pub const AUDIO_SERVICE_METHOD_PLAY_STREAM_JSON_V1: &str = "play_stream_json_v1";
pub const AUDIO_SERVICE_METHOD_STOP_VOICE_JSON_V1: &str = "stop_voice_json_v1";
pub const AUDIO_SERVICE_METHOD_SET_VOICE_JSON_V1: &str = "set_voice_json_v1";
pub const AUDIO_SERVICE_METHOD_SET_LISTENER_JSON_V1: &str = "set_listener_json_v1";
pub const AUDIO_SERVICE_METHOD_SET_ROUTE_GAIN_JSON_V1: &str = "set_route_gain_json_v1";
pub const AUDIO_SERVICE_METHOD_SET_VOICE_BUDGETS_JSON_V1: &str = "set_voice_budgets_json_v1";
pub const AUDIO_SERVICE_METHOD_DIAGNOSTICS_JSON_V1: &str = "diagnostics_json_v1";
pub const AUDIO_SERVICE_METHOD_RENDER_CLOCK_JSON_V1: &str = "render_clock_json_v1";
pub const AUDIO_SERVICE_METHOD_SCHEDULE_VOICE_RENDER_JSON_V1: &str =
    "schedule_voice_render_json_v1";

pub const AUDIO_REQUIRED_METHODS_V1: &[&str] = &[
    AUDIO_SERVICE_METHOD_INFO,
    AUDIO_SERVICE_METHOD_INVOKE,
    AUDIO_SERVICE_METHOD_SHUTDOWN_V1,
    AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1,
];

pub const AUDIO_PLAYBACK_METHODS_V1: &[&str] = &[
    AUDIO_SERVICE_METHOD_PRELOAD_CLIP_JSON_V1,
    AUDIO_SERVICE_METHOD_PRELOAD_CUE_JSON_V1,
    AUDIO_SERVICE_METHOD_PLAY_CUE_JSON_V1,
    AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1,
    AUDIO_SERVICE_METHOD_PLAY_STREAM_JSON_V1,
    AUDIO_SERVICE_METHOD_STOP_VOICE_JSON_V1,
    AUDIO_SERVICE_METHOD_SET_VOICE_JSON_V1,
    AUDIO_SERVICE_METHOD_SET_LISTENER_JSON_V1,
    AUDIO_SERVICE_METHOD_SET_ROUTE_GAIN_JSON_V1,
    AUDIO_SERVICE_METHOD_DIAGNOSTICS_JSON_V1,
];

pub const AUDIO_VOICE_POLICY_METHODS_V2: &[&str] =
    &[AUDIO_SERVICE_METHOD_SET_VOICE_BUDGETS_JSON_V1];

pub const AUDIO_BLOCK_RENDER_METHODS_V1: &[&str] = &[
    AUDIO_SERVICE_METHOD_RENDER_CLOCK_JSON_V1,
    AUDIO_SERVICE_METHOD_SCHEDULE_VOICE_RENDER_JSON_V1,
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
/// requires it. The engine-runtime queue provider gives UI/gameplay systems a
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
    pub provider: String,
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
            provider: "engine.audio.echo-event-queue".to_owned(),
            features: vec![
                "semantic-feedback-events".to_owned(),
                "ui-feedback".to_owned(),
                "engine.audio.echo-event-queue".to_owned(),
                "plugin-override-ready".to_owned(),
            ],
            methods: AUDIO_REQUIRED_METHODS_V1
                .iter()
                .map(|it| (*it).to_owned())
                .chain(std::iter::once(
                    AUDIO_SERVICE_METHOD_DRAIN_EVENTS_JSON_V1.to_owned(),
                ))
                .collect(),
        }
    }
}

impl AudioServiceInfo {
    #[inline]
    pub fn supports_method(&self, method: &str) -> bool {
        self.methods.iter().any(|candidate| candidate == method)
    }

    #[inline]
    pub fn supports_playback(&self) -> bool {
        AUDIO_PLAYBACK_METHODS_V1
            .iter()
            .all(|method| self.supports_method(method))
    }

    pub fn playback_provider(provider: impl Into<String>) -> Self {
        let mut methods = AUDIO_REQUIRED_METHODS_V1
            .iter()
            .chain(AUDIO_PLAYBACK_METHODS_V1.iter())
            .chain(AUDIO_VOICE_POLICY_METHODS_V2.iter())
            .chain(AUDIO_BLOCK_RENDER_METHODS_V1.iter())
            .map(|method| (*method).to_owned())
            .collect::<Vec<_>>();
        methods.sort();
        methods.dedup();
        Self {
            protocol: "newengine.audio-api/playback-v1".to_owned(),
            provider: provider.into(),
            features: vec![
                "semantic-feedback-events".to_owned(),
                "clip-playback".to_owned(),
                "voice-control".to_owned(),
                "audio-buses".to_owned(),
                "listener-state".to_owned(),
                "spatial-audio".to_owned(),
                "clip-cache".to_owned(),
                "voice-budget".to_owned(),
                "voice-policy-v2".to_owned(),
                "reserved-voice-budgets".to_owned(),
                "voice-virtualization".to_owned(),
                "stream-logical-virtualization".to_owned(),
                "block-native-render-graph".to_owned(),
                "sample-addressed-render-scheduling".to_owned(),
                "yscd-sound-graph-v1".to_owned(),
                "sound-graph-trigger-parameters".to_owned(),
                "block-based-native-render-graph".to_owned(),
                "single-master-output".to_owned(),
                "sample-accurate-render-scheduling".to_owned(),
                "authored-attenuation".to_owned(),
                "environment-state".to_owned(),
                "reverb-sends".to_owned(),
                "streaming-playback".to_owned(),
                "pcm-ring-buffer".to_owned(),
                "long-form-audio".to_owned(),
            ],
            methods,
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
        self.intensity = sanitize_unit(intensity);
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
fn default_intensity() -> f32 {
    1.0
}
