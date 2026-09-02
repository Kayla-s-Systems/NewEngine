use super::*;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioConcurrencyScope {
    #[default]
    Global,
    Object,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioVoiceStealRule {
    RejectNew,
    #[default]
    LowerPriorityThenOldest,
    Oldest,
    Quietest,
    Farthest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioVoicePolicy {
    pub group: String,
    pub limit: usize,
    pub scope: AudioConcurrencyScope,
    pub steal_rule: AudioVoiceStealRule,
    /// Opaque project-authored physical-budget class. Empty means the shared pool.
    pub budget: String,
    pub priority: i32,
}

impl Default for AudioVoicePolicy {
    fn default() -> Self {
        Self {
            group: String::new(),
            limit: 1,
            scope: AudioConcurrencyScope::Global,
            steal_rule: AudioVoiceStealRule::LowerPriorityThenOldest,
            budget: String::new(),
            priority: 0,
        }
    }
}

impl AudioVoicePolicy {
    pub fn sanitized(mut self) -> Result<Self, String> {
        self.group = self.group.trim().to_owned();
        self.budget = self.budget.trim().to_ascii_lowercase();
        if self.group.len() > 256 {
            return Err("audio concurrency group exceeds 256 bytes".to_owned());
        }
        if self.budget.len() > 256 {
            return Err("audio voice budget id exceeds 256 bytes".to_owned());
        }
        self.limit = self.limit.clamp(1, 4096);
        self.priority = self.priority.clamp(-100_000, 100_000);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioVoiceBudgetReservation {
    pub id: String,
    pub reserved_physical_voices: usize,
}

impl Default for AudioVoiceBudgetReservation {
    fn default() -> Self {
        Self {
            id: String::new(),
            reserved_physical_voices: 1,
        }
    }
}

impl AudioVoiceBudgetReservation {
    pub fn sanitized(mut self) -> Result<Self, String> {
        self.id = self.id.trim().to_ascii_lowercase();
        if self.id.is_empty() || self.id.len() > 256 {
            return Err("audio voice budget id must contain 1..=256 bytes".to_owned());
        }
        self.reserved_physical_voices = self.reserved_physical_voices.clamp(1, 4096);
        Ok(self)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioVoiceBudgetConfig {
    pub reservations: Vec<AudioVoiceBudgetReservation>,
}

impl AudioVoiceBudgetConfig {
    pub fn sanitized(mut self) -> Result<Self, String> {
        let mut ids = std::collections::BTreeSet::new();
        let mut reservations = Vec::with_capacity(self.reservations.len());
        for reservation in self.reservations {
            let reservation = reservation.sanitized()?;
            let key = reservation.id.to_ascii_lowercase();
            if !ids.insert(key) {
                return Err(format!("duplicate audio voice budget '{}'", reservation.id));
            }
            reservations.push(reservation);
        }
        reservations.sort_by(|a, b| a.id.cmp(&b.id));
        self.reservations = reservations;
        Ok(self)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioVoiceBudgetAck {
    pub accepted: bool,
    pub max_physical_voices: usize,
    #[serde(default)]
    pub reservations: std::collections::BTreeMap<String, usize>,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioPlayRequest {
    #[serde(default = "protocol_version_one")]
    pub version: u32,
    pub clip: AudioClipRef,
    #[serde(default)]
    pub route: AudioRouteId,
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
    /// Optional absolute provider render-sample at which the physical node becomes audible.
    /// This is a low-level executor coordinate, never a gameplay/music semantic.
    #[serde(default)]
    pub render_start_sample: Option<u64>,
}

impl AudioPlayRequest {
    #[inline]
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            version: 1,
            clip: AudioClipRef::new(uri),
            route: AudioRouteId::default(),
            gain: 1.0,
            speed: 1.0,
            looping: false,
            spatial: None,
            attenuation: None,
            acoustic: AudioAcousticState::clear(),
            environment: AudioEnvironmentState::clear(),
            render_start_sample: None,
        }
    }

    #[inline]
    pub fn sanitized(mut self) -> Self {
        self.route.0 = self.route.0.trim().to_owned();
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
    /// All physical/logical provider voices owned by this accepted play operation.
    /// `voice_id` remains the compatibility primary handle. Layered cues populate
    /// this collection so higher-level AudioInstance lifetime can stop/update the
    /// complete cue rather than leaking secondary layers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub voice_ids: Vec<u64>,
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
    /// Provider-authored semantic trace lines, e.g. YSNCD dictionary resolution.
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRouteGainRequest {
    pub route: AudioRouteId,
    pub gain: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRouteGainAck {
    pub accepted: bool,
    pub route: AudioRouteId,
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
    pub route: AudioRouteId,
    pub looping: bool,
    pub concurrency_group: String,
    /// Maximum simultaneous logical cue instances in the selected scope.
    pub concurrency_limit: usize,
    pub concurrency_scope: AudioConcurrencyScope,
    pub steal_rule: AudioVoiceStealRule,
    /// Opaque project-authored physical voice budget class.
    pub voice_budget: String,
    pub priority: i32,
    /// Number of recently selected clips excluded from the next weighted draw.
    /// Zero preserves unconstrained weighted random selection.
    pub repeat_avoidance: usize,
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
            route: AudioRouteId::default(),
            looping: false,
            concurrency_group: String::new(),
            concurrency_limit: 1,
            concurrency_scope: AudioConcurrencyScope::Global,
            steal_rule: AudioVoiceStealRule::LowerPriorityThenOldest,
            voice_budget: String::new(),
            priority: 0,
            repeat_avoidance: 0,
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
        self.route.0 = self.route.0.trim().to_owned();
        if !self.route.0.is_empty() {
            self.route.validate()?;
        }
        let policy = AudioVoicePolicy {
            group: self.concurrency_group,
            limit: self.concurrency_limit,
            scope: self.concurrency_scope,
            steal_rule: self.steal_rule,
            budget: self.voice_budget,
            priority: self.priority,
        }
        .sanitized()?;
        self.concurrency_group = policy.group;
        self.concurrency_limit = policy.limit;
        self.concurrency_scope = policy.scope;
        self.steal_rule = policy.steal_rule;
        self.voice_budget = policy.budget;
        self.priority = policy.priority;
        self.repeat_avoidance = self.repeat_avoidance.min(64);
        self.attenuation = self.attenuation.map(AudioAttenuationSettings::sanitized);
        Ok(self)
    }

    #[inline]
    pub fn voice_policy(&self) -> AudioVoicePolicy {
        AudioVoicePolicy {
            group: self.concurrency_group.clone(),
            limit: self.concurrency_limit,
            scope: self.concurrency_scope,
            steal_rule: self.steal_rule,
            budget: self.voice_budget.clone(),
            priority: self.priority,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioCuePlayRequest {
    pub version: u32,
    pub cue: SoundCueRef,
    /// Optional caller-owned route override. Empty preserves the cue-authored route.
    #[serde(default)]
    pub route: AudioRouteId,
    pub position: Option<[f32; 3]>,
    pub gain: f32,
    /// Per-playback pitch/speed multiplier applied after authored cue/clip pitch randomization.
    /// Gameplay may use this for physically derived contact energy without selecting clips.
    pub pitch: f32,
    pub seed: Option<u64>,
    /// Opaque owner/object identity used only when an authored concurrency policy has object scope.
    #[serde(default)]
    pub scope_id: Option<u64>,
    /// Logical source offset when a transport-scheduled cue is dispatched after its intended
    /// sample boundary. Zero means start from source origin.
    #[serde(default)]
    pub start_sample_offset: u64,
    /// Sample rate for `start_sample_offset`. Zero is valid only when the offset is zero.
    #[serde(default)]
    pub transport_sample_rate: u32,
    #[serde(default)]
    pub acoustic: AudioAcousticState,
    #[serde(default)]
    pub environment: AudioEnvironmentState,
    /// Trigger-time project parameters consumed by YSNCD SoundGraph. Names are opaque.
    #[serde(default)]
    pub parameters: AudioParameterSet,
    /// Optional absolute provider render sample for exact physical onset.
    #[serde(default)]
    pub render_start_sample: Option<u64>,
}

impl Default for AudioCuePlayRequest {
    fn default() -> Self {
        Self {
            version: 1,
            cue: SoundCueRef::new(String::new()),
            route: AudioRouteId::default(),
            position: None,
            gain: 1.0,
            pitch: 1.0,
            seed: None,
            scope_id: None,
            start_sample_offset: 0,
            transport_sample_rate: 0,
            acoustic: AudioAcousticState::clear(),
            environment: AudioEnvironmentState::clear(),
            parameters: AudioParameterSet::default(),
            render_start_sample: None,
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
        self.route.0 = self.route.0.trim().to_owned();
        self.gain = sanitize_gain(self.gain);
        self.pitch = sanitize_speed(self.pitch);
        self.position = self.position.map(sanitize_vec3);
        self.scope_id = self.scope_id.filter(|id| *id != 0);
        if self.start_sample_offset == 0 {
            self.transport_sample_rate = 0;
        }
        self.acoustic = self.acoustic.sanitized();
        self.environment = self.environment.sanitized();
        self.parameters = self.parameters.sanitized();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioCuePreloadRequest {
    pub cue: SoundCueRef,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioRenderClock {
    pub ready: bool,
    pub sample_rate: u32,
    pub sample: u64,
    pub block_frames: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AudioVoiceRenderAction {
    GainRamp {
        target_gain: f32,
        duration_samples: u64,
    },
    Stop,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AudioVoiceRenderScheduleRequest {
    pub voice_id: u64,
    pub at_sample: u64,
    /// Opaque caller-owned id used to cancel a previously armed render action.
    #[serde(default)]
    pub schedule_id: u64,
    pub action: AudioVoiceRenderAction,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioVoiceRenderScheduleAck {
    pub accepted: bool,
    pub voice_id: u64,
    pub at_sample: u64,
    #[serde(default)]
    pub schedule_id: u64,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub message: String,
}
