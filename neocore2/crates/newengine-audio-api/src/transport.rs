use serde::{Deserialize, Serialize};

use super::{
    AudioInstanceId, AudioObjectId, AudioParameterTarget, AudioPlayInstanceRequest,
    AudioPlayStreamInstanceRequest,
};

pub const AUDIO_TRANSPORT_SCHEMA: &str = "newengine.audio.transport.v1";
pub const AUDIO_TRANSPORT_CAPABILITY_ID: &str = "audio.transport";
pub const AUDIO_TRANSPORT_VERSION: u32 = 1;
pub const AUDIO_TRANSPORT_DEFAULT_SAMPLE_RATE: u32 = 48_000;
pub const AUDIO_TRANSPORT_MAX_MARKERS: usize = 4_096;
pub const AUDIO_TRANSPORT_MAX_SCHEDULED_ACTIONS: usize = 8_192;
const MICROBPM_PER_BPM: u128 = 1_000_000;
const SECONDS_PER_MINUTE: u128 = 60;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct AudioTransportActionId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioTempoGrid {
    /// Beats per minute in millionths. 120 BPM = 120_000_000.
    /// Integer storage keeps beat boundaries deterministic across platforms.
    pub micro_bpm: u64,
    pub beats_per_bar: u16,
    pub beat_unit: u16,
}

impl Default for AudioTempoGrid {
    fn default() -> Self {
        Self {
            micro_bpm: 120_000_000,
            beats_per_bar: 4,
            beat_unit: 4,
        }
    }
}

impl AudioTempoGrid {
    pub fn validate(self) -> Result<Self, String> {
        if !(1_000_000..=1_000_000_000).contains(&self.micro_bpm) {
            return Err("audio transport tempo must be in [1, 1000] BPM".to_owned());
        }
        if !(1..=32).contains(&self.beats_per_bar) {
            return Err("audio transport beats_per_bar must be in [1, 32]".to_owned());
        }
        if !matches!(self.beat_unit, 1 | 2 | 4 | 8 | 16 | 32 | 64) {
            return Err(
                "audio transport beat_unit must be a power-of-two note value in [1, 64]".to_owned(),
            );
        }
        Ok(self)
    }

    #[inline]
    fn beat_denominator(self) -> u128 {
        u128::from(self.micro_bpm)
    }

    #[inline]
    fn beat_numerator(self, sample_rate: u32) -> u128 {
        u128::from(sample_rate) * SECONDS_PER_MINUTE * MICROBPM_PER_BPM
    }

    pub fn beat_start_sample(self, sample_rate: u32, beat_index: u64) -> u64 {
        let numerator = u128::from(beat_index).saturating_mul(self.beat_numerator(sample_rate));
        let denominator = self.beat_denominator();
        let rounded_up = if numerator == 0 {
            0
        } else {
            numerator.saturating_add(denominator.saturating_sub(1)) / denominator
        };
        u64::try_from(rounded_up).unwrap_or(u64::MAX)
    }

    pub fn beat_index_at_sample(self, sample_rate: u32, sample: u64) -> u64 {
        let numerator = u128::from(sample).saturating_mul(self.beat_denominator());
        u64::try_from(numerator / self.beat_numerator(sample_rate)).unwrap_or(u64::MAX)
    }

    pub fn next_beat_sample(self, sample_rate: u32, sample: u64) -> u64 {
        let beat = self.beat_index_at_sample(sample_rate, sample);
        let current = self.beat_start_sample(sample_rate, beat);
        if current > sample {
            current
        } else {
            self.beat_start_sample(sample_rate, beat.saturating_add(1))
        }
    }

    pub fn next_bar_sample(self, sample_rate: u32, sample: u64) -> u64 {
        let beat = self.beat_index_at_sample(sample_rate, sample);
        let beats_per_bar = u64::from(self.beats_per_bar.max(1));
        let current_bar = beat / beats_per_bar;
        let next_bar_beat = current_bar.saturating_add(1).saturating_mul(beats_per_bar);
        self.beat_start_sample(sample_rate, next_bar_beat)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioTransportMarker {
    pub id: String,
    pub sample: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioTransportConfig {
    pub schema: String,
    pub version: u32,
    pub sample_rate: u32,
    pub tempo: AudioTempoGrid,
    pub markers: Vec<AudioTransportMarker>,
}

impl Default for AudioTransportConfig {
    fn default() -> Self {
        Self {
            schema: AUDIO_TRANSPORT_SCHEMA.to_owned(),
            version: AUDIO_TRANSPORT_VERSION,
            sample_rate: AUDIO_TRANSPORT_DEFAULT_SAMPLE_RATE,
            tempo: AudioTempoGrid::default(),
            markers: Vec::new(),
        }
    }
}

impl AudioTransportConfig {
    pub fn validate(mut self) -> Result<Self, String> {
        if self.schema != AUDIO_TRANSPORT_SCHEMA || self.version != AUDIO_TRANSPORT_VERSION {
            return Err(format!(
                "unsupported AudioTransport contract schema='{}' version={}",
                self.schema, self.version
            ));
        }
        if !(8_000..=384_000).contains(&self.sample_rate) {
            return Err("audio transport sample_rate must be in [8000, 384000]".to_owned());
        }
        self.tempo = self.tempo.validate()?;
        if self.markers.len() > AUDIO_TRANSPORT_MAX_MARKERS {
            return Err(format!(
                "audio transport marker count {} exceeds {}",
                self.markers.len(),
                AUDIO_TRANSPORT_MAX_MARKERS
            ));
        }
        let mut ids = std::collections::BTreeSet::<String>::new();
        for marker in &mut self.markers {
            marker.id = marker.id.trim().to_owned();
            validate_symbol("audio transport marker", &marker.id)?;
            if !ids.insert(marker.id.to_ascii_lowercase()) {
                return Err(format!("duplicate audio transport marker '{}'", marker.id));
            }
        }
        self.markers
            .sort_by(|a, b| a.sample.cmp(&b.sample).then_with(|| a.id.cmp(&b.id)));
        Ok(self)
    }

    pub fn marker(&self, id: &str) -> Option<&AudioTransportMarker> {
        self.markers
            .iter()
            .find(|marker| marker.id.eq_ignore_ascii_case(id))
    }

    pub fn position(&self, sample: u64) -> AudioTransportPosition {
        let beat = self.tempo.beat_index_at_sample(self.sample_rate, sample);
        let beats_per_bar = u64::from(self.tempo.beats_per_bar.max(1));
        AudioTransportPosition {
            sample,
            beat,
            bar: beat / beats_per_bar,
            beat_in_bar: u16::try_from(beat % beats_per_bar).unwrap_or(0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioTransportPosition {
    pub sample: u64,
    /// Zero-based absolute beat index.
    pub beat: u64,
    /// Zero-based bar index.
    pub bar: u64,
    /// Zero-based beat within current bar.
    pub beat_in_bar: u16,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "at", rename_all = "snake_case")]
pub enum AudioTransportSchedulePoint {
    #[default]
    Immediate,
    AbsoluteSample {
        sample: u64,
    },
    NextBeat,
    NextBar,
    Marker {
        id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AudioTransportAction {
    Play {
        instance_id: AudioInstanceId,
        object_id: AudioObjectId,
        request: AudioPlayInstanceRequest,
    },
    PlayStream {
        instance_id: AudioInstanceId,
        object_id: AudioObjectId,
        request: Box<AudioPlayStreamInstanceRequest>,
    },
    StopInstance {
        instance_id: AudioInstanceId,
    },
    SetScalar {
        target: AudioParameterTarget,
        name: String,
        value: f32,
    },
    SetSwitch {
        target: AudioParameterTarget,
        name: String,
        value: String,
    },
    TransitionScalar {
        target: AudioParameterTarget,
        name: String,
        target_value: f32,
        duration_samples: u64,
    },
    TransitionInstanceGain {
        instance_id: AudioInstanceId,
        target_gain: f32,
        duration_samples: u64,
    },
    TransitionSnapshot {
        snapshot: String,
        target_weight: f32,
        duration_samples: u64,
    },
}

impl AudioTransportAction {
    pub fn validate(mut self) -> Result<Self, String> {
        match &mut self {
            Self::Play {
                instance_id,
                object_id,
                request,
            } => {
                if instance_id.0 == 0 || object_id.0 == 0 {
                    return Err(
                        "audio transport Play requires non-zero object/instance ids".to_owned()
                    );
                }
                *request = request.clone().sanitized()?;
            }
            Self::PlayStream {
                instance_id,
                object_id,
                request,
            } => {
                if instance_id.0 == 0 || object_id.0 == 0 {
                    return Err(
                        "audio transport PlayStream requires non-zero object/instance ids"
                            .to_owned(),
                    );
                }
                **request = (**request).clone().sanitized()?;
            }
            Self::StopInstance { instance_id } => {
                if instance_id.0 == 0 {
                    return Err(
                        "audio transport StopInstance requires non-zero instance id".to_owned()
                    );
                }
            }
            Self::SetScalar { name, value, .. } => {
                validate_symbol("audio transport scalar", name)?;
                if !value.is_finite() {
                    return Err("audio transport scalar value must be finite".to_owned());
                }
                *name = name.trim().to_owned();
            }
            Self::SetSwitch { name, value, .. } => {
                validate_symbol("audio transport switch", name)?;
                validate_value("audio transport switch value", value)?;
                *name = name.trim().to_owned();
                *value = value.trim().to_owned();
            }
            Self::TransitionInstanceGain {
                instance_id,
                target_gain,
                ..
            } => {
                if instance_id.0 == 0 {
                    return Err(
                        "audio transport instance-gain transition requires non-zero instance id"
                            .to_owned(),
                    );
                }
                if !target_gain.is_finite() || !(0.0..=4.0).contains(target_gain) {
                    return Err("audio transport instance-gain target must be in [0, 4]".to_owned());
                }
            }
            Self::TransitionScalar {
                name, target_value, ..
            } => {
                validate_symbol("audio transport scalar transition", name)?;
                if !target_value.is_finite() {
                    return Err(
                        "audio transport scalar transition target must be finite".to_owned()
                    );
                }
                *name = name.trim().to_owned();
            }
            Self::TransitionSnapshot {
                snapshot,
                target_weight,
                ..
            } => {
                validate_symbol("audio transport snapshot", snapshot)?;
                if !target_weight.is_finite() || !(0.0..=1.0).contains(target_weight) {
                    return Err(
                        "audio transport snapshot target_weight must be in [0, 1]".to_owned()
                    );
                }
                *snapshot = snapshot.trim().to_owned();
            }
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioTransportMarkerOccurrence {
    pub id: String,
    pub sample: u64,
    pub position: AudioTransportPosition,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioTransportInstanceState {
    pub start_sample: u64,
    pub dispatch_sample: u64,
    pub logical_sample: u64,
    pub dispatch_lateness_samples: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioTransportRuntimeState {
    pub sample_rate: u32,
    pub sample: u64,
    pub beat: u64,
    pub bar: u64,
    pub beat_in_bar: u16,
    pub pending_actions: usize,
    pub active_transitions: usize,
    pub emitted_markers: u64,
    pub executed_actions: u64,
    pub late_actions: u64,
    pub max_lateness_samples: u64,
}

fn validate_symbol(kind: &str, value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(format!("{kind} is invalid"));
    }
    Ok(())
}

fn validate_value(kind: &str, value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(format!("{kind} is invalid"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rational_grid_is_deterministic_without_float_drift() {
        let tempo = AudioTempoGrid {
            micro_bpm: 123_000_000,
            beats_per_bar: 7,
            beat_unit: 8,
        }
        .validate()
        .unwrap();
        let sample_rate = 48_000;
        for beat in [0, 1, 2, 17, 10_000] {
            let sample = tempo.beat_start_sample(sample_rate, beat);
            assert_eq!(tempo.beat_index_at_sample(sample_rate, sample), beat);
            assert!(tempo.next_beat_sample(sample_rate, sample) > sample);
        }
    }

    #[test]
    fn config_rejects_duplicate_marker_ids_case_insensitively() {
        let config = AudioTransportConfig {
            markers: vec![
                AudioTransportMarker {
                    id: "Drop".to_owned(),
                    sample: 100,
                },
                AudioTransportMarker {
                    id: "drop".to_owned(),
                    sample: 200,
                },
            ],
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn position_reports_zero_based_bar_and_beat() {
        let config = AudioTransportConfig::default().validate().unwrap();
        let sample = config.tempo.beat_start_sample(config.sample_rate, 9);
        let position = config.position(sample);
        assert_eq!(position.beat, 9);
        assert_eq!(position.bar, 2);
        assert_eq!(position.beat_in_bar, 1);
    }

    #[test]
    fn transport_play_stream_preserves_generic_long_form_request() {
        let mut request = AudioPlayStreamInstanceRequest::new("shared/audio/music/stem.ogg");
        request.route = crate::AudioRouteId::new("project.music.stems");
        request.tags = vec!["project.layer.a".to_owned()];
        request.stream.concurrency_group = "project.music.group".to_owned();
        request.stream.voice_budget = "project.music.budget".to_owned();
        request.stream.priority = 77;
        let action = AudioTransportAction::PlayStream {
            instance_id: AudioInstanceId(11),
            object_id: AudioObjectId(22),
            request: Box::new(request),
        }
        .validate()
        .expect("valid transport stream action");
        let AudioTransportAction::PlayStream { request, .. } = action else {
            panic!("expected PlayStream");
        };
        assert_eq!(request.stream.clip.uri, "shared/audio/music/stem.ogg");
        assert_eq!(request.route.0, "project.music.stems");
        assert_eq!(request.stream.concurrency_group, "project.music.group");
        assert_eq!(request.stream.voice_budget, "project.music.budget");
        assert_eq!(request.stream.priority, 77);
    }
}
