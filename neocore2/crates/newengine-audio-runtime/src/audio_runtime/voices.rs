enum VoiceControl {
    Flat {
        player: Player,
        spectral: Option<SpectralFilterControl>,
        environment: Option<EnvironmentFilterControl>,
    },
    Spatial {
        player: SpatialPlayer,
        spectral: Option<SpectralFilterControl>,
        environment: Option<EnvironmentFilterControl>,
    },
}

impl VoiceControl {
    #[inline]
    fn set_volume(&self, value: f32) {
        match self {
            Self::Flat { player, .. } => player.set_volume(value),
            Self::Spatial { player, .. } => player.set_volume(value),
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
            Self::Spatial { player, .. } => {
                player.set_emitter_position(position);
                true
            }
            Self::Flat { .. } => false,
        }
    }

    #[inline]
    fn update_listener(&self, listener: AudioListenerState) {
        if let Self::Spatial { player, .. } = self {
            let (left, right) = listener.ear_positions();
            player.set_left_ear_position(left);
            player.set_right_ear_position(right);
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
            Self::Stream { .. } => None,
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
            }
        )
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
    environment: AudioEnvironmentState,
    stream_stats: Option<Arc<StreamingStats>>,
    concurrency_group: String,
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
        let Some(duration) = self.source.source_duration() else {
            return position;
        };
        if duration.is_zero() {
            return Duration::ZERO;
        }
        if self.looping {
            let duration_secs = duration.as_secs_f64();
            Duration::from_secs_f64(position.as_secs_f64() % duration_secs)
        } else {
            position.min(duration)
        }
    }

    fn current_source_position(&self, now: Instant) -> Duration {
        if let Some(control) = self.control.as_ref() {
            return self.normalized_source_position(control.get_pos().mul_f32(self.speed));
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

#[derive(Clone, Copy, Debug, PartialEq)]
struct VoiceRank {
    voice_id: u64,
    priority: i32,
    audibility: f32,
    distance: f32,
    already_physical: bool,
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
) -> HashSet<u64> {
    sort_voice_ranks(&mut ranks);
    ranks
        .into_iter()
        .take(max_physical_voices)
        .map(|rank| rank.voice_id)
        .collect()
}
