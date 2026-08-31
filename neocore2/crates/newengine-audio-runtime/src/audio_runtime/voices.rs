const SPEED_OF_SOUND_MPS: f32 = 343.0;
const MAX_DOPPLER_RADIAL_SPEED_MPS: f32 = SPEED_OF_SOUND_MPS * 0.25;
const MAX_TRACKED_AUDIO_VELOCITY_MPS: f32 = 120.0;
const MIN_VELOCITY_SAMPLE_SECONDS: f32 = 1.0 / 500.0;
const MAX_VELOCITY_SAMPLE_SECONDS: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq)]
struct AudioPropagationState {
    distance: f32,
    air_gain: f32,
    air_high_frequency_gain: f32,
    air_low_pass_hz: f32,
    doppler_ratio: f32,
}

impl Default for AudioPropagationState {
    fn default() -> Self {
        Self {
            distance: 0.0,
            air_gain: 1.0,
            air_high_frequency_gain: 1.0,
            air_low_pass_hz: 20_000.0,
            doppler_ratio: 1.0,
        }
    }
}

#[inline]
fn vec3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
fn vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn vec3_length(value: [f32; 3]) -> f32 {
    vec3_dot(value, value).max(0.0).sqrt()
}

#[inline]
fn vec3_normalize_or_zero(value: [f32; 3]) -> [f32; 3] {
    let length = vec3_length(value);
    if !length.is_finite() || length <= 1.0e-5 {
        [0.0; 3]
    } else {
        [value[0] / length, value[1] / length, value[2] / length]
    }
}

#[inline]
fn clamp_velocity(value: [f32; 3]) -> [f32; 3] {
    let speed = vec3_length(value);
    if !speed.is_finite() || speed <= 0.0 {
        return [0.0; 3];
    }
    if speed <= MAX_TRACKED_AUDIO_VELOCITY_MPS {
        value
    } else {
        let scale = MAX_TRACKED_AUDIO_VELOCITY_MPS / speed;
        [value[0] * scale, value[1] * scale, value[2] * scale]
    }
}

#[inline]
fn estimate_velocity(previous: [f32; 3], current: [f32; 3], dt_seconds: f32) -> [f32; 3] {
    if !dt_seconds.is_finite()
        || !(MIN_VELOCITY_SAMPLE_SECONDS..=MAX_VELOCITY_SAMPLE_SECONDS).contains(&dt_seconds)
    {
        return [0.0; 3];
    }
    let delta = vec3_sub(current, previous);
    let raw = [
        delta[0] / dt_seconds,
        delta[1] / dt_seconds,
        delta[2] / dt_seconds,
    ];
    // Camera/entity teleports are discontinuities, not acoustic velocities.
    if vec3_length(raw) > MAX_TRACKED_AUDIO_VELOCITY_MPS {
        [0.0; 3]
    } else {
        clamp_velocity(raw)
    }
}

#[inline]
fn smooth_velocity(previous: [f32; 3], target: [f32; 3]) -> [f32; 3] {
    const ALPHA: f32 = 0.42;
    [
        previous[0] + (target[0] - previous[0]) * ALPHA,
        previous[1] + (target[1] - previous[1]) * ALPHA,
        previous[2] + (target[2] - previous[2]) * ALPHA,
    ]
}

#[inline]
fn doppler_ratio(
    listener_position: [f32; 3],
    listener_velocity: [f32; 3],
    emitter_position: [f32; 3],
    emitter_velocity: [f32; 3],
) -> f32 {
    let listener_to_source = vec3_sub(emitter_position, listener_position);
    let direction = vec3_normalize_or_zero(listener_to_source);
    if vec3_length(direction) <= 0.0 {
        return 1.0;
    }
    let listener_toward_source = vec3_dot(listener_velocity, direction)
        .clamp(-MAX_DOPPLER_RADIAL_SPEED_MPS, MAX_DOPPLER_RADIAL_SPEED_MPS);
    let source_along_listener_to_source = vec3_dot(emitter_velocity, direction)
        .clamp(-MAX_DOPPLER_RADIAL_SPEED_MPS, MAX_DOPPLER_RADIAL_SPEED_MPS);
    let numerator = SPEED_OF_SOUND_MPS + listener_toward_source;
    let denominator = (SPEED_OF_SOUND_MPS + source_along_listener_to_source).max(1.0);
    (numerator / denominator).clamp(0.75, 1.35)
}

#[inline]
fn propagation_state(
    listener: AudioListenerState,
    listener_velocity: [f32; 3],
    spatial: Option<AudioSpatialParams>,
    emitter_velocity: [f32; 3],
) -> AudioPropagationState {
    let Some(spatial) = spatial else {
        return AudioPropagationState::default();
    };
    let distance = distance3(spatial.position, listener.position).max(0.0);
    // Standard-atmosphere approximation. Authored distance attenuation remains the dominant
    // energy law; this layer models the additional frequency-dependent air loss.
    let air_gain = (-distance * 0.0008).exp().clamp(0.55, 1.0);
    let air_high_frequency_gain = (-distance * 0.0060).exp().clamp(0.10, 1.0);
    let air_low_pass_hz = (20_000.0 / (1.0 + distance * 0.012)).clamp(2_500.0, 20_000.0);
    AudioPropagationState {
        distance,
        air_gain,
        air_high_frequency_gain,
        air_low_pass_hz,
        doppler_ratio: doppler_ratio(
            listener.position,
            listener_velocity,
            spatial.position,
            emitter_velocity,
        ),
    }
}

enum VoiceControl {
    Flat {
        player: Player,
        spectral: Option<SpectralFilterControl>,
        environment: Option<EnvironmentFilterControl>,
        late_binding: Option<RoomBusVoiceBinding>,
    },
    Spatial {
        player: Player,
        spatial: SpatialMixControl,
        spectral: Option<SpectralFilterControl>,
        environment: Option<EnvironmentFilterControl>,
        late_binding: Option<RoomBusVoiceBinding>,
    },
}

impl VoiceControl {
    #[inline]
    fn set_volume(&self, value: f32) {
        match self {
            Self::Flat {
                player,
                late_binding,
                ..
            }
            | Self::Spatial {
                player,
                late_binding,
                ..
            } => {
                player.set_volume(value);
                if let Some(binding) = late_binding {
                    binding.set_voice_gain(value);
                }
            }
        }
    }

    #[inline]
    fn set_speed(&self, value: f32) {
        match self {
            Self::Flat { player, .. } => player.set_speed(value),
            Self::Spatial { player, .. } => player.set_speed(value),
        }
    }

    #[inline]
    fn set_paused(&self, paused: bool) {
        match (self, paused) {
            (Self::Flat { player, .. }, true) => player.pause(),
            (Self::Flat { player, .. }, false) => player.play(),
            (Self::Spatial { player, .. }, true) => player.pause(),
            (Self::Spatial { player, .. }, false) => player.play(),
        }
    }

    #[inline]
    fn set_emitter_position(&self, position: [f32; 3]) -> bool {
        match self {
            Self::Spatial { spatial, .. } => {
                spatial.set_emitter_position(position);
                true
            }
            Self::Flat { .. } => false,
        }
    }

    #[inline]
    fn update_listener(&self, listener: AudioListenerState) {
        if let Self::Spatial { spatial, .. } = self {
            let (left, right) = listener.ear_positions();
            spatial.set_ears(left, right);
        }
    }

    #[inline]
    fn set_acoustic(&self, acoustic: AudioAcousticState) {
        let spectral = match self {
            Self::Flat { spectral, .. } | Self::Spatial { spectral, .. } => spectral.as_ref(),
        };
        if let Some(spectral) = spectral {
            spectral.set_acoustic(acoustic);
        }
    }

    #[inline]
    fn set_environment(&self, environment_state: AudioEnvironmentState) {
        let environment = match self {
            Self::Flat { environment, .. } | Self::Spatial { environment, .. } => {
                environment.as_ref()
            }
        };
        if let Some(environment) = environment {
            environment.set_environment(environment_state);
        }
    }

    #[inline]
    fn late_binding(&self) -> Option<&RoomBusVoiceBinding> {
        match self {
            Self::Flat { late_binding, .. } | Self::Spatial { late_binding, .. } => {
                late_binding.as_ref()
            }
        }
    }

    #[inline]
    fn stop(&self) {
        match self {
            Self::Flat { player, .. } => player.stop(),
            Self::Spatial { player, .. } => player.stop(),
        }
    }

    #[inline]
    fn empty(&self) -> bool {
        match self {
            Self::Flat { player, .. } => player.empty(),
            Self::Spatial { player, .. } => player.empty(),
        }
    }

    #[inline]
    fn get_pos(&self) -> Duration {
        match self {
            Self::Flat { player, .. } => player.get_pos(),
            Self::Spatial { player, .. } => player.get_pos(),
        }
    }

    fn try_seek(&self, position: Duration) -> Result<(), String> {
        match self {
            Self::Flat { player, .. } => player.try_seek(position),
            Self::Spatial { player, .. } => player.try_seek(position),
        }
        .map_err(|error| format!("audio voice seek failed: {error}"))
    }
}

#[derive(Clone)]
enum VoiceSource {
    Clip {
        uri: String,
        source_duration: Option<Duration>,
    },
    Stream {
        uri: String,
        buffer: AudioStreamBufferConfig,
        source_duration: Option<Duration>,
    },
    Tone {
        frequency: f32,
        duration: Duration,
    },
}

impl VoiceSource {
    #[inline]
    fn source_duration(&self) -> Option<Duration> {
        match self {
            Self::Clip {
                source_duration, ..
            } => *source_duration,
            Self::Stream { source_duration, .. } => *source_duration,
            Self::Tone { duration, .. } => Some(*duration),
        }
    }

    #[inline]
    fn virtualizable(&self) -> bool {
        matches!(
            self,
            Self::Clip {
                source_duration: Some(_),
                ..
            } | Self::Stream { .. }
        )
    }
}

fn physical_source_position(
    output_position: Duration,
    effective_speed: f32,
    source_origin: Duration,
) -> Duration {
    source_origin.saturating_add(output_position.mul_f32(effective_speed))
}

fn normalize_timeline_position(
    position: Duration,
    source_duration: Option<Duration>,
    looping: bool,
) -> Duration {
    let Some(duration) = source_duration else {
        return position;
    };
    if duration.is_zero() {
        return Duration::ZERO;
    }
    if looping {
        Duration::from_secs_f64(position.as_secs_f64() % duration.as_secs_f64())
    } else {
        position.min(duration)
    }
}

struct VoiceEntry {
    control: Option<VoiceControl>,
    source: VoiceSource,
    bus: AudioBus,
    gain: f32,
    speed: f32,
    looping: bool,
    spatial: Option<AudioSpatialParams>,
    attenuation: Option<AudioAttenuationSettings>,
    acoustic: AudioAcousticState,
    propagation: AudioPropagationState,
    emitter_velocity: [f32; 3],
    last_spatial_update: Option<Instant>,
    environment: AudioEnvironmentState,
    stream_stats: Option<Arc<StreamingStats>>,
    /// Absolute media timeline position corresponding to physical Player::get_pos() == 0.
    /// Non-stream sources keep this at zero; rebuilt streams set it to their resume point.
    physical_source_origin: Duration,
    concurrency_group: String,
    concurrency_scope: AudioConcurrencyScope,
    concurrency_scope_id: Option<u64>,
    policy_instance_id: u64,
    voice_budget: String,
    priority: i32,
    paused: bool,
    /// Timeline position in source time, independent of playback speed.
    virtual_source_position: Duration,
    virtual_since: Option<Instant>,
}

impl VoiceEntry {
    #[inline]
    fn is_physical(&self) -> bool {
        self.control.is_some()
    }

    #[inline]
    fn is_virtual(&self) -> bool {
        self.control.is_none()
    }

    #[inline]
    fn virtualizable(&self) -> bool {
        self.source.virtualizable()
    }

    fn normalized_source_position(&self, position: Duration) -> Duration {
        normalize_timeline_position(position, self.source.source_duration(), self.looping)
    }

    fn current_source_position(&self, now: Instant) -> Duration {
        if let Some(control) = self.control.as_ref() {
            return self.normalized_source_position(physical_source_position(
                control.get_pos(),
                self.effective_speed(),
                self.physical_source_origin,
            ));
        }
        let mut position = self.virtual_source_position;
        if !self.paused {
            if let Some(since) = self.virtual_since {
                position = position
                    .saturating_add(now.saturating_duration_since(since).mul_f32(self.speed));
            }
        }
        self.normalized_source_position(position)
    }

    fn freeze_virtual_timeline(&mut self, now: Instant) {
        if self.control.is_none() {
            self.virtual_source_position = self.current_source_position(now);
            self.virtual_since = None;
        }
    }

    fn resume_virtual_timeline(&mut self, now: Instant) {
        if self.control.is_none() && !self.paused {
            self.virtual_since = Some(now);
        }
    }

    fn is_finished(&self, now: Instant) -> bool {
        if let Some(control) = self.control.as_ref() {
            return !self.looping && control.empty();
        }
        if self.looping {
            return false;
        }
        self.source
            .source_duration()
            .is_some_and(|duration| self.current_source_position(now) >= duration)
    }

    #[inline]
    fn environment_audibility_gain(&self) -> f32 {
        let environment = self.environment.sanitized();
        let indirect = (environment.source_send.gain + environment.listener_send.gain) * 0.22;
        environment
            .portal_gain
            .max(indirect.clamp(0.0, 1.0))
            .clamp(0.0, 1.0)
    }

    #[inline]
    fn effective_speed(&self) -> f32 {
        match self.source {
            VoiceSource::Stream { .. } => 1.0,
            _ => sanitize_speed(self.speed * self.propagation.doppler_ratio),
        }
    }

    #[inline]
    fn propagated_acoustic(&self) -> AudioAcousticState {
        let acoustic = self.acoustic.sanitized();
        AudioAcousticState {
            obstruction: acoustic.obstruction,
            occlusion: acoustic.occlusion,
            transmission_gain: (acoustic.transmission_gain * self.propagation.air_gain)
                .clamp(0.0, 1.0),
            high_frequency_gain: (acoustic.high_frequency_gain
                * self.propagation.air_high_frequency_gain)
                .clamp(0.0, 1.0),
            low_pass_hz: acoustic.low_pass_hz.min(self.propagation.air_low_pass_hz),
        }
        .sanitized()
    }

    fn refresh_propagation(&mut self, listener: AudioListenerState, listener_velocity: [f32; 3]) {
        let target = propagation_state(
            listener,
            listener_velocity,
            self.spatial,
            self.emitter_velocity,
        );
        let previous_doppler = self.propagation.doppler_ratio;
        self.propagation = target;
        // Smooth only the pitch-rate term. Distance and air absorption follow geometry directly.
        self.propagation.doppler_ratio = if self.spatial.is_some() {
            previous_doppler + (target.doppler_ratio - previous_doppler) * 0.35
        } else {
            1.0
        };
        if let Some(control) = self.control.as_ref() {
            control.set_speed(self.effective_speed());
            control.set_acoustic(self.propagated_acoustic());
        }
    }

    fn update_emitter_motion(&mut self, position: [f32; 3], now: Instant) {
        if let Some(previous) = self.spatial.map(|spatial| spatial.position) {
            let dt = self
                .last_spatial_update
                .map(|last| now.saturating_duration_since(last).as_secs_f32())
                .unwrap_or(0.0);
            let target = estimate_velocity(previous, position, dt);
            self.emitter_velocity = smooth_velocity(self.emitter_velocity, target);
        } else {
            self.emitter_velocity = [0.0; 3];
        }
        self.last_spatial_update = Some(now);
        self.spatial = self.spatial.map(|_| AudioSpatialParams { position });
    }

    #[inline]
    fn distance_to(&self, listener: AudioListenerState) -> f32 {
        self.spatial
            .map(|spatial| distance3(spatial.position, listener.position))
            .unwrap_or(0.0)
    }

    #[inline]
    fn attenuation_gain(&self, listener: AudioListenerState) -> f32 {
        match (&self.attenuation, self.spatial) {
            (Some(attenuation), Some(spatial)) => {
                attenuation.gain_at_distance(distance3(spatial.position, listener.position))
            }
            _ => 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct VoiceRank {
    voice_id: u64,
    priority: i32,
    audibility: f32,
    distance: f32,
    already_physical: bool,
    budget: String,
}

fn sort_voice_ranks(ranks: &mut [VoiceRank]) {
    ranks.sort_unstable_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| b.audibility.total_cmp(&a.audibility))
            .then_with(|| a.distance.total_cmp(&b.distance))
            .then_with(|| b.already_physical.cmp(&a.already_physical))
            .then_with(|| a.voice_id.cmp(&b.voice_id))
    });
}

fn select_physical_voice_ids(
    mut ranks: Vec<VoiceRank>,
    max_physical_voices: usize,
    reservations: &BTreeMap<String, usize>,
) -> HashSet<u64> {
    sort_voice_ranks(&mut ranks);
    let mut selected = HashSet::with_capacity(max_physical_voices);

    // Reservations are project-authored opaque budget ids. Each class may claim up to its
    // reserved slots when it has eligible voices; unused reserved slots immediately return to
    // the shared pool. BTreeMap order keeps selection deterministic when several reservations
    // become active in the same frame.
    for (budget, reserved) in reservations {
        let mut remaining = (*reserved).min(max_physical_voices.saturating_sub(selected.len()));
        if remaining == 0 {
            break;
        }
        for rank in &ranks {
            if remaining == 0 {
                break;
            }
            if rank.budget == *budget && selected.insert(rank.voice_id) {
                remaining -= 1;
            }
        }
    }

    if selected.len() < max_physical_voices {
        for rank in &ranks {
            if selected.len() >= max_physical_voices {
                break;
            }
            selected.insert(rank.voice_id);
        }
    }
    selected
}
