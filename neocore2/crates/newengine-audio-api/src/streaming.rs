use serde::{Deserialize, Serialize};

use super::{
    AudioAcousticState, AudioAttenuationSettings, AudioBus, AudioClipRef, AudioConcurrencyScope,
    AudioEnvironmentState, AudioSpatialParams, AudioVoiceStealRule,
};

pub const AUDIO_AMBIENCE_BED_COMPONENT_TYPE: &str = "audio.ambience_bed";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioStreamBufferConfig {
    /// Total decoded PCM capacity in milliseconds.
    pub capacity_ms: u32,
    /// Producer attempts to prefill this much audio before normal playback catches up.
    pub prefill_ms: u32,
    /// Decoder work granularity. This is frames, not interleaved samples.
    pub producer_chunk_frames: u32,
    /// Compressed VFS read granularity used by the seekable ranged reader.
    pub compressed_chunk_bytes: u32,
    /// Maximum compressed bytes retained by one stream reader cache.
    pub compressed_cache_bytes: u32,
}

impl Default for AudioStreamBufferConfig {
    fn default() -> Self {
        Self {
            capacity_ms: 1_500,
            prefill_ms: 300,
            producer_chunk_frames: 2_048,
            compressed_chunk_bytes: 64 * 1024,
            compressed_cache_bytes: 512 * 1024,
        }
    }
}

impl AudioStreamBufferConfig {
    pub fn sanitized(self) -> Self {
        let capacity_ms = self.capacity_ms.clamp(250, 10_000);
        Self {
            capacity_ms,
            prefill_ms: self.prefill_ms.clamp(50, capacity_ms),
            producer_chunk_frames: self.producer_chunk_frames.clamp(256, 16_384),
            compressed_chunk_bytes: self.compressed_chunk_bytes.clamp(4 * 1024, 1024 * 1024),
            compressed_cache_bytes: self
                .compressed_cache_bytes
                .clamp(16 * 1024, 16 * 1024 * 1024)
                .max(self.compressed_chunk_bytes.clamp(4 * 1024, 1024 * 1024)),
        }
    }
}

/// Provider-neutral request for long-form audio. `stop_voice_json_v1` and
/// `set_voice_json_v1` operate on the returned voice id exactly like clip voices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioStreamPlayRequest {
    pub version: u32,
    pub clip: AudioClipRef,
    pub bus: AudioBus,
    pub gain: f32,
    pub looping: bool,
    /// Optional initial timeline position for resumable music/ambience streams.
    pub start_seconds: f64,
    pub spatial: Option<AudioSpatialParams>,
    pub attenuation: Option<AudioAttenuationSettings>,
    pub acoustic: AudioAcousticState,
    pub environment: AudioEnvironmentState,
    pub buffer: AudioStreamBufferConfig,
    pub concurrency_group: String,
    pub concurrency_limit: usize,
    pub concurrency_scope: AudioConcurrencyScope,
    pub steal_rule: AudioVoiceStealRule,
    pub voice_budget: String,
    /// Opaque owner/object identity used by object-scoped concurrency.
    pub scope_id: Option<u64>,
    pub priority: i32,
}

impl Default for AudioStreamPlayRequest {
    fn default() -> Self {
        Self {
            version: 1,
            clip: AudioClipRef::new(String::new()),
            bus: AudioBus::Ambience,
            gain: 1.0,
            looping: true,
            start_seconds: 0.0,
            spatial: None,
            attenuation: None,
            acoustic: AudioAcousticState::clear(),
            environment: AudioEnvironmentState::clear(),
            buffer: AudioStreamBufferConfig::default(),
            concurrency_group: String::new(),
            concurrency_limit: 1,
            concurrency_scope: AudioConcurrencyScope::Global,
            steal_rule: AudioVoiceStealRule::LowerPriorityThenOldest,
            voice_budget: String::new(),
            scope_id: None,
            priority: 32,
        }
    }
}

impl AudioStreamPlayRequest {
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            clip: AudioClipRef::new(uri),
            ..Self::default()
        }
    }

    pub fn sanitized(mut self) -> Self {
        self.clip.uri = self.clip.uri.trim().replace('\\', "/");
        while self.clip.uri.starts_with('/') {
            self.clip.uri.remove(0);
        }
        self.gain = finite_clamped(self.gain, 1.0, 0.0, 4.0);
        self.start_seconds = if self.start_seconds.is_finite() {
            self.start_seconds.clamp(0.0, 86_400.0)
        } else {
            0.0
        };
        self.spatial = self.spatial.map(AudioSpatialParams::sanitized);
        self.attenuation = self.attenuation.map(AudioAttenuationSettings::sanitized);
        self.acoustic = self.acoustic.sanitized();
        self.environment = self.environment.sanitized();
        self.buffer = self.buffer.sanitized();
        self.concurrency_group = self.concurrency_group.trim().to_owned();
        self.concurrency_limit = self.concurrency_limit.clamp(1, 4096);
        self.voice_budget = self.voice_budget.trim().to_ascii_lowercase();
        self.priority = self.priority.clamp(-100_000, 100_000);
        self.scope_id = self.scope_id.filter(|id| *id != 0);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioAmbienceScope {
    #[default]
    Global,
    Indoor,
    Outdoor,
    Zones,
}

/// Durable authored long-form ambience layer. `spatial=false` is a background bed;
/// `spatial=true` turns the owner Transform into an ambient emitter while retaining
/// the same environment/portal gating policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioAmbienceBed {
    pub bed_id: String,
    pub enabled: bool,
    pub stream: AudioClipRef,
    pub scope: AudioAmbienceScope,
    pub zones: Vec<String>,
    pub gain: f32,
    pub fade_seconds: f32,
    pub portal_bleed: f32,
    pub spatial: bool,
    pub looping: bool,
    pub priority: i32,
    pub attenuation: Option<AudioAttenuationSettings>,
    pub buffer: AudioStreamBufferConfig,
}

impl Default for AudioAmbienceBed {
    fn default() -> Self {
        Self {
            bed_id: "ambience.default".to_owned(),
            enabled: true,
            stream: AudioClipRef::new(String::new()),
            scope: AudioAmbienceScope::Global,
            zones: Vec::new(),
            gain: 1.0,
            fade_seconds: 1.5,
            portal_bleed: 0.35,
            spatial: false,
            looping: true,
            priority: 32,
            attenuation: None,
            buffer: AudioStreamBufferConfig::default(),
        }
    }
}

impl AudioAmbienceBed {
    pub fn new(bed_id: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            bed_id: bed_id.into(),
            stream: AudioClipRef::new(uri),
            ..Self::default()
        }
        .sanitized()
    }

    pub fn sanitized(mut self) -> Self {
        self.bed_id = self.bed_id.trim().to_owned();
        if self.bed_id.is_empty() {
            self.bed_id = "ambience.default".to_owned();
        }
        self.stream.uri = self.stream.uri.trim().replace('\\', "/");
        while self.stream.uri.starts_with('/') {
            self.stream.uri.remove(0);
        }
        self.zones = self
            .zones
            .into_iter()
            .map(|zone| zone.trim().to_owned())
            .filter(|zone| !zone.is_empty())
            .collect();
        self.zones.sort();
        self.zones.dedup();
        self.gain = finite_clamped(self.gain, 1.0, 0.0, 4.0);
        self.fade_seconds = finite_clamped(self.fade_seconds, 1.5, 0.02, 30.0);
        self.portal_bleed = finite_clamped(self.portal_bleed, 0.35, 0.0, 1.0);
        self.priority = self.priority.clamp(-100_000, 100_000);
        self.attenuation = self.attenuation.map(AudioAttenuationSettings::sanitized);
        self.buffer = self.buffer.sanitized();
        self
    }
}

fn finite_clamped(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_buffer_policy_is_bounded_and_prefill_never_exceeds_capacity() {
        let config = AudioStreamBufferConfig {
            capacity_ms: 20_000,
            prefill_ms: 30_000,
            producer_chunk_frames: 1,
            compressed_chunk_bytes: 1,
            compressed_cache_bytes: 1,
        }
        .sanitized();
        assert_eq!(config.capacity_ms, 10_000);
        assert_eq!(config.prefill_ms, 10_000);
        assert_eq!(config.producer_chunk_frames, 256);
    }

    #[test]
    fn ambience_bed_sanitizes_zone_membership_and_stream_path() {
        let mut bed = AudioAmbienceBed::new(" wind ", "/shared\\audio\\wind.ogg");
        bed.scope = AudioAmbienceScope::Zones;
        bed.zones = vec![
            " room.b ".to_owned(),
            "room.a".to_owned(),
            "room.a".to_owned(),
        ];
        let bed = bed.sanitized();
        assert_eq!(bed.bed_id, "wind");
        assert_eq!(bed.stream.uri, "shared/audio/wind.ogg");
        assert_eq!(bed.zones, vec!["room.a", "room.b"]);
    }
}
