#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

/// Engine-facing audio gateway id. Runtime consumers call this facade; the host
/// resolves it to the active audio provider by descriptor metadata.
pub const ENGINE_AUDIO_SERVICE_ID: &str = "engine.audio";

/// First-party/default provider service id for audio backends.
pub const AUDIO_SERVICE_ID: &str = "audio.api";
pub const AUDIO_BACKEND_CAPABILITY_ID: &str = "audio.backend";
pub const AUDIO_PROVIDER_ABI_ID: &str = "newengine.audio.provider.v1";

pub const AUDIO_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const AUDIO_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const AUDIO_SERVICE_METHOD_SHUTDOWN_V1: &str =
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1: &str = "play_event_json_v1";
pub const AUDIO_SERVICE_METHOD_DRAIN_EVENTS_JSON_V1: &str = "drain_events_json_v1";
pub const AUDIO_SERVICE_METHOD_PRELOAD_CLIP_JSON_V1: &str = "preload_clip_json_v1";
pub const AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1: &str = "play_clip_json_v1";
pub const AUDIO_SERVICE_METHOD_STOP_VOICE_JSON_V1: &str = "stop_voice_json_v1";
pub const AUDIO_SERVICE_METHOD_SET_VOICE_JSON_V1: &str = "set_voice_json_v1";
pub const AUDIO_SERVICE_METHOD_SET_LISTENER_JSON_V1: &str = "set_listener_json_v1";
pub const AUDIO_SERVICE_METHOD_SET_BUS_GAIN_JSON_V1: &str = "set_bus_gain_json_v1";
pub const AUDIO_SERVICE_METHOD_DIAGNOSTICS_JSON_V1: &str = "diagnostics_json_v1";

pub const AUDIO_REQUIRED_METHODS_V1: &[&str] = &[
    AUDIO_SERVICE_METHOD_INFO,
    AUDIO_SERVICE_METHOD_INVOKE,
    AUDIO_SERVICE_METHOD_SHUTDOWN_V1,
    AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1,
];

pub const AUDIO_PLAYBACK_METHODS_V1: &[&str] = &[
    AUDIO_SERVICE_METHOD_PRELOAD_CLIP_JSON_V1,
    AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1,
    AUDIO_SERVICE_METHOD_STOP_VOICE_JSON_V1,
    AUDIO_SERVICE_METHOD_SET_VOICE_JSON_V1,
    AUDIO_SERVICE_METHOD_SET_LISTENER_JSON_V1,
    AUDIO_SERVICE_METHOD_SET_BUS_GAIN_JSON_V1,
    AUDIO_SERVICE_METHOD_DIAGNOSTICS_JSON_V1,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioBus {
    Master,
    Music,
    Sfx,
    Ui,
    Dialogue,
    Ambience,
}

impl Default for AudioBus {
    #[inline]
    fn default() -> Self {
        Self::Sfx
    }
}

impl AudioBus {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Master => "master",
            Self::Music => "music",
            Self::Sfx => "sfx",
            Self::Ui => "ui",
            Self::Dialogue => "dialogue",
            Self::Ambience => "ambience",
        }
    }

    #[inline]
    pub const fn all() -> [Self; 6] {
        [
            Self::Master,
            Self::Music,
            Self::Sfx,
            Self::Ui,
            Self::Dialogue,
            Self::Ambience,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioClipRef {
    /// Runtime-resolved URI/path. The first native provider supports filesystem
    /// paths; asset/VFS URI resolution can be added without changing playback DTOs.
    pub uri: String,
}

impl AudioClipRef {
    #[inline]
    pub fn new(uri: impl Into<String>) -> Self {
        Self { uri: uri.into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AudioSpatialParams {
    pub position: [f32; 3],
}

impl Default for AudioSpatialParams {
    fn default() -> Self {
        Self { position: [0.0; 3] }
    }
}

impl AudioSpatialParams {
    #[inline]
    pub fn sanitized(mut self) -> Self {
        self.position = sanitize_vec3(self.position);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioPlayRequest {
    #[serde(default = "protocol_version_one")]
    pub version: u32,
    pub clip: AudioClipRef,
    #[serde(default)]
    pub bus: AudioBus,
    #[serde(default = "default_gain")]
    pub gain: f32,
    #[serde(default = "default_speed")]
    pub speed: f32,
    #[serde(default)]
    pub looping: bool,
    #[serde(default)]
    pub spatial: Option<AudioSpatialParams>,
}

impl AudioPlayRequest {
    #[inline]
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            version: 1,
            clip: AudioClipRef::new(uri),
            bus: AudioBus::Sfx,
            gain: 1.0,
            speed: 1.0,
            looping: false,
            spatial: None,
        }
    }

    #[inline]
    pub fn sanitized(mut self) -> Self {
        self.gain = sanitize_gain(self.gain);
        self.speed = sanitize_speed(self.speed);
        self.spatial = self.spatial.map(AudioSpatialParams::sanitized);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioPlayAck {
    pub accepted: bool,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub voice_id: Option<u64>,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioPreloadRequest {
    pub clip: AudioClipRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioPreloadAck {
    pub accepted: bool,
    pub cached: bool,
    pub bytes: usize,
    #[serde(default)]
    pub provider: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AudioStopVoiceRequest {
    pub voice_id: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AudioVoiceUpdateRequest {
    pub voice_id: u64,
    #[serde(default)]
    pub gain: Option<f32>,
    #[serde(default)]
    pub speed: Option<f32>,
    #[serde(default)]
    pub paused: Option<bool>,
    #[serde(default)]
    pub position: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioVoiceAck {
    pub accepted: bool,
    pub voice_id: u64,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AudioListenerState {
    pub position: [f32; 3],
    #[serde(default = "default_forward")]
    pub forward: [f32; 3],
    #[serde(default = "default_up")]
    pub up: [f32; 3],
    #[serde(default = "default_ear_distance")]
    pub ear_distance: f32,
}

impl Default for AudioListenerState {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            forward: default_forward(),
            up: default_up(),
            ear_distance: default_ear_distance(),
        }
    }
}

impl AudioListenerState {
    pub fn sanitized(mut self) -> Self {
        self.position = sanitize_vec3(self.position);
        self.forward = normalize_or(self.forward, default_forward());
        self.up = normalize_or(self.up, default_up());
        self.ear_distance = if self.ear_distance.is_finite() {
            self.ear_distance.clamp(0.01, 1.0)
        } else {
            default_ear_distance()
        };
        self
    }

    /// Returns stereo ear positions derived from listener orientation.
    pub fn ear_positions(self) -> ([f32; 3], [f32; 3]) {
        let listener = self.sanitized();
        let right = normalize_or(cross(listener.forward, listener.up), [1.0, 0.0, 0.0]);
        let half = listener.ear_distance * 0.5;
        let left = [
            listener.position[0] - right[0] * half,
            listener.position[1] - right[1] * half,
            listener.position[2] - right[2] * half,
        ];
        let right_ear = [
            listener.position[0] + right[0] * half,
            listener.position[1] + right[1] * half,
            listener.position[2] + right[2] * half,
        ];
        (left, right_ear)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AudioBusGainRequest {
    pub bus: AudioBus,
    pub gain: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioBusGainAck {
    pub accepted: bool,
    pub bus: AudioBus,
    pub gain: f32,
    #[serde(default)]
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDiagnostics {
    pub provider: String,
    pub output_ready: bool,
    pub active_voices: usize,
    pub spatial_voices: usize,
    pub cached_clips: usize,
    pub cached_bytes: usize,
    pub listener: AudioListenerState,
    #[serde(default)]
    pub bus_gains: std::collections::BTreeMap<String, f32>,
}

#[inline]
fn protocol_version_one() -> u32 {
    1
}

#[inline]
fn default_gain() -> f32 {
    1.0
}

#[inline]
fn default_speed() -> f32 {
    1.0
}

#[inline]
fn default_ear_distance() -> f32 {
    0.18
}

#[inline]
fn default_forward() -> [f32; 3] {
    [0.0, 0.0, -1.0]
}

#[inline]
fn default_up() -> [f32; 3] {
    [0.0, 1.0, 0.0]
}

#[inline]
fn sanitize_unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

#[inline]
pub fn sanitize_gain(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 4.0)
    } else {
        1.0
    }
}

#[inline]
pub fn sanitize_speed(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.05, 4.0)
    } else {
        1.0
    }
}

#[inline]
fn sanitize_vec3(value: [f32; 3]) -> [f32; 3] {
    value.map(|component| {
        if component.is_finite() {
            component
        } else {
            0.0
        }
    })
}

#[inline]
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
fn normalize_or(value: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
    let length_sq = value[0] * value[0] + value[1] * value[1] + value[2] * value[2];
    if !length_sq.is_finite() || length_sq <= 1.0e-10 {
        return fallback;
    }
    let inv = length_sq.sqrt().recip();
    [value[0] * inv, value[1] * inv, value[2] * inv]
}

#[cfg(test)]
mod playback_contract_tests {
    use super::*;

    #[test]
    fn playback_provider_exposes_voice_and_spatial_methods() {
        let info = AudioServiceInfo::playback_provider("test.audio");
        assert!(info
            .methods
            .iter()
            .any(|method| method == AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1));
        assert!(info
            .methods
            .iter()
            .any(|method| method == AUDIO_SERVICE_METHOD_SET_LISTENER_JSON_V1));
        assert!(info
            .features
            .iter()
            .any(|feature| feature == "spatial-audio"));
    }

    #[test]
    fn listener_ear_positions_are_centered_and_finite() {
        let (left, right) = AudioListenerState::default().ear_positions();
        assert!(left.iter().copied().all(f32::is_finite));
        assert!(right.iter().copied().all(f32::is_finite));
        assert!((left[0] + right[0]).abs() < 1.0e-6);
        assert!(left[0] < right[0]);
    }

    #[test]
    fn play_request_sanitizes_runtime_controls() {
        let mut request = AudioPlayRequest::new("test.wav");
        request.gain = f32::INFINITY;
        request.speed = -10.0;
        let request = request.sanitized();
        assert_eq!(request.gain, 1.0);
        assert_eq!(request.speed, 0.05);
    }
}
