#![forbid(unsafe_op_in_unsafe_fn)]

mod environment;
mod streaming;
pub use environment::*;
pub use streaming::*;

use serde::{Deserialize, Serialize};

/// Engine-facing audio gateway id. Runtime consumers call this facade; the host
/// resolves it to the active audio provider by descriptor metadata.
pub const ENGINE_AUDIO_SERVICE_ID: &str = "engine.audio";

/// First-party/default provider service id for audio backends.
pub const AUDIO_SERVICE_ID: &str = "audio.api";
pub const AUDIO_BACKEND_CAPABILITY_ID: &str = "audio.backend";
pub const AUDIO_PROVIDER_ABI_ID: &str = "newengine.audio.provider.v1";
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
    AUDIO_SERVICE_METHOD_PRELOAD_CUE_JSON_V1,
    AUDIO_SERVICE_METHOD_PLAY_CUE_JSON_V1,
    AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1,
    AUDIO_SERVICE_METHOD_PLAY_STREAM_JSON_V1,
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
                "voice-budget".to_owned(),
                "voice-virtualization".to_owned(),
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
    /// VFS logical path resolved by `engine.assets`. Physical filesystem paths are
    /// deliberately outside the audio provider contract.
    pub uri: String,
}

impl AudioClipRef {
    #[inline]
    pub fn new(uri: impl Into<String>) -> Self {
        Self { uri: uri.into() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioAttenuationCurve {
    Linear,
    Smoothstep,
    #[default]
    Inverse,
    Exponential,
    Custom,
}

/// Authored distance attenuation policy. Distances are engine world units and
/// custom points use normalized `[distance_fraction, gain]` coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioAttenuationSettings {
    pub min_distance: f32,
    pub max_distance: f32,
    pub curve: AudioAttenuationCurve,
    pub rolloff: f32,
    pub curve_points: Vec<[f32; 2]>,
}

impl Default for AudioAttenuationSettings {
    fn default() -> Self {
        Self {
            min_distance: 1.0,
            max_distance: 50.0,
            curve: AudioAttenuationCurve::Inverse,
            rolloff: 1.0,
            curve_points: Vec::new(),
        }
    }
}

impl AudioAttenuationSettings {
    pub fn sanitized(mut self) -> Self {
        self.min_distance = if self.min_distance.is_finite() {
            self.min_distance.clamp(0.0, 1_000_000.0)
        } else {
            1.0
        };
        self.max_distance = if self.max_distance.is_finite() {
            self.max_distance.clamp(0.01, 1_000_000.0)
        } else {
            50.0
        };
        if self.max_distance <= self.min_distance {
            self.max_distance = (self.min_distance + 0.01).min(1_000_000.0);
            if self.max_distance <= self.min_distance {
                self.min_distance = (self.max_distance - 0.01).max(0.0);
            }
        }
        self.rolloff = if self.rolloff.is_finite() {
            self.rolloff.clamp(0.1, 8.0)
        } else {
            1.0
        };
        self.curve_points
            .retain(|point| point[0].is_finite() && point[1].is_finite());
        for point in &mut self.curve_points {
            point[0] = point[0].clamp(0.0, 1.0);
            point[1] = point[1].clamp(0.0, 1.0);
        }
        self.curve_points.sort_by(|a, b| a[0].total_cmp(&b[0]));
        self.curve_points
            .dedup_by(|a, b| (a[0] - b[0]).abs() <= 1.0e-6);
        if self.curve == AudioAttenuationCurve::Custom {
            if self.curve_points.first().is_none_or(|point| point[0] > 0.0) {
                self.curve_points.insert(0, [0.0, 1.0]);
            }
            if self.curve_points.last().is_none_or(|point| point[0] < 1.0) {
                self.curve_points.push([1.0, 0.0]);
            }
        }
        self
    }

    #[inline]
    pub fn gain_at_distance(&self, distance: f32) -> f32 {
        let min_distance = if self.min_distance.is_finite() {
            self.min_distance.max(0.0)
        } else {
            1.0
        };
        let mut max_distance = if self.max_distance.is_finite() {
            self.max_distance.max(0.01)
        } else {
            50.0
        };
        if max_distance <= min_distance {
            max_distance = min_distance + 0.01;
        }
        let rolloff = if self.rolloff.is_finite() {
            self.rolloff.clamp(0.1, 8.0)
        } else {
            1.0
        };
        let distance = if distance.is_finite() {
            distance.max(0.0)
        } else {
            max_distance
        };
        if distance <= min_distance {
            return 1.0;
        }
        if distance >= max_distance {
            return 0.0;
        }
        let t = ((distance - min_distance) / (max_distance - min_distance)).clamp(0.0, 1.0);
        match self.curve {
            AudioAttenuationCurve::Linear => 1.0 - t,
            AudioAttenuationCurve::Smoothstep => 1.0 - t * t * (3.0 - 2.0 * t),
            AudioAttenuationCurve::Exponential => (1.0 - t).powf(rolloff),
            AudioAttenuationCurve::Inverse => {
                let scale = 4.0 * rolloff;
                let raw = 1.0 / (1.0 + scale * t);
                let end = 1.0 / (1.0 + scale);
                ((raw - end) / (1.0 - end)).clamp(0.0, 1.0)
            }
            AudioAttenuationCurve::Custom => sample_custom_attenuation(&self.curve_points, t),
        }
    }
}

fn sample_custom_attenuation(points: &[[f32; 2]], t: f32) -> f32 {
    let Some(first) = points.first() else {
        return 1.0 - t;
    };
    if t <= first[0] {
        return first[1];
    }
    for pair in points.windows(2) {
        let a = pair[0];
        let b = pair[1];
        if t <= b[0] {
            let width = (b[0] - a[0]).max(1.0e-6);
            let local = ((t - a[0]) / width).clamp(0.0, 1.0);
            return (a[1] + (b[1] - a[1]) * local).clamp(0.0, 1.0);
        }
    }
    points.last().map(|point| point[1]).unwrap_or(0.0)
}

/// Material-domain acoustic response resolved from a blocker surface. The physics
/// backend never owns this data; engine/audio semantics map stable surface ids to
/// these coefficients after a provider-neutral query hit is returned.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AcousticMaterialProfile {
    /// Broadband energy transmitted through the material when a ray is blocked.
    pub transmission_gain: f32,
    /// Fraction of high-frequency energy absorbed by the material in `[0,1]`.
    pub high_frequency_absorption: f32,
    /// Nominal low-pass cutoff for a fully blocked path.
    pub low_pass_hz: f32,
}

impl Default for AcousticMaterialProfile {
    #[inline]
    fn default() -> Self {
        Self {
            transmission_gain: 0.35,
            high_frequency_absorption: 0.65,
            low_pass_hz: 3_500.0,
        }
    }
}

impl AcousticMaterialProfile {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            transmission_gain: finite_clamped(self.transmission_gain, 0.35, 0.0, 1.0),
            high_frequency_absorption: finite_clamped(
                self.high_frequency_absorption,
                0.65,
                0.0,
                1.0,
            ),
            low_pass_hz: finite_clamped(self.low_pass_hz, 3_500.0, 80.0, 20_000.0),
        }
    }

    #[inline]
    pub fn high_frequency_gain(self) -> f32 {
        1.0 - self.sanitized().high_frequency_absorption
    }
}

/// Durable authored acoustic material override attached to a collidable ECS entity.
/// The physics backend remains unaware of this component; engine-runtime resolves it
/// from the stable blocker entity returned by `engine.physics`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AcousticSurface {
    pub material_id: String,
    pub profile: AcousticMaterialProfile,
}

impl Default for AcousticSurface {
    fn default() -> Self {
        Self {
            material_id: "material.default".to_owned(),
            profile: AcousticMaterialProfile::default(),
        }
    }
}

impl AcousticSurface {
    pub fn new(material_id: impl Into<String>, profile: AcousticMaterialProfile) -> Self {
        Self {
            material_id: material_id.into(),
            profile,
        }
        .sanitized()
    }

    pub fn sanitized(mut self) -> Self {
        self.material_id = self.material_id.trim().to_owned();
        if self.material_id.is_empty() {
            self.material_id = "material.default".to_owned();
        }
        self.profile = self.profile.sanitized();
        self
    }
}

/// Authored spatial occlusion policy for ECS-owned audio emitters. The physics
/// transport stays provider-neutral: engine-runtime turns this policy into a
/// bounded batch of `PhysicsQueryDto::Ray` probes through `engine.physics`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioOcclusionSettings {
    pub enabled: bool,
    pub max_distance: f32,
    pub ray_count: u8,
    pub probe_radius: f32,
    pub obstruction_gain: f32,
    pub occlusion_gain: f32,
    pub attack_seconds: f32,
    pub release_seconds: f32,
}

impl Default for AudioOcclusionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_distance: 80.0,
            ray_count: 3,
            probe_radius: 0.35,
            obstruction_gain: 0.65,
            occlusion_gain: 0.22,
            attack_seconds: 0.06,
            release_seconds: 0.22,
        }
    }
}

impl AudioOcclusionSettings {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            enabled: self.enabled,
            max_distance: finite_clamped(self.max_distance, 80.0, 0.5, 10_000.0),
            ray_count: self.ray_count.clamp(1, 5),
            probe_radius: finite_clamped(self.probe_radius, 0.35, 0.0, 4.0),
            obstruction_gain: finite_clamped(self.obstruction_gain, 0.65, 0.0, 1.0),
            occlusion_gain: finite_clamped(self.occlusion_gain, 0.22, 0.0, 1.0),
            attack_seconds: finite_clamped(self.attack_seconds, 0.06, 0.005, 5.0),
            release_seconds: finite_clamped(self.release_seconds, 0.22, 0.005, 5.0),
        }
    }

    #[inline]
    pub fn transmission_gain(self, obstruction: f32, occlusion: f32) -> f32 {
        let settings = self.sanitized();
        let obstruction = finite_clamped(obstruction, 0.0, 0.0, 1.0);
        let occlusion = finite_clamped(occlusion, 0.0, 0.0, 1.0);
        let obstructed = 1.0 - obstruction * (1.0 - settings.obstruction_gain);
        (obstructed + (settings.occlusion_gain - obstructed) * occlusion).clamp(0.0, 1.0)
    }

    #[inline]
    pub fn acoustic_state(self, obstruction: f32, occlusion: f32) -> AudioAcousticState {
        self.acoustic_state_with_material(
            obstruction,
            occlusion,
            AcousticMaterialProfile {
                transmission_gain: 1.0,
                high_frequency_absorption: 0.0,
                low_pass_hz: 20_000.0,
            },
        )
    }

    /// Combines geometric blockage with the material-domain spectral response.
    /// Clear rays contribute unity; blocked rays contribute the material profile.
    #[inline]
    pub fn acoustic_state_with_material(
        self,
        obstruction: f32,
        occlusion: f32,
        material: AcousticMaterialProfile,
    ) -> AudioAcousticState {
        let obstruction = finite_clamped(obstruction, 0.0, 0.0, 1.0);
        let occlusion = finite_clamped(occlusion, 0.0, 0.0, 1.0);
        let material = material.sanitized();
        let geometry_gain = self.transmission_gain(obstruction, occlusion);
        let material_gain = lerp(1.0, material.transmission_gain, obstruction);
        let spectral_weight = obstruction.max(occlusion);
        AudioAcousticState {
            obstruction,
            occlusion,
            transmission_gain: (geometry_gain * material_gain).clamp(0.0, 1.0),
            high_frequency_gain: lerp(1.0, material.high_frequency_gain(), spectral_weight),
            low_pass_hz: lerp(20_000.0, material.low_pass_hz, spectral_weight),
        }
        .sanitized()
    }
}

/// Smoothed acoustic result applied to a logical voice after distance attenuation.
/// `transmission_gain` is part of audibility ranking, while `high_frequency_gain`
/// and `low_pass_hz` drive the provider-neutral spectral transmission controls.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioAcousticState {
    pub obstruction: f32,
    pub occlusion: f32,
    pub transmission_gain: f32,
    pub high_frequency_gain: f32,
    pub low_pass_hz: f32,
}

impl Default for AudioAcousticState {
    #[inline]
    fn default() -> Self {
        Self::clear()
    }
}

impl AudioAcousticState {
    #[inline]
    pub const fn clear() -> Self {
        Self {
            obstruction: 0.0,
            occlusion: 0.0,
            transmission_gain: 1.0,
            high_frequency_gain: 1.0,
            low_pass_hz: 20_000.0,
        }
    }

    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            obstruction: finite_clamped(self.obstruction, 0.0, 0.0, 1.0),
            occlusion: finite_clamped(self.occlusion, 0.0, 0.0, 1.0),
            transmission_gain: finite_clamped(self.transmission_gain, 1.0, 0.0, 1.0),
            high_frequency_gain: finite_clamped(self.high_frequency_gain, 1.0, 0.0, 1.0),
            low_pass_hz: finite_clamped(self.low_pass_hz, 20_000.0, 80.0, 20_000.0),
        }
    }

    pub fn smoothed_towards(
        self,
        target: Self,
        dt: f32,
        attack_seconds: f32,
        release_seconds: f32,
    ) -> Self {
        let current = self.sanitized();
        let target = target.sanitized();
        let dt = finite_clamped(dt, 1.0 / 60.0, 0.0, 0.25);
        let closing = target.transmission_gain < current.transmission_gain
            || target.high_frequency_gain < current.high_frequency_gain
            || target.low_pass_hz < current.low_pass_hz;
        let time = if closing {
            finite_clamped(attack_seconds, 0.06, 0.005, 5.0)
        } else {
            finite_clamped(release_seconds, 0.22, 0.005, 5.0)
        };
        let alpha = if dt <= 0.0 {
            0.0
        } else {
            1.0 - (-dt / time).exp()
        };
        Self {
            obstruction: lerp(current.obstruction, target.obstruction, alpha),
            occlusion: lerp(current.occlusion, target.occlusion, alpha),
            transmission_gain: lerp(current.transmission_gain, target.transmission_gain, alpha),
            high_frequency_gain: lerp(
                current.high_frequency_gain,
                target.high_frequency_gain,
                alpha,
            ),
            low_pass_hz: lerp(current.low_pass_hz, target.low_pass_hz, alpha),
        }
        .sanitized()
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
    #[serde(default)]
    pub attenuation: Option<AudioAttenuationSettings>,
    #[serde(default)]
    pub acoustic: AudioAcousticState,
    #[serde(default)]
    pub environment: AudioEnvironmentState,
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
            attenuation: None,
            acoustic: AudioAcousticState::clear(),
            environment: AudioEnvironmentState::clear(),
        }
    }

    #[inline]
    pub fn sanitized(mut self) -> Self {
        self.gain = sanitize_gain(self.gain);
        self.speed = sanitize_speed(self.speed);
        self.spatial = self.spatial.map(AudioSpatialParams::sanitized);
        self.attenuation = self.attenuation.map(AudioAttenuationSettings::sanitized);
        self.acoustic = self.acoustic.sanitized();
        self.environment = self.environment.sanitized();
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
    /// True when the logical voice was accepted but currently owns no physical
    /// mixer slot. It may be promoted later by the provider's voice arbiter.
    #[serde(default)]
    pub virtualized: bool,
    /// Provider-authored semantic trace lines. This stays optional at the JSON
    /// boundary so older providers/consumers remain wire-compatible.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
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
    /// Provider-authored semantic trace lines, e.g. YSCD dictionary resolution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
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
    /// Absolute source timeline seek for seekable voices/streams.
    #[serde(default)]
    pub seek_seconds: Option<f64>,
    #[serde(default)]
    pub paused: Option<bool>,
    #[serde(default)]
    pub position: Option<[f32; 3]>,
    #[serde(default)]
    pub acoustic: Option<AudioAcousticState>,
    #[serde(default)]
    pub environment: Option<AudioEnvironmentState>,
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

/// Stable authored ECS payload for an entity-owned audio emitter. This DTO is
/// also the native ECS component type used by the first-party runtime, allowing
/// `engine.ecs` to author it without exposing `World` across service boundaries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioEmitter {
    pub cue: String,
    pub enabled: bool,
    pub autoplay: bool,
    pub gain: f32,
    /// Supplies the owning entity's world position to the cue. `non_spatial`
    /// cues ignore it; `spatial` cues require a position.
    pub spatial: bool,
    #[serde(default)]
    pub occlusion: AudioOcclusionSettings,
}

impl Default for AudioEmitter {
    fn default() -> Self {
        Self {
            cue: String::new(),
            enabled: true,
            autoplay: true,
            gain: 1.0,
            spatial: true,
            occlusion: AudioOcclusionSettings::default(),
        }
    }
}

impl AudioEmitter {
    #[inline]
    pub fn new(cue: impl Into<String>) -> Self {
        Self {
            cue: cue.into(),
            ..Self::default()
        }
    }

    #[inline]
    pub fn sanitized_gain(&self) -> f32 {
        sanitize_gain(self.gain)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundCueRef {
    pub logical_path: String,
}

impl SoundCueRef {
    #[inline]
    pub fn new(logical_path: impl Into<String>) -> Self {
        Self {
            logical_path: logical_path.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundCueSpatialPolicy {
    #[default]
    Inherit,
    NonSpatial,
    Spatial,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundCueClip {
    pub clip: AudioClipRef,
    pub weight: f32,
    pub gain: f32,
    pub pitch: f32,
}

impl Default for SoundCueClip {
    fn default() -> Self {
        Self {
            clip: AudioClipRef::new(String::new()),
            weight: 1.0,
            gain: 1.0,
            pitch: 1.0,
        }
    }
}

impl SoundCueClip {
    #[inline]
    pub fn sanitized(mut self) -> Self {
        self.weight = if self.weight.is_finite() && self.weight > 0.0 {
            self.weight
        } else {
            0.0
        };
        self.gain = sanitize_gain(self.gain);
        self.pitch = sanitize_speed(self.pitch);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundCue {
    pub version: u32,
    pub clips: Vec<SoundCueClip>,
    pub gain_range: [f32; 2],
    pub pitch_range: [f32; 2],
    pub bus: AudioBus,
    pub looping: bool,
    pub concurrency_group: String,
    pub priority: i32,
    pub spatial_policy: SoundCueSpatialPolicy,
    pub attenuation: Option<AudioAttenuationSettings>,
}

impl Default for SoundCue {
    fn default() -> Self {
        Self {
            version: 1,
            clips: Vec::new(),
            gain_range: [1.0, 1.0],
            pitch_range: [1.0, 1.0],
            bus: AudioBus::Sfx,
            looping: false,
            concurrency_group: String::new(),
            priority: 0,
            spatial_policy: SoundCueSpatialPolicy::Inherit,
            attenuation: None,
        }
    }
}

impl SoundCue {
    pub fn sanitized(mut self) -> Result<Self, String> {
        if self.version != 1 {
            return Err(format!("unsupported SoundCue version {}", self.version));
        }
        self.clips = self
            .clips
            .into_iter()
            .map(SoundCueClip::sanitized)
            .filter(|clip| !clip.clip.uri.trim().is_empty() && clip.weight > 0.0)
            .collect();
        if self.clips.is_empty() {
            return Err("SoundCue requires at least one positively weighted clip".to_owned());
        }
        self.gain_range = sanitize_range(self.gain_range, 0.0, 4.0, [1.0, 1.0]);
        self.pitch_range = sanitize_range(self.pitch_range, 0.05, 4.0, [1.0, 1.0]);
        self.concurrency_group = self.concurrency_group.trim().to_owned();
        self.attenuation = self.attenuation.map(AudioAttenuationSettings::sanitized);
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioCuePlayRequest {
    pub version: u32,
    pub cue: SoundCueRef,
    pub position: Option<[f32; 3]>,
    pub gain: f32,
    pub seed: Option<u64>,
    #[serde(default)]
    pub acoustic: AudioAcousticState,
    #[serde(default)]
    pub environment: AudioEnvironmentState,
}

impl Default for AudioCuePlayRequest {
    fn default() -> Self {
        Self {
            version: 1,
            cue: SoundCueRef::new(String::new()),
            position: None,
            gain: 1.0,
            seed: None,
            acoustic: AudioAcousticState::clear(),
            environment: AudioEnvironmentState::clear(),
        }
    }
}

impl AudioCuePlayRequest {
    #[inline]
    pub fn new(logical_path: impl Into<String>) -> Self {
        Self {
            cue: SoundCueRef::new(logical_path),
            ..Self::default()
        }
    }

    #[inline]
    pub fn sanitized(mut self) -> Self {
        self.gain = sanitize_gain(self.gain);
        self.position = self.position.map(sanitize_vec3);
        self.acoustic = self.acoustic.sanitized();
        self.environment = self.environment.sanitized();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioCuePreloadRequest {
    pub cue: SoundCueRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDiagnostics {
    pub provider: String,
    pub output_ready: bool,
    /// Logical voices known to the provider (physical + virtual).
    pub active_voices: usize,
    pub spatial_voices: usize,
    #[serde(default)]
    pub physical_voices: usize,
    #[serde(default)]
    pub virtual_voices: usize,
    #[serde(default)]
    pub max_physical_voices: usize,
    #[serde(default)]
    pub attenuated_voices: usize,
    #[serde(default)]
    pub obstructed_voices: usize,
    #[serde(default)]
    pub occluded_voices: usize,
    #[serde(default)]
    pub spectrally_filtered_voices: usize,
    #[serde(default)]
    pub reverberant_voices: usize,
    #[serde(default)]
    pub active_streams: usize,
    #[serde(default)]
    pub stream_buffered_frames: usize,
    #[serde(default)]
    pub stream_buffer_capacity_frames: usize,
    #[serde(default)]
    pub stream_underruns: u64,
    #[serde(default)]
    pub stream_range_requests: u64,
    #[serde(default)]
    pub stream_compressed_bytes_fetched: u64,
    #[serde(default)]
    pub stream_seek_operations: u64,
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
fn sanitize_range(
    value: [f32; 2],
    min_allowed: f32,
    max_allowed: f32,
    fallback: [f32; 2],
) -> [f32; 2] {
    if !value[0].is_finite() || !value[1].is_finite() {
        return fallback;
    }
    let a = value[0].clamp(min_allowed, max_allowed);
    let b = value[1].clamp(min_allowed, max_allowed);
    [a.min(b), a.max(b)]
}

#[inline]
fn finite_clamped(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
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
    #[test]
    fn sound_cue_sanitizes_ranges_and_rejects_empty_clip_sets() {
        assert!(SoundCue::default().sanitized().is_err());
        let cue = SoundCue {
            clips: vec![SoundCueClip {
                clip: AudioClipRef::new("shared/audio/test.ogg"),
                weight: 1.0,
                gain: 1.0,
                pitch: 1.0,
            }],
            gain_range: [1.25, 0.75],
            pitch_range: [1.1, 0.9],
            ..SoundCue::default()
        }
        .sanitized()
        .expect("valid cue");
        assert_eq!(cue.gain_range, [0.75, 1.25]);
        assert_eq!(cue.pitch_range, [0.9, 1.1]);
    }

    #[test]
    fn attenuation_curves_are_bounded_and_custom_points_interpolate() {
        let linear = AudioAttenuationSettings {
            min_distance: 2.0,
            max_distance: 12.0,
            curve: AudioAttenuationCurve::Linear,
            ..Default::default()
        };
        assert_eq!(linear.gain_at_distance(2.0), 1.0);
        assert_eq!(linear.gain_at_distance(12.0), 0.0);
        assert!((linear.gain_at_distance(7.0) - 0.5).abs() < 1.0e-5);

        let custom = AudioAttenuationSettings {
            min_distance: 0.0,
            max_distance: 100.0,
            curve: AudioAttenuationCurve::Custom,
            curve_points: vec![[0.75, 0.1], [0.25, 0.8]],
            ..Default::default()
        }
        .sanitized();
        assert_eq!(custom.curve_points.first().copied(), Some([0.0, 1.0]));
        assert_eq!(custom.curve_points.last().copied(), Some([1.0, 0.0]));
        let gain = custom.gain_at_distance(50.0);
        assert!((0.1..0.8).contains(&gain));
    }

    #[test]
    fn sound_cue_carries_authored_attenuation() {
        let cue = SoundCue {
            clips: vec![SoundCueClip {
                clip: AudioClipRef::new("shared/audio/test.wav"),
                ..SoundCueClip::default()
            }],
            spatial_policy: SoundCueSpatialPolicy::Spatial,
            attenuation: Some(AudioAttenuationSettings {
                min_distance: 3.0,
                max_distance: 60.0,
                curve: AudioAttenuationCurve::Smoothstep,
                ..Default::default()
            }),
            ..SoundCue::default()
        }
        .sanitized()
        .expect("valid attenuated cue");
        let attenuation = cue.attenuation.expect("attenuation retained");
        assert_eq!(attenuation.min_distance, 3.0);
        assert_eq!(attenuation.max_distance, 60.0);
    }

    #[test]
    fn audio_emitter_is_a_stable_semantic_component_payload() {
        let emitter = AudioEmitter::new("shared/audio/test.yscd@test");
        let json = serde_json::to_value(&emitter).expect("serialize emitter");
        let decoded: AudioEmitter = serde_json::from_value(json).expect("decode emitter");
        assert_eq!(decoded, emitter);
        assert_eq!(AUDIO_EMITTER_COMPONENT_TYPE, "audio.emitter");
    }

    #[test]
    fn occlusion_policy_produces_distinct_obstruction_and_occlusion_gain() {
        let settings = AudioOcclusionSettings::default().sanitized();
        assert_eq!(settings.ray_count, 3);
        let clear = settings.acoustic_state(0.0, 0.0);
        let obstructed = settings.acoustic_state(0.5, 0.0);
        let occluded = settings.acoustic_state(1.0, 1.0);
        assert_eq!(clear.transmission_gain, 1.0);
        assert!(obstructed.transmission_gain < clear.transmission_gain);
        assert!(occluded.transmission_gain < obstructed.transmission_gain);
        assert!((occluded.transmission_gain - settings.occlusion_gain).abs() < 1.0e-6);
    }

    #[test]
    fn acoustic_state_smoothing_uses_attack_and_release_time_constants() {
        let blocked = AudioAcousticState {
            obstruction: 1.0,
            occlusion: 1.0,
            transmission_gain: 0.2,
            high_frequency_gain: 0.15,
            low_pass_hz: 1_200.0,
        };
        let attacked = AudioAcousticState::clear().smoothed_towards(blocked, 0.016, 0.05, 0.4);
        assert!(attacked.transmission_gain < 1.0);
        let released = attacked.smoothed_towards(AudioAcousticState::clear(), 0.016, 0.05, 0.4);
        assert!(released.transmission_gain > attacked.transmission_gain);
        assert!(released.transmission_gain < 1.0);
    }

    #[test]
    fn acoustic_surface_is_a_stable_semantic_component_payload() {
        let surface = AcousticSurface::new(
            "material.concrete.wall",
            AcousticMaterialProfile {
                transmission_gain: 0.2,
                high_frequency_absorption: 0.9,
                low_pass_hz: 1_200.0,
            },
        );
        let json = serde_json::to_value(&surface).expect("serialize acoustic surface");
        let decoded: AcousticSurface =
            serde_json::from_value(json).expect("decode acoustic surface");
        assert_eq!(decoded, surface);
        assert_eq!(
            AUDIO_ACOUSTIC_SURFACE_COMPONENT_TYPE,
            "audio.acoustic_surface"
        );
    }

    #[test]
    fn acoustic_material_profiles_change_energy_and_spectrum() {
        let settings = AudioOcclusionSettings::default();
        let concrete = AcousticMaterialProfile {
            transmission_gain: 0.16,
            high_frequency_absorption: 0.92,
            low_pass_hz: 1_100.0,
        };
        let glass = AcousticMaterialProfile {
            transmission_gain: 0.58,
            high_frequency_absorption: 0.42,
            low_pass_hz: 6_500.0,
        };
        let concrete_state = settings.acoustic_state_with_material(1.0, 1.0, concrete);
        let glass_state = settings.acoustic_state_with_material(1.0, 1.0, glass);
        assert!(concrete_state.transmission_gain < glass_state.transmission_gain);
        assert!(concrete_state.high_frequency_gain < glass_state.high_frequency_gain);
        assert!(concrete_state.low_pass_hz < glass_state.low_pass_hz);
    }

    #[test]
    fn invalid_acoustic_state_fails_open_instead_of_muting_audio() {
        let state = AudioAcousticState {
            obstruction: f32::NAN,
            occlusion: f32::INFINITY,
            transmission_gain: f32::NAN,
            high_frequency_gain: f32::NAN,
            low_pass_hz: f32::NAN,
        }
        .sanitized();
        assert_eq!(state, AudioAcousticState::clear());
    }
}
