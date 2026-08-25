#![forbid(unsafe_op_in_unsafe_fn)]

mod streaming_asset;
mod streaming_pcm;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Cursor;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use abi_stable::std_types::{RResult, RString};
use newengine_assets_api::AssetServiceClient;
use newengine_audio_api::{
    sanitize_gain, sanitize_speed, AudioAcousticState, AudioAttenuationSettings, AudioBus,
    AudioBusGainAck, AudioBusGainRequest, AudioCuePlayRequest, AudioCuePreloadRequest,
    AudioDiagnostics, AudioEnvironmentState, AudioFeedbackAck, AudioFeedbackEvent,
    AudioListenerState, AudioPlayAck, AudioPlayRequest, AudioPreloadAck, AudioPreloadRequest,
    AudioReverbSend, AudioServiceInfo, AudioSpatialParams, AudioStopVoiceRequest,
    AudioStreamBufferConfig, AudioStreamPlayRequest, AudioVoiceAck, AudioVoiceUpdateRequest,
    SoundCue, SoundCueClip, SoundCueSpatialPolicy, AUDIO_BACKEND_CAPABILITY_ID,
    AUDIO_PROVIDER_ABI_ID, AUDIO_SERVICE_ID, AUDIO_SERVICE_METHOD_DIAGNOSTICS_JSON_V1,
    AUDIO_SERVICE_METHOD_INVOKE, AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1,
    AUDIO_SERVICE_METHOD_PLAY_CUE_JSON_V1, AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1,
    AUDIO_SERVICE_METHOD_PLAY_STREAM_JSON_V1, AUDIO_SERVICE_METHOD_PRELOAD_CLIP_JSON_V1,
    AUDIO_SERVICE_METHOD_PRELOAD_CUE_JSON_V1, AUDIO_SERVICE_METHOD_SET_BUS_GAIN_JSON_V1,
    AUDIO_SERVICE_METHOD_SET_LISTENER_JSON_V1, AUDIO_SERVICE_METHOD_SET_VOICE_JSON_V1,
    AUDIO_SERVICE_METHOD_SHUTDOWN_V1, AUDIO_SERVICE_METHOD_STOP_VOICE_JSON_V1,
    ENGINE_AUDIO_SERVICE_ID,
};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json,
    register_engine_gateway_provider_service, EngineGatewayProviderDecl, JsonServiceRouter,
};
use rodio::source::{SeekError, SineWave, Source};
use rodio::stream::{DeviceSinkBuilder, MixerDeviceSink};
use rodio::{ChannelCount, Decoder, Player, SampleRate, SpatialPlayer};

use streaming_asset::RangedAssetReader;
use streaming_pcm::{build_streaming_source, StreamingStats};

pub const NATIVE_AUDIO_SERVICE_ID: &str = AUDIO_SERVICE_ID;
pub const NATIVE_AUDIO_PROVIDER_ROUTE: &str = "engine.audio.native";
pub const NATIVE_AUDIO_OWNER: &str = "newengine-audio-runtime";
pub const NATIVE_AUDIO_PRIORITY: i32 = 100;

const DEFAULT_CLIP_CACHE_LIMIT_BYTES: usize = 128 * 1024 * 1024;
const DEFAULT_UI_TONE_GAIN: f32 = 0.10;
const DEFAULT_MAX_PHYSICAL_VOICES: usize = 64;
const MAX_CONFIGURED_PHYSICAL_VOICES: usize = 512;
const MIN_PHYSICAL_AUDIBILITY: f32 = 1.0e-4;
/// Symphonia-backed decoders can reject sub-frame/sub-packet random access near zero.
/// A voice promoted this early is perceptually equivalent to starting at sample zero.
const MIN_MATERIALIZE_SEEK_MS: u64 = 50;
const UI_FEEDBACK_PRIORITY: i32 = 10_000;

#[derive(Debug)]
struct CachedClip {
    bytes: Arc<[u8]>,
    source_duration: OnceLock<Option<Duration>>,
}

impl CachedClip {
    #[inline]
    fn len(&self) -> usize {
        self.bytes.len()
    }
}

#[derive(Clone, Debug)]
struct EmbeddedYscdClipLocator {
    dictionary_path: String,
    cue_name: String,
    clip_index: usize,
}

#[derive(Clone, Debug)]
struct YscdRuntimeLayer {
    name: String,
    role: String,
    clips: Vec<SoundCueClip>,
    gain: f32,
    pitch: f32,
    attenuation: Option<AudioAttenuationSettings>,
}

#[derive(Clone, Debug)]
struct YscdRuntimeMeta {
    dictionary_path: String,
    cue_name: String,
    embedded_bytes: usize,
}

#[derive(Clone, Debug)]
struct SpectralFilterControl {
    low_pass_bits: Arc<AtomicU32>,
    high_frequency_gain_bits: Arc<AtomicU32>,
}

impl SpectralFilterControl {
    fn new(acoustic: AudioAcousticState) -> Self {
        let acoustic = acoustic.sanitized();
        Self {
            low_pass_bits: Arc::new(AtomicU32::new(acoustic.low_pass_hz.to_bits())),
            high_frequency_gain_bits: Arc::new(AtomicU32::new(
                acoustic.high_frequency_gain.to_bits(),
            )),
        }
    }

    #[inline]
    fn set_acoustic(&self, acoustic: AudioAcousticState) {
        let acoustic = acoustic.sanitized();
        self.low_pass_bits
            .store(acoustic.low_pass_hz.to_bits(), Ordering::Relaxed);
        self.high_frequency_gain_bits
            .store(acoustic.high_frequency_gain.to_bits(), Ordering::Relaxed);
    }

    #[inline]
    fn low_pass_hz(&self) -> f32 {
        f32::from_bits(self.low_pass_bits.load(Ordering::Relaxed)).clamp(80.0, 20_000.0)
    }

    #[inline]
    fn high_frequency_gain(&self) -> f32 {
        f32::from_bits(self.high_frequency_gain_bits.load(Ordering::Relaxed)).clamp(0.0, 1.0)
    }
}

/// Runtime-adjustable spectral transmission filter. It implements a one-pole
/// low-pass per channel and blends the removed high-frequency residual back by
/// `high_frequency_gain`, approximating a material-dependent high shelf without
/// rebuilding the physical voice source chain.
struct DynamicSpectralSource<I> {
    input: I,
    control: SpectralFilterControl,
    low_state: Vec<f32>,
    channel_index: usize,
    cached_cutoff_bits: u32,
    cached_alpha: f32,
}

impl<I> DynamicSpectralSource<I>
where
    I: Source<Item = f32>,
{
    fn new(input: I, control: SpectralFilterControl) -> Self {
        let channels = usize::from(input.channels().get()).max(1);
        Self {
            input,
            control,
            low_state: vec![0.0; channels],
            channel_index: 0,
            cached_cutoff_bits: u32::MAX,
            cached_alpha: 1.0,
        }
    }

    #[inline]
    fn alpha(&mut self) -> f32 {
        let cutoff = self.control.low_pass_hz();
        let bits = cutoff.to_bits();
        if bits != self.cached_cutoff_bits {
            let sample_rate = self.input.sample_rate().get() as f32;
            let cutoff = cutoff.min(sample_rate * 0.49).max(1.0);
            self.cached_alpha = 1.0 - (-std::f32::consts::TAU * cutoff / sample_rate).exp();
            self.cached_cutoff_bits = bits;
        }
        self.cached_alpha.clamp(0.0, 1.0)
    }

    fn reset_filter_state(&mut self) {
        self.low_state.fill(0.0);
        self.channel_index = 0;
    }
}

impl<I> Iterator for DynamicSpectralSource<I>
where
    I: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.input.next()?;
        let channels = self.low_state.len().max(1);
        let channel = self.channel_index.min(channels - 1);
        let alpha = self.alpha();
        let low = self.low_state[channel] + alpha * (sample - self.low_state[channel]);
        self.low_state[channel] = low;
        self.channel_index = (self.channel_index + 1) % channels;
        let high_gain = self.control.high_frequency_gain();
        Some(low + (sample - low) * high_gain)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.input.size_hint()
    }
}

impl<I> Source for DynamicSpectralSource<I>
where
    I: Source<Item = f32>,
{
    #[inline]
    fn current_span_len(&self) -> Option<usize> {
        self.input.current_span_len()
    }

    #[inline]
    fn channels(&self) -> ChannelCount {
        self.input.channels()
    }

    #[inline]
    fn sample_rate(&self) -> SampleRate {
        self.input.sample_rate()
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.input.try_seek(pos)?;
        self.reset_filter_state();
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct ReverbSendSnapshot {
    gain: f32,
    early_reflections_gain: f32,
    pre_delay_ms: f32,
    decay_seconds: f32,
    damping: f32,
    diffusion: f32,
}

impl ReverbSendSnapshot {
    fn from_send(send: AudioReverbSend) -> Self {
        let send = send.sanitized();
        Self {
            gain: send.gain,
            early_reflections_gain: send.preset.early_reflections_gain,
            pre_delay_ms: send.preset.pre_delay_ms,
            decay_seconds: send.preset.decay_seconds,
            damping: send.preset.damping,
            diffusion: send.preset.diffusion,
        }
    }
}

#[derive(Clone, Debug)]
struct ReverbSendControl {
    gain_bits: Arc<AtomicU32>,
    early_bits: Arc<AtomicU32>,
    pre_delay_bits: Arc<AtomicU32>,
    decay_bits: Arc<AtomicU32>,
    damping_bits: Arc<AtomicU32>,
    diffusion_bits: Arc<AtomicU32>,
}

impl ReverbSendControl {
    fn new(send: AudioReverbSend) -> Self {
        let snapshot = ReverbSendSnapshot::from_send(send);
        Self {
            gain_bits: Arc::new(AtomicU32::new(snapshot.gain.to_bits())),
            early_bits: Arc::new(AtomicU32::new(snapshot.early_reflections_gain.to_bits())),
            pre_delay_bits: Arc::new(AtomicU32::new(snapshot.pre_delay_ms.to_bits())),
            decay_bits: Arc::new(AtomicU32::new(snapshot.decay_seconds.to_bits())),
            damping_bits: Arc::new(AtomicU32::new(snapshot.damping.to_bits())),
            diffusion_bits: Arc::new(AtomicU32::new(snapshot.diffusion.to_bits())),
        }
    }

    fn set(&self, send: AudioReverbSend) {
        let snapshot = ReverbSendSnapshot::from_send(send);
        self.gain_bits
            .store(snapshot.gain.to_bits(), Ordering::Relaxed);
        self.early_bits
            .store(snapshot.early_reflections_gain.to_bits(), Ordering::Relaxed);
        self.pre_delay_bits
            .store(snapshot.pre_delay_ms.to_bits(), Ordering::Relaxed);
        self.decay_bits
            .store(snapshot.decay_seconds.to_bits(), Ordering::Relaxed);
        self.damping_bits
            .store(snapshot.damping.to_bits(), Ordering::Relaxed);
        self.diffusion_bits
            .store(snapshot.diffusion.to_bits(), Ordering::Relaxed);
    }

    fn snapshot(&self) -> ReverbSendSnapshot {
        ReverbSendSnapshot {
            gain: f32::from_bits(self.gain_bits.load(Ordering::Relaxed)).clamp(0.0, 2.0),
            early_reflections_gain: f32::from_bits(self.early_bits.load(Ordering::Relaxed))
                .clamp(0.0, 2.0),
            pre_delay_ms: f32::from_bits(self.pre_delay_bits.load(Ordering::Relaxed))
                .clamp(0.0, 250.0),
            decay_seconds: f32::from_bits(self.decay_bits.load(Ordering::Relaxed))
                .clamp(0.05, 20.0),
            damping: f32::from_bits(self.damping_bits.load(Ordering::Relaxed)).clamp(0.0, 1.0),
            diffusion: f32::from_bits(self.diffusion_bits.load(Ordering::Relaxed)).clamp(0.0, 1.0),
        }
    }
}

#[derive(Clone, Debug)]
struct EnvironmentFilterControl {
    source: ReverbSendControl,
    listener: ReverbSendControl,
}

impl EnvironmentFilterControl {
    fn new(environment: AudioEnvironmentState) -> Self {
        let environment = environment.sanitized();
        Self {
            source: ReverbSendControl::new(environment.source_send),
            listener: ReverbSendControl::new(environment.listener_send),
        }
    }

    #[inline]
    fn set_environment(&self, environment: AudioEnvironmentState) {
        let environment = environment.sanitized();
        self.source.set(environment.source_send);
        self.listener.set(environment.listener_send);
    }
}

struct ReverbTank {
    history: Vec<f32>,
    write_index: usize,
    damped_feedback: Vec<f32>,
    channels: usize,
    sample_rate: f32,
}

impl ReverbTank {
    fn new(sample_rate: SampleRate, channels: ChannelCount) -> Self {
        let channels = usize::from(channels.get()).max(1);
        let sample_rate = sample_rate.get() as f32;
        let max_delay_frames = (sample_rate * 0.40).ceil() as usize + 8;
        Self {
            history: vec![0.0; max_delay_frames * channels],
            write_index: 0,
            damped_feedback: vec![0.0; channels],
            channels,
            sample_rate,
        }
    }

    fn reset(&mut self) {
        self.history.fill(0.0);
        self.damped_feedback.fill(0.0);
        self.write_index = 0;
    }

    #[inline]
    fn read_delay(&self, delay_frames: usize) -> f32 {
        let offset = delay_frames
            .saturating_mul(self.channels)
            .min(self.history.len().saturating_sub(self.channels));
        let index = (self.write_index + self.history.len() - offset) % self.history.len();
        self.history[index]
    }

    fn process(&mut self, input: f32, params: ReverbSendSnapshot) -> f32 {
        if self.history.is_empty() {
            return 0.0;
        }
        let channel = self.write_index % self.channels;
        let pre_delay = ((params.pre_delay_ms * 0.001 * self.sample_rate).round() as usize)
            .min((self.sample_rate * 0.25) as usize);
        let diffusion = params.diffusion.clamp(0.0, 1.0);
        let delay_a = pre_delay + (self.sample_rate * (0.021 + diffusion * 0.011)) as usize;
        let delay_b = pre_delay + (self.sample_rate * (0.037 + diffusion * 0.019)) as usize;
        let early_delay = pre_delay.max(1);
        let tap_a = self.read_delay(delay_a.max(1));
        let tap_b = self.read_delay(delay_b.max(1));
        let early = self.read_delay(early_delay);
        let diffuse = tap_a * (0.65 - diffusion * 0.15) + tap_b * (0.35 + diffusion * 0.15);
        let damping_alpha = (0.05 + (1.0 - params.damping) * 0.90).clamp(0.05, 0.95);
        let damped = self.damped_feedback[channel]
            + damping_alpha * (diffuse - self.damped_feedback[channel]);
        self.damped_feedback[channel] = damped;
        let delay_seconds = ((delay_a + delay_b) as f32 * 0.5 / self.sample_rate).max(0.001);
        let feedback = 0.001_f32
            .powf(delay_seconds / params.decay_seconds.max(0.05))
            .clamp(0.0, 0.985);
        self.history[self.write_index] = input + damped * feedback;
        self.write_index = (self.write_index + 1) % self.history.len();
        early * params.early_reflections_gain + diffuse * (0.45 + diffusion * 0.35)
    }
}

/// Two independent dynamic room sends are rendered from the same dry source. The
/// control values are atomically replaceable; delay/feedback history remains attached
/// to the physical voice and is reset only by an actual source seek.
struct DynamicEnvironmentSource<I> {
    input: I,
    control: EnvironmentFilterControl,
    source_tank: ReverbTank,
    listener_tank: ReverbTank,
    source_params: ReverbSendSnapshot,
    listener_params: ReverbSendSnapshot,
    control_countdown: u8,
}

impl<I> DynamicEnvironmentSource<I>
where
    I: Source<Item = f32>,
{
    fn new(input: I, control: EnvironmentFilterControl) -> Self {
        let sample_rate = input.sample_rate();
        let channels = input.channels();
        let source_params = control.source.snapshot();
        let listener_params = control.listener.snapshot();
        Self {
            input,
            control,
            source_tank: ReverbTank::new(sample_rate, channels),
            listener_tank: ReverbTank::new(sample_rate, channels),
            source_params,
            listener_params,
            control_countdown: 0,
        }
    }

    fn refresh_controls(&mut self) {
        if self.control_countdown == 0 {
            self.source_params = self.control.source.snapshot();
            self.listener_params = self.control.listener.snapshot();
            self.control_countdown = 63;
        } else {
            self.control_countdown -= 1;
        }
    }

    fn reset_environment_state(&mut self) {
        self.source_tank.reset();
        self.listener_tank.reset();
        self.control_countdown = 0;
    }
}

impl<I> Iterator for DynamicEnvironmentSource<I>
where
    I: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let dry = self.input.next()?;
        self.refresh_controls();
        let source_wet =
            self.source_tank.process(dry, self.source_params) * self.source_params.gain;
        let listener_wet =
            self.listener_tank.process(dry, self.listener_params) * self.listener_params.gain;
        Some(dry + source_wet + listener_wet)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.input.size_hint()
    }
}

impl<I> Source for DynamicEnvironmentSource<I>
where
    I: Source<Item = f32>,
{
    #[inline]
    fn current_span_len(&self) -> Option<usize> {
        self.input.current_span_len()
    }

    #[inline]
    fn channels(&self) -> ChannelCount {
        self.input.channels()
    }

    #[inline]
    fn sample_rate(&self) -> SampleRate {
        self.input.sample_rate()
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.input.try_seek(pos)?;
        self.reset_environment_state();
        Ok(())
    }
}

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

pub struct AudioRuntimeState {
    assets: AssetServiceClient,
    output: Option<MixerDeviceSink>,
    output_rx: Option<mpsc::Receiver<Result<MixerDeviceSink, String>>>,
    output_error: Option<String>,
    output_init_started: bool,
    voices: HashMap<u64, VoiceEntry>,
    next_voice_id: u64,
    cue_counter: u64,
    listener: AudioListenerState,
    bus_gains: BTreeMap<AudioBus, f32>,
    clips: HashMap<String, CachedClip>,
    cues: HashMap<String, SoundCue>,
    cue_layers: HashMap<String, Vec<YscdRuntimeLayer>>,
    cue_meta: HashMap<String, YscdRuntimeMeta>,
    embedded_yscd_clips: HashMap<String, EmbeddedYscdClipLocator>,
    materialization_errors: HashMap<u64, String>,
    cached_bytes: usize,
    cache_limit_bytes: usize,
    max_physical_voices: usize,
}

impl AudioRuntimeState {
    /// Creates only the semantic/provider state. Physical device initialization is lazy and
    /// never starts from plugin/DLL init; Windows audio APIs may load COM/MMDevAPI modules and
    /// are not safe to initialize under the plugin loader lifecycle.
    pub fn open_default(assets: AssetServiceClient) -> Result<Self, String> {
        let mut bus_gains = BTreeMap::new();
        for bus in AudioBus::all() {
            bus_gains.insert(bus, 1.0);
        }
        Ok(Self {
            assets,
            output: None,
            output_rx: None,
            output_error: None,
            output_init_started: false,
            voices: HashMap::new(),
            next_voice_id: 1,
            cue_counter: 1,
            listener: AudioListenerState::default(),
            bus_gains,
            clips: HashMap::new(),
            cues: HashMap::new(),
            cue_layers: HashMap::new(),
            cue_meta: HashMap::new(),
            embedded_yscd_clips: HashMap::new(),
            materialization_errors: HashMap::new(),
            cached_bytes: 0,
            cache_limit_bytes: cache_limit_bytes_from_env(),
            max_physical_voices: max_physical_voices_from_env(),
        })
    }

    fn start_output_init(&mut self) {
        if self.output.is_some() || self.output_init_started || self.output_error.is_some() {
            return;
        }
        self.output_init_started = true;
        let (output_tx, output_rx) = mpsc::channel();
        match thread::Builder::new()
            .name("engine.audio.device-init".to_owned())
            .spawn(move || {
                newengine_ulog_api::ulog::info!(
                    "audio device init: phase='begin' thread='engine.audio.device-init' trigger='first-play'"
                );
                let result = DeviceSinkBuilder::open_default_sink()
                    .map_err(|error| format!("open default audio output failed: {error}"))
                    .map(|mut output| {
                        output.log_on_drop(false);
                        output
                    });
                let _ = output_tx.send(result);
            })
        {
            Ok(_) => {
                self.output_rx = Some(output_rx);
            }
            Err(error) => {
                self.output_error = Some(format!("spawn audio device init worker failed: {error}"));
            }
        }
    }

    fn poll_output_init(&mut self) {
        if self.output.is_some() {
            return;
        }
        let Some(receiver) = self.output_rx.as_ref() else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(output)) => {
                self.output = Some(output);
                self.output_rx = None;
                self.output_error = None;
                newengine_ulog_api::ulog::info!(
                    "audio device init: phase='ready' provider='{}'",
                    NATIVE_AUDIO_PROVIDER_ROUTE
                );
            }
            Ok(Err(error)) => {
                self.output_rx = None;
                self.output_error = Some(error.clone());
                newengine_ulog_api::ulog::warn!(
                    "audio device init: phase='failed' provider='{}' err='{}'",
                    NATIVE_AUDIO_PROVIDER_ROUTE,
                    error
                );
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.output_rx = None;
                self.output_error = Some("audio device init worker disconnected".to_owned());
                newengine_ulog_api::ulog::warn!(
                    "audio device init: phase='failed' provider='{}' err='worker disconnected'",
                    NATIVE_AUDIO_PROVIDER_ROUTE
                );
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    #[inline]
    fn output_ready(&mut self) -> bool {
        self.poll_output_init();
        self.output.is_some()
    }

    #[inline]
    fn alloc_voice_id(&mut self) -> u64 {
        let id = self.next_voice_id.max(1);
        self.next_voice_id = id.wrapping_add(1).max(1);
        id
    }

    fn remove_voice(&mut self, voice_id: u64) -> Option<VoiceEntry> {
        self.materialization_errors.remove(&voice_id);
        let voice = self.voices.remove(&voice_id)?;
        if let Some(control) = voice.control.as_ref() {
            control.stop();
        }
        Some(voice)
    }

    fn prune_finished(&mut self) {
        let now = Instant::now();
        let finished = self
            .voices
            .iter()
            .filter_map(|(voice_id, voice)| voice.is_finished(now).then_some(*voice_id))
            .collect::<Vec<_>>();
        for voice_id in finished {
            let _ = self.remove_voice(voice_id);
        }
    }

    #[inline]
    fn bus_gain(&self, bus: AudioBus) -> f32 {
        let master = self
            .bus_gains
            .get(&AudioBus::Master)
            .copied()
            .unwrap_or(1.0);
        if bus == AudioBus::Master {
            master
        } else {
            master * self.bus_gains.get(&bus).copied().unwrap_or(1.0)
        }
    }

    #[inline]
    fn voice_audibility(&self, voice: &VoiceEntry) -> f32 {
        sanitize_gain(voice.gain)
            * self.bus_gain(voice.bus)
            * voice.attenuation_gain(self.listener)
            * voice.acoustic.sanitized().transmission_gain
    }

    fn refresh_voice_gains(&self) {
        for voice in self.voices.values() {
            if let Some(control) = voice.control.as_ref() {
                control.set_volume(self.voice_audibility(voice));
            }
        }
    }

    fn preload(&mut self, request: AudioPreloadRequest) -> Result<AudioPreloadAck, String> {
        let uri = normalize_vfs_path(&request.clip.uri)?;
        if let Some(existing) = self.clips.get(&uri) {
            return Ok(AudioPreloadAck {
                accepted: true,
                cached: true,
                bytes: existing.len(),
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                diagnostics: Vec::new(),
            });
        }

        let bytes = if let Some(locator) = self.embedded_yscd_clips.get(&uri).cloned() {
            self.read_embedded_yscd_clip(&locator)?
        } else {
            self.assets
                .raw_bytes_v1(&uri)
                .map_err(|error| format!("audio VFS read failed logical_path='{uri}': {error}"))?
        };
        self.cache_clip_bytes(uri, bytes)
    }

    fn cache_clip_bytes(&mut self, uri: String, bytes: Vec<u8>) -> Result<AudioPreloadAck, String> {
        if let Some(existing) = self.clips.get(&uri) {
            return Ok(AudioPreloadAck {
                accepted: true,
                cached: true,
                bytes: existing.len(),
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                diagnostics: Vec::new(),
            });
        }
        if bytes.is_empty() {
            return Err(format!("audio clip is empty: '{uri}'"));
        }
        if bytes.len() > self.cache_limit_bytes {
            return Err(format!(
                "audio clip '{uri}' is {} bytes and exceeds cache limit {} bytes",
                bytes.len(),
                self.cache_limit_bytes
            ));
        }
        self.make_cache_room(bytes.len());
        let bytes: Arc<[u8]> = Arc::from(bytes.into_boxed_slice());
        let len = bytes.len();
        self.clips.insert(
            uri,
            CachedClip {
                bytes,
                source_duration: OnceLock::new(),
            },
        );
        self.cached_bytes = self.cached_bytes.saturating_add(len);
        Ok(AudioPreloadAck {
            accepted: true,
            cached: false,
            bytes: len,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            diagnostics: Vec::new(),
        })
    }

    fn read_embedded_yscd_clip(
        &self,
        locator: &EmbeddedYscdClipLocator,
    ) -> Result<Vec<u8>, String> {
        let source = self
            .assets
            .raw_bytes_v1(&locator.dictionary_path)
            .map_err(|error| {
                format!(
                    "YSCD VFS read failed dictionary='{}' cue='{}': {error}",
                    locator.dictionary_path, locator.cue_name
                )
            })?;
        let dictionary =
            newengine_asset_format_nef8::decode_yscd_nef8(&source, &locator.dictionary_path)?;
        let cue = dictionary.cue(&locator.cue_name).ok_or_else(|| {
            format!(
                "YSCD cue '{}' not found in '{}'",
                locator.cue_name, locator.dictionary_path
            )
        })?;
        cue.clips
            .get(locator.clip_index)
            .map(|clip| clip.bytes.clone())
            .ok_or_else(|| {
                format!(
                    "YSCD cue '{}' clip index {} out of range in '{}'",
                    locator.cue_name, locator.clip_index, locator.dictionary_path
                )
            })
    }

    fn make_cache_room(&mut self, incoming: usize) {
        if self.cached_bytes.saturating_add(incoming) <= self.cache_limit_bytes {
            return;
        }
        // V1 uses a deterministic all-or-nothing eviction. LRU/residency belongs
        // in the shared asset/VFS layer rather than leaking into the provider API.
        self.clips.clear();
        self.cues.clear();
        self.cue_layers.clear();
        self.cue_meta.clear();
        self.cached_bytes = 0;
    }

    fn clip_bytes(&mut self, uri: &str) -> Result<Arc<[u8]>, String> {
        let normalized = normalize_vfs_path(uri)?;
        if !self.clips.contains_key(&normalized) {
            self.preload(AudioPreloadRequest {
                clip: newengine_audio_api::AudioClipRef::new(normalized.clone()),
            })?;
        }
        self.clips
            .get(&normalized)
            .map(|clip| Arc::clone(&clip.bytes))
            .ok_or_else(|| format!("audio clip cache admission failed: '{normalized}'"))
    }

    fn clip_source_duration(&mut self, uri: &str) -> Result<Option<Duration>, String> {
        let normalized = normalize_vfs_path(uri)?;
        if !self.clips.contains_key(&normalized) {
            let _ = self.clip_bytes(&normalized)?;
        }
        if let Some(duration) = self
            .clips
            .get(&normalized)
            .and_then(|clip| clip.source_duration.get().copied())
        {
            return Ok(duration);
        }
        let bytes = self
            .clips
            .get(&normalized)
            .map(|clip| Arc::clone(&clip.bytes))
            .ok_or_else(|| format!("audio clip cache admission failed: '{normalized}'"))?;
        let decoder = Decoder::try_from(Cursor::new(bytes))
            .map_err(|error| format!("audio decode failed '{normalized}': {error}"))?;
        let duration = decoder.total_duration();
        if let Some(clip) = self.clips.get(&normalized) {
            let _ = clip.source_duration.set(duration);
        }
        Ok(duration)
    }

    fn play_clip(&mut self, request: AudioPlayRequest) -> Result<AudioPlayAck, String> {
        self.play_clip_with_policy(request, String::new(), 0)
    }

    fn play_clip_with_policy(
        &mut self,
        request: AudioPlayRequest,
        concurrency_group: String,
        priority: i32,
    ) -> Result<AudioPlayAck, String> {
        self.prune_finished();
        if !concurrency_group.is_empty() {
            let conflicts = self
                .voices
                .iter()
                .filter(|(_, voice)| voice.concurrency_group == concurrency_group)
                .map(|(id, voice)| (*id, voice.priority))
                .collect::<Vec<_>>();
            if conflicts
                .iter()
                .any(|(_, current_priority)| *current_priority > priority)
            {
                return Ok(AudioPlayAck {
                    accepted: false,
                    provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                    voice_id: None,
                    message: format!(
                        "concurrency group '{concurrency_group}' is occupied by a higher-priority voice"
                    ),
                    virtualized: false,
                    diagnostics: Vec::new(),
                });
            }
            for (voice_id, _) in conflicts {
                let _ = self.remove_voice(voice_id);
            }
        }

        let request = request.sanitized();
        let uri = normalize_vfs_path(&request.clip.uri)?;
        let source_duration = self.clip_source_duration(&uri)?;
        let voice_id = self.alloc_voice_id();
        let now = Instant::now();
        self.voices.insert(
            voice_id,
            VoiceEntry {
                control: None,
                source: VoiceSource::Clip {
                    uri,
                    source_duration,
                },
                bus: request.bus,
                gain: request.gain,
                speed: sanitize_speed(request.speed),
                looping: request.looping,
                spatial: request.spatial,
                attenuation: request.attenuation,
                acoustic: request.acoustic.sanitized(),
                environment: request.environment.sanitized(),
                stream_stats: None,
                concurrency_group,
                priority,
                paused: false,
                virtual_source_position: Duration::ZERO,
                virtual_since: Some(now),
            },
        );
        self.rebalance_physical_voices();

        let Some(voice) = self.voices.get(&voice_id) else {
            return Ok(AudioPlayAck {
                accepted: false,
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                voice_id: None,
                message: "physical voice budget exhausted for a non-virtualizable source"
                    .to_owned(),
                virtualized: false,
                diagnostics: Vec::new(),
            });
        };
        Ok(AudioPlayAck {
            accepted: true,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            voice_id: Some(voice_id),
            message: if voice.is_virtual() {
                "voice accepted as virtual; awaiting a physical mixer slot".to_owned()
            } else {
                String::new()
            },
            virtualized: voice.is_virtual(),
            diagnostics: Vec::new(),
        })
    }

    fn play_stream(&mut self, request: AudioStreamPlayRequest) -> Result<AudioPlayAck, String> {
        self.prune_finished();
        let request = request.sanitized();
        if request.version != 1 {
            return Err(format!(
                "unsupported AudioStreamPlayRequest version {}",
                request.version
            ));
        }
        if request.clip.uri.trim().is_empty() {
            return Err("streaming audio requires a non-empty VFS clip uri".to_owned());
        }
        if !request.concurrency_group.is_empty() {
            let conflicts = self
                .voices
                .iter()
                .filter(|(_, voice)| voice.concurrency_group == request.concurrency_group)
                .map(|(id, voice)| (*id, voice.priority))
                .collect::<Vec<_>>();
            if conflicts
                .iter()
                .any(|(_, current_priority)| *current_priority > request.priority)
            {
                return Ok(AudioPlayAck {
                    accepted: false,
                    provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                    voice_id: None,
                    message: format!(
                        "concurrency group '{}' is occupied by a higher-priority voice",
                        request.concurrency_group
                    ),
                    virtualized: false,
                    diagnostics: Vec::new(),
                });
            }
            for (voice_id, _) in conflicts {
                let _ = self.remove_voice(voice_id);
            }
        }

        let uri = normalize_vfs_path(&request.clip.uri)?;
        let voice_id = self.alloc_voice_id();
        self.voices.insert(
            voice_id,
            VoiceEntry {
                control: None,
                source: VoiceSource::Stream {
                    uri,
                    buffer: request.buffer,
                },
                bus: request.bus,
                gain: request.gain,
                speed: 1.0,
                looping: request.looping,
                spatial: request.spatial,
                attenuation: request.attenuation,
                acoustic: request.acoustic,
                environment: request.environment,
                stream_stats: None,
                concurrency_group: request.concurrency_group,
                priority: request.priority,
                paused: false,
                virtual_source_position: Duration::from_secs_f64(request.start_seconds),
                virtual_since: Some(Instant::now()),
            },
        );
        self.rebalance_physical_voices();

        let Some(voice) = self.voices.get(&voice_id) else {
            return Ok(AudioPlayAck {
                accepted: false,
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                voice_id: None,
                message: "physical voice budget exhausted for streaming source".to_owned(),
                virtualized: false,
                diagnostics: Vec::new(),
            });
        };
        Ok(AudioPlayAck {
            accepted: true,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            voice_id: Some(voice_id),
            message: String::new(),
            virtualized: voice.is_virtual(),
            diagnostics: Vec::new(),
        })
    }

    fn load_cue(&mut self, cue_reference: &str) -> Result<SoundCue, String> {
        let reference = newengine_assets_api::parse_asset_reference(cue_reference)
            .map_err(|error| format!("audio cue reference invalid '{cue_reference}': {error}"))?;
        if !reference.has_extension(newengine_asset_format_nef8::yscd::EXTENSION) {
            return Err(format!(
                "authored SoundCue JSON is retired; cue '{}' must use .yscd@entry",
                reference.canonical
            ));
        }
        reference.require_entry()?;
        let canonical = reference.canonical.clone();
        if let Some(cue) = self.cues.get(&canonical) {
            return Ok(cue.clone());
        }

        let source = self
            .assets
            .raw_bytes_v1(&reference.logical_path)
            .map_err(|error| {
                format!(
                    "YSCD VFS read failed logical_path='{}': {error}",
                    reference.logical_path
                )
            })?;
        let dictionary =
            newengine_asset_format_nef8::decode_yscd_nef8(&source, &reference.logical_path)?;
        let cue_name = reference.entry.as_deref().expect("entry required above");
        let authored = dictionary.cue(cue_name).ok_or_else(|| {
            format!(
                "YSCD cue '{}' not found in '{}'",
                cue_name, reference.logical_path
            )
        })?;

        let mut clips = Vec::with_capacity(authored.clips.len());
        let mut clips_by_name = HashMap::<String, SoundCueClip>::new();
        for (clip_index, clip) in authored.clips.iter().enumerate() {
            let key = embedded_yscd_clip_key(&canonical, clip_index, &clip.codec);
            self.embedded_yscd_clips.insert(
                key.clone(),
                EmbeddedYscdClipLocator {
                    dictionary_path: reference.logical_path.clone(),
                    cue_name: authored.name.clone(),
                    clip_index,
                },
            );
            if !self.clips.contains_key(&key) {
                let _ = self.cache_clip_bytes(key.clone(), clip.bytes.clone())?;
            }
            let runtime_clip = SoundCueClip {
                clip: newengine_audio_api::AudioClipRef::new(key),
                weight: clip.weight,
                gain: clip.gain,
                pitch: clip.pitch,
            };
            clips_by_name.insert(clip.name.trim().to_ascii_lowercase(), runtime_clip.clone());
            clips.push(runtime_clip);
        }

        let mut runtime_layers = Vec::with_capacity(authored.descriptor.layers.len());
        for layer in &authored.descriptor.layers {
            let mut layer_clips = Vec::with_capacity(layer.clip_names.len());
            for clip_name in &layer.clip_names {
                let key = clip_name.trim().to_ascii_lowercase();
                let clip = clips_by_name.get(&key).cloned().ok_or_else(|| {
                    format!(
                        "YSCD cue '{}' layer '{}' references unknown clip '{}'",
                        authored.name, layer.name, clip_name
                    )
                })?;
                layer_clips.push(clip);
            }
            if layer_clips.is_empty() {
                return Err(format!(
                    "YSCD cue '{}' layer '{}' resolved no clips",
                    authored.name, layer.name
                ));
            }
            runtime_layers.push(YscdRuntimeLayer {
                name: layer.name.trim().to_owned(),
                role: layer.role.trim().to_ascii_lowercase(),
                clips: layer_clips,
                gain: sanitize_gain(layer.gain),
                pitch: sanitize_speed(layer.pitch),
                attenuation: layer
                    .attenuation
                    .as_ref()
                    .map(audio_attenuation_from_yscd)
                    .transpose()?,
            });
        }

        let embedded_bytes = authored
            .clips
            .iter()
            .map(|clip| clip.bytes.len())
            .sum::<usize>();
        newengine_ulog_api::ulog::info!(
            "YSCD resolve dictionary='{}' cue='{}' embedded_clip_bytes={} clips={} layers={} source='engine.assets.raw_bytes_v1' body='NEF8/YSCD-v1'",
            reference.logical_path,
            authored.name,
            embedded_bytes,
            authored.clips.len(),
            runtime_layers.len(),
        );

        let cue = SoundCue {
            version: 1,
            clips,
            gain_range: authored.descriptor.gain_range,
            pitch_range: authored.descriptor.pitch_range,
            bus: audio_bus_from_yscd(&authored.descriptor.bus)?,
            looping: authored.descriptor.looping,
            concurrency_group: authored.descriptor.concurrency_group.clone(),
            priority: authored.descriptor.priority,
            spatial_policy: sound_cue_spatial_policy_from_yscd(
                &authored.descriptor.spatial_policy,
            )?,
            attenuation: authored
                .descriptor
                .attenuation
                .as_ref()
                .map(audio_attenuation_from_yscd)
                .transpose()?,
        }
        .sanitized()?;
        self.cue_layers.insert(canonical.clone(), runtime_layers);
        self.cue_meta.insert(
            canonical.clone(),
            YscdRuntimeMeta {
                dictionary_path: reference.logical_path.clone(),
                cue_name: authored.name.clone(),
                embedded_bytes,
            },
        );
        self.cues.insert(canonical, cue.clone());
        Ok(cue)
    }

    fn preload_cue(&mut self, request: AudioCuePreloadRequest) -> Result<AudioPreloadAck, String> {
        let parsed = newengine_assets_api::parse_asset_reference(&request.cue.logical_path)
            .map_err(|error| {
                format!(
                    "audio cue reference invalid '{}': {error}",
                    request.cue.logical_path
                )
            })?;
        let canonical = parsed.canonical.clone();
        let cue = self.load_cue(&request.cue.logical_path)?;
        let clip_count = cue.clips.len();
        let layer_count = self.cue_layers.get(&canonical).map_or(0, Vec::len);
        let mut bytes = 0usize;
        let mut all_cached = true;
        for entry in cue.clips {
            let ack = self.preload(AudioPreloadRequest { clip: entry.clip })?;
            bytes = bytes.saturating_add(ack.bytes);
            all_cached &= ack.cached;
        }

        // Device creation remains forbidden during DLL/plugin initialization, but cue
        // preload runs on the normal runtime loading path. Starting the async worker
        // here hides first-shot device latency without blocking the loading thread.
        self.start_output_init();
        self.poll_output_init();

        let mut diagnostics = self
            .cue_meta
            .get(&canonical)
            .map(|meta| {
                vec![format!(
                    "YSCD resolve dictionary='{}' cue='{}' embedded_clip_bytes={} clips={} layers={}",
                    meta.dictionary_path,
                    meta.cue_name,
                    meta.embedded_bytes,
                    clip_count,
                    layer_count,
                )]
            })
            .unwrap_or_default();
        let device_state = if self.output.is_some() {
            "ready"
        } else if self.output_error.is_some() {
            "failed"
        } else if self.output_init_started {
            "initializing"
        } else {
            "idle"
        };
        diagnostics.push(format!(
            "audio device prewarm state='{}' init_started={} output_ready={} error='{}'",
            device_state,
            self.output_init_started,
            self.output.is_some(),
            self.output_error.as_deref().unwrap_or(""),
        ));

        Ok(AudioPreloadAck {
            accepted: true,
            cached: all_cached,
            bytes,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            diagnostics,
        })
    }

    fn play_cue(&mut self, request: AudioCuePlayRequest) -> Result<AudioPlayAck, String> {
        let request = request.sanitized();
        if request.version != 1 {
            return Err(format!(
                "unsupported AudioCuePlayRequest version {}",
                request.version
            ));
        }
        let parsed = newengine_assets_api::parse_asset_reference(&request.cue.logical_path)
            .map_err(|error| {
                format!(
                    "audio cue reference invalid '{}': {error}",
                    request.cue.logical_path
                )
            })?;
        let canonical = parsed.canonical.clone();
        let cue = self.load_cue(&request.cue.logical_path)?;
        let layers = self.cue_layers.get(&canonical).cloned().unwrap_or_default();
        let seed = request.seed.unwrap_or_else(|| {
            let seed = self.cue_counter;
            self.cue_counter = self.cue_counter.wrapping_add(1).max(1);
            seed
        }) ^ stable_text_hash(&request.cue.logical_path);
        let spatial = match cue.spatial_policy {
            SoundCueSpatialPolicy::NonSpatial => None,
            SoundCueSpatialPolicy::Spatial => Some(AudioSpatialParams {
                position: request.position.ok_or_else(|| {
                    format!(
                        "SoundCue '{}' requires a spatial position",
                        request.cue.logical_path
                    )
                })?,
            }),
            SoundCueSpatialPolicy::Inherit => request
                .position
                .map(|position| AudioSpatialParams { position }),
        };

        if layers.is_empty() {
            let random_a = splitmix64(seed);
            let random_b = splitmix64(random_a);
            let random_c = splitmix64(random_b);
            let selected = select_weighted_clip(&cue, unit_f32(random_a))
                .cloned()
                .ok_or_else(|| "SoundCue weighted selection produced no clip".to_owned())?;
            let gain = sanitize_gain(
                request.gain * selected.gain * sample_range(cue.gain_range, unit_f32(random_b)),
            );
            let speed =
                sanitize_speed(selected.pitch * sample_range(cue.pitch_range, unit_f32(random_c)));
            let ack = self.play_clip_with_policy(
                AudioPlayRequest {
                    version: 1,
                    clip: selected.clip.clone(),
                    bus: cue.bus,
                    gain,
                    speed,
                    looping: cue.looping,
                    spatial,
                    attenuation: cue.attenuation.clone(),
                    acoustic: request.acoustic,
                    environment: request.environment,
                },
                cue.concurrency_group.clone(),
                cue.priority,
            )?;
            let mut ack = ack;
            if let Some(diagnostic) = self.yscd_play_diagnostic(&canonical, "body", &selected, &ack)
            {
                ack.diagnostics.push(diagnostic);
            }
            return Ok(ack);
        }

        let mut primary: Option<AudioPlayAck> = None;
        let mut accepted_layers = 0usize;
        let mut diagnostics = Vec::with_capacity(layers.len());
        for (index, layer) in layers.iter().enumerate() {
            let layer_seed = splitmix64(seed ^ stable_text_hash(&layer.name) ^ index as u64);
            let random_a = splitmix64(layer_seed);
            let random_b = splitmix64(random_a);
            let random_c = splitmix64(random_b);
            let selected = select_weighted_clips(&layer.clips, unit_f32(random_a))
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "YSCD layer '{}' weighted selection produced no clip",
                        layer.name
                    )
                })?;
            let gain = sanitize_gain(
                request.gain
                    * layer.gain
                    * selected.gain
                    * sample_range(cue.gain_range, unit_f32(random_b)),
            );
            let speed = sanitize_speed(
                layer.pitch * selected.pitch * sample_range(cue.pitch_range, unit_f32(random_c)),
            );
            let concurrency_group = if cue.concurrency_group.trim().is_empty() {
                String::new()
            } else {
                format!("{}#{}", cue.concurrency_group, layer.name)
            };
            let ack = self.play_clip_with_policy(
                AudioPlayRequest {
                    version: 1,
                    clip: selected.clip.clone(),
                    bus: cue.bus,
                    gain,
                    speed,
                    looping: cue.looping,
                    spatial,
                    attenuation: layer
                        .attenuation
                        .clone()
                        .or_else(|| cue.attenuation.clone()),
                    acoustic: request.acoustic,
                    environment: request.environment,
                },
                concurrency_group,
                cue.priority,
            )?;
            if let Some(diagnostic) =
                self.yscd_play_diagnostic(&canonical, &layer.name, &selected, &ack)
            {
                diagnostics.push(diagnostic);
            }
            if ack.accepted {
                accepted_layers = accepted_layers.saturating_add(1);
                let preferred_primary = matches!(layer.role.as_str(), "body" | "near");
                if primary.is_none() || preferred_primary {
                    primary = Some(ack.clone());
                }
            }
        }

        if let Some(mut ack) = primary {
            ack.message = format!("YSCD layered cue accepted layers={accepted_layers}");
            ack.diagnostics = diagnostics;
            return Ok(ack);
        }
        Ok(AudioPlayAck {
            accepted: false,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            voice_id: None,
            message: "YSCD layered cue produced no accepted voices".to_owned(),
            virtualized: false,
            diagnostics,
        })
    }

    fn yscd_play_diagnostic(
        &self,
        canonical: &str,
        layer: &str,
        selected: &SoundCueClip,
        ack: &AudioPlayAck,
    ) -> Option<String> {
        let meta = self.cue_meta.get(canonical)?;
        let clip_bytes = self
            .clips
            .get(&selected.clip.uri)
            .map(CachedClip::len)
            .unwrap_or(0);
        let voice = ack.voice_id.and_then(|voice_id| self.voices.get(&voice_id));
        let physical_voice = voice.is_some_and(VoiceEntry::is_physical);
        let arbiter_selected = ack
            .voice_id
            .is_some_and(|voice_id| self.desired_physical_voices().contains(&voice_id));
        let audibility = voice
            .map(|voice| self.voice_audibility(voice))
            .unwrap_or(0.0);
        let distance = voice
            .map(|voice| voice.distance_to(self.listener))
            .unwrap_or(0.0);
        let attenuation_gain = voice
            .map(|voice| voice.attenuation_gain(self.listener))
            .unwrap_or(0.0);
        let bus_gain = voice.map(|voice| self.bus_gain(voice.bus)).unwrap_or(0.0);
        let transmission_gain = voice
            .map(|voice| voice.acoustic.sanitized().transmission_gain)
            .unwrap_or(0.0);
        let output_state = if self.output.is_some() {
            "ready"
        } else if self.output_error.is_some() {
            "failed"
        } else if self.output_init_started {
            "initializing"
        } else {
            "idle"
        };
        let materialize_error = ack
            .voice_id
            .and_then(|voice_id| self.materialization_errors.get(&voice_id))
            .map(String::as_str)
            .unwrap_or("");
        Some(format!(
            "YSCD play dictionary='{}' cue='{}' layer='{}' embedded_clip_bytes={} dictionary_embedded_bytes={} physical_voice={} virtualized={} voice_id={:?} output_state='{}' arbiter_selected={} audibility={:.6} distance={:.3} attenuation_gain={:.6} bus_gain={:.3} transmission_gain={:.3} max_physical_voices={} output_error='{}' materialize_error='{}'",
            meta.dictionary_path,
            meta.cue_name,
            layer,
            clip_bytes,
            meta.embedded_bytes,
            physical_voice,
            ack.virtualized,
            ack.voice_id,
            output_state,
            arbiter_selected,
            audibility,
            distance,
            attenuation_gain,
            bus_gain,
            transmission_gain,
            self.max_physical_voices,
            self.output_error.as_deref().unwrap_or(""),
            materialize_error,
        ))
    }

    fn play_feedback(&mut self, event: AudioFeedbackEvent) -> AudioFeedbackAck {
        self.prune_finished();
        let (frequency, duration_ms) = feedback_tone(&event.id);
        let gain = sanitize_gain(DEFAULT_UI_TONE_GAIN * event.intensity.clamp(0.0, 1.0));
        let voice_id = self.alloc_voice_id();
        self.voices.insert(
            voice_id,
            VoiceEntry {
                control: None,
                source: VoiceSource::Tone {
                    frequency,
                    duration: Duration::from_millis(duration_ms),
                },
                bus: AudioBus::Ui,
                gain,
                speed: 1.0,
                looping: false,
                spatial: None,
                attenuation: None,
                acoustic: AudioAcousticState::clear(),
                environment: AudioEnvironmentState::clear(),
                stream_stats: None,
                concurrency_group: String::new(),
                priority: UI_FEEDBACK_PRIORITY,
                paused: false,
                virtual_source_position: Duration::ZERO,
                virtual_since: Some(Instant::now()),
            },
        );
        self.rebalance_physical_voices();
        AudioFeedbackAck {
            accepted: self.voices.contains_key(&voice_id),
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            queued_events: self.voices.len(),
        }
    }

    fn stop_voice(&mut self, request: AudioStopVoiceRequest) -> AudioVoiceAck {
        self.prune_finished();
        let accepted = self.remove_voice(request.voice_id).is_some();
        if accepted {
            self.rebalance_physical_voices();
        }
        AudioVoiceAck {
            accepted,
            voice_id: request.voice_id,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            message: if accepted {
                String::new()
            } else {
                "voice not found".to_owned()
            },
        }
    }

    fn update_voice(&mut self, request: AudioVoiceUpdateRequest) -> AudioVoiceAck {
        self.prune_finished();
        let now = Instant::now();
        let mut needs_rebalance = false;
        let Some(voice) = self.voices.get_mut(&request.voice_id) else {
            return AudioVoiceAck {
                accepted: false,
                voice_id: request.voice_id,
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                message: "voice not found".to_owned(),
            };
        };

        if let Some(gain) = request.gain {
            voice.gain = sanitize_gain(gain);
        }
        if let Some(speed) = request.speed {
            if matches!(voice.source, VoiceSource::Stream { .. }) {
                return AudioVoiceAck {
                    accepted: false,
                    voice_id: request.voice_id,
                    provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                    message: "streaming voices do not support runtime speed changes".to_owned(),
                };
            }
            let speed = sanitize_speed(speed);
            if (speed - voice.speed).abs() > f32::EPSILON {
                if let Some(control) = voice.control.as_ref() {
                    let source_position = control.get_pos().mul_f32(voice.speed);
                    voice.speed = speed;
                    control.set_speed(speed);
                    let output_position = source_position.div_f32(speed);
                    if let Err(error) = control.try_seek(output_position) {
                        return AudioVoiceAck {
                            accepted: false,
                            voice_id: request.voice_id,
                            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                            message: error,
                        };
                    }
                } else {
                    voice.freeze_virtual_timeline(now);
                    voice.speed = speed;
                    voice.resume_virtual_timeline(now);
                }
            }
        }
        if let Some(seek_seconds) = request.seek_seconds {
            if !seek_seconds.is_finite() || seek_seconds < 0.0 {
                return AudioVoiceAck {
                    accepted: false,
                    voice_id: request.voice_id,
                    provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                    message: "voice seek_seconds must be finite and non-negative".to_owned(),
                };
            }
            let target = Duration::from_secs_f64(seek_seconds.min(86_400.0));
            if let Some(control) = voice.control.as_ref() {
                if let Err(error) = control.try_seek(target) {
                    return AudioVoiceAck {
                        accepted: false,
                        voice_id: request.voice_id,
                        provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                        message: error,
                    };
                }
            }
            voice.virtual_source_position =
                voice.normalized_source_position(target.mul_f32(voice.speed));
            voice.virtual_since = (!voice.paused).then_some(now);
        }
        if let Some(paused) = request.paused {
            if paused != voice.paused {
                voice.freeze_virtual_timeline(now);
                voice.paused = paused;
                if let Some(control) = voice.control.as_ref() {
                    control.set_paused(paused);
                } else {
                    voice.resume_virtual_timeline(now);
                }
                needs_rebalance = true;
            }
        }
        if let Some(position) = request.position {
            let position = sanitize_position(position);
            voice.spatial = voice.spatial.map(|_| AudioSpatialParams { position });
            if let Some(control) = voice.control.as_ref() {
                if !control.set_emitter_position(position) && voice.spatial.is_some() {
                    return AudioVoiceAck {
                        accepted: false,
                        voice_id: request.voice_id,
                        provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                        message: "position update requires a spatial voice".to_owned(),
                    };
                }
            }
        }
        if let Some(acoustic) = request.acoustic {
            let acoustic = acoustic.sanitized();
            if acoustic != voice.acoustic {
                voice.acoustic = acoustic;
                if let Some(control) = voice.control.as_ref() {
                    control.set_acoustic(acoustic);
                }
                needs_rebalance = true;
            }
        }
        if let Some(environment) = request.environment {
            let environment = environment.sanitized();
            if environment != voice.environment {
                voice.environment = environment;
                if let Some(control) = voice.control.as_ref() {
                    control.set_environment(environment);
                }
            }
        }

        // Release the mutable voice borrow before applying bus/attenuation/acoustic gain.
        let _ = voice;
        if let Some(voice) = self.voices.get(&request.voice_id) {
            if let Some(control) = voice.control.as_ref() {
                control.set_volume(self.voice_audibility(voice));
            }
        }
        if needs_rebalance {
            self.rebalance_physical_voices();
        }
        AudioVoiceAck {
            accepted: true,
            voice_id: request.voice_id,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            message: String::new(),
        }
    }

    fn set_listener(&mut self, listener: AudioListenerState) -> AudioListenerState {
        self.listener = listener.sanitized();
        for voice in self.voices.values() {
            if let Some(control) = voice.control.as_ref() {
                control.update_listener(self.listener);
            }
        }
        self.refresh_voice_gains();
        // Camera -> listener synchronization is presentation-cadence, making it the
        // natural once-per-frame arbitration point for distance/audibility changes.
        self.rebalance_physical_voices();
        self.listener
    }

    fn set_bus_gain(&mut self, request: AudioBusGainRequest) -> AudioBusGainAck {
        let gain = sanitize_gain(request.gain);
        self.bus_gains.insert(request.bus, gain);
        self.refresh_voice_gains();
        self.rebalance_physical_voices();
        AudioBusGainAck {
            accepted: true,
            bus: request.bus,
            gain,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
        }
    }

    fn desired_physical_voices(&self) -> HashSet<u64> {
        let ranks = self
            .voices
            .iter()
            .filter_map(|(voice_id, voice)| {
                if voice.paused {
                    return None;
                }
                let audibility = self.voice_audibility(voice);
                if !audibility.is_finite() || audibility <= MIN_PHYSICAL_AUDIBILITY {
                    return None;
                }
                Some(VoiceRank {
                    voice_id: *voice_id,
                    priority: voice.priority,
                    audibility,
                    distance: voice.distance_to(self.listener),
                    already_physical: voice.is_physical(),
                })
            })
            .collect::<Vec<_>>();
        select_physical_voice_ids(ranks, self.max_physical_voices)
    }

    fn demote_voice(&mut self, voice_id: u64, now: Instant) {
        let Some(voice) = self.voices.get_mut(&voice_id) else {
            return;
        };
        let Some(control) = voice.control.take() else {
            return;
        };
        voice.virtual_source_position =
            voice.normalized_source_position(control.get_pos().mul_f32(voice.speed));
        control.stop();
        voice.virtual_since = (!voice.paused).then_some(now);
    }

    fn materialize_voice(&mut self, voice_id: u64, now: Instant) -> Result<(), String> {
        let Some(voice) = self.voices.get(&voice_id) else {
            return Err("voice disappeared before materialization".to_owned());
        };
        if voice.control.is_some() {
            return Ok(());
        }
        let source = voice.source.clone();
        let bus = voice.bus;
        let gain = voice.gain;
        let speed = voice.speed;
        let looping = voice.looping;
        let spatial = voice.spatial;
        let attenuation = voice.attenuation.clone();
        let acoustic = voice.acoustic.sanitized();
        let environment_state = voice.environment.sanitized();
        let paused = voice.paused;
        let source_position = voice.current_source_position(now);
        let seek_position = if speed > 0.0 {
            source_position.div_f32(speed)
        } else {
            Duration::ZERO
        };
        let volume = sanitize_gain(gain)
            * self.bus_gain(bus)
            * match (&attenuation, spatial) {
                (Some(attenuation), Some(spatial)) => attenuation
                    .gain_at_distance(distance3(spatial.position, self.listener.position)),
                _ => 1.0,
            }
            * acoustic.transmission_gain;

        self.start_output_init();
        self.poll_output_init();
        if self.output.is_none() {
            return Err(self
                .output_error
                .clone()
                .unwrap_or_else(|| "audio output device is still initializing".to_owned()));
        }

        let mut materialized_stream_stats = None;
        let control = match source {
            VoiceSource::Clip { uri, .. } => {
                let clip_bytes = self.clip_bytes(&uri)?;
                let decoder = Decoder::try_from(Cursor::new(clip_bytes))
                    .map_err(|error| format!("audio decode failed '{uri}': {error}"))?;
                let spectral = SpectralFilterControl::new(acoustic);
                let environment = EnvironmentFilterControl::new(environment_state);
                if let Some(spatial) = spatial {
                    let (left, right) = self.listener.ear_positions();
                    let player = SpatialPlayer::connect_new(
                        self.output.as_ref().expect("output checked").mixer(),
                        spatial.sanitized().position,
                        left,
                        right,
                    );
                    player.set_volume(volume);
                    player.set_speed(speed);
                    if looping {
                        player.append(DynamicEnvironmentSource::new(
                            DynamicSpectralSource::new(decoder.repeat_infinite(), spectral.clone()),
                            environment.clone(),
                        ));
                    } else {
                        player.append(DynamicEnvironmentSource::new(
                            DynamicSpectralSource::new(decoder, spectral.clone()),
                            environment.clone(),
                        ));
                    }
                    let control = VoiceControl::Spatial {
                        player,
                        spectral: Some(spectral),
                        environment: Some(environment),
                    };
                    if should_seek_materialized_voice(seek_position) {
                        control.try_seek(seek_position)?;
                    }
                    control.set_paused(paused);
                    control
                } else {
                    let player =
                        Player::connect_new(self.output.as_ref().expect("output checked").mixer());
                    player.set_volume(volume);
                    player.set_speed(speed);
                    if looping {
                        player.append(DynamicEnvironmentSource::new(
                            DynamicSpectralSource::new(decoder.repeat_infinite(), spectral.clone()),
                            environment.clone(),
                        ));
                    } else {
                        player.append(DynamicEnvironmentSource::new(
                            DynamicSpectralSource::new(decoder, spectral.clone()),
                            environment.clone(),
                        ));
                    }
                    let control = VoiceControl::Flat {
                        player,
                        spectral: Some(spectral),
                        environment: Some(environment),
                    };
                    if should_seek_materialized_voice(seek_position) {
                        control.try_seek(seek_position)?;
                    }
                    control.set_paused(paused);
                    control
                }
            }
            VoiceSource::Stream { uri, buffer } => {
                let reader = RangedAssetReader::new(
                    self.assets.clone(),
                    uri.clone(),
                    buffer.compressed_chunk_bytes,
                    buffer.compressed_cache_bytes,
                );
                let asset_io = reader.stats();
                let (stream, stats) = build_streaming_source(
                    reader,
                    Some(asset_io),
                    looping,
                    buffer,
                    seek_position,
                    &voice_id.to_string(),
                )?;
                materialized_stream_stats = Some(Arc::clone(&stats));
                let spectral = SpectralFilterControl::new(acoustic);
                let environment = EnvironmentFilterControl::new(environment_state);
                if let Some(spatial) = spatial {
                    let (left, right) = self.listener.ear_positions();
                    let player = SpatialPlayer::connect_new(
                        self.output.as_ref().expect("output checked").mixer(),
                        spatial.sanitized().position,
                        left,
                        right,
                    );
                    player.set_volume(volume);
                    player.set_speed(1.0);
                    player.append(DynamicEnvironmentSource::new(
                        DynamicSpectralSource::new(stream, spectral.clone()),
                        environment.clone(),
                    ));
                    let control = VoiceControl::Spatial {
                        player,
                        spectral: Some(spectral),
                        environment: Some(environment),
                    };
                    control.set_paused(paused);
                    control
                } else {
                    let player =
                        Player::connect_new(self.output.as_ref().expect("output checked").mixer());
                    player.set_volume(volume);
                    player.set_speed(1.0);
                    player.append(DynamicEnvironmentSource::new(
                        DynamicSpectralSource::new(stream, spectral.clone()),
                        environment.clone(),
                    ));
                    let control = VoiceControl::Flat {
                        player,
                        spectral: Some(spectral),
                        environment: Some(environment),
                    };
                    control.set_paused(paused);
                    control
                }
            }
            VoiceSource::Tone {
                frequency,
                duration,
            } => {
                let player =
                    Player::connect_new(self.output.as_ref().expect("output checked").mixer());
                player.set_volume(volume);
                player.append(SineWave::new(frequency).take_duration(duration).fade_out(
                    Duration::from_millis((duration.as_millis() as u64 / 2).max(8)),
                ));
                VoiceControl::Flat {
                    player,
                    spectral: None,
                    environment: None,
                }
            }
        };

        let Some(voice) = self.voices.get_mut(&voice_id) else {
            control.stop();
            return Err("voice disappeared during materialization".to_owned());
        };
        voice.control = Some(control);
        voice.stream_stats = materialized_stream_stats;
        voice.virtual_source_position = source_position;
        voice.virtual_since = None;
        Ok(())
    }

    fn rebalance_physical_voices(&mut self) {
        self.prune_finished();
        let now = Instant::now();
        let desired = self.desired_physical_voices();

        let demote = self
            .voices
            .iter()
            .filter_map(|(voice_id, voice)| {
                (voice.is_physical() && !desired.contains(voice_id)).then_some(*voice_id)
            })
            .collect::<Vec<_>>();
        for voice_id in demote {
            if self
                .voices
                .get(&voice_id)
                .is_some_and(VoiceEntry::virtualizable)
            {
                self.demote_voice(voice_id, now);
            } else {
                let _ = self.remove_voice(voice_id);
            }
        }

        let promote = desired
            .iter()
            .copied()
            .filter(|voice_id| {
                self.voices
                    .get(voice_id)
                    .is_some_and(VoiceEntry::is_virtual)
            })
            .collect::<Vec<_>>();
        for voice_id in promote {
            match self.materialize_voice(voice_id, now) {
                Ok(()) => {
                    self.materialization_errors.remove(&voice_id);
                }
                Err(error) => {
                    self.materialization_errors.insert(voice_id, error.clone());
                    newengine_ulog_api::ulog::warn!(
                        "audio virtualization: promote failed voice_id={} err='{}'",
                        voice_id,
                        error
                    );
                }
            }
        }

        // Non-virtualizable logical voices are valid only while physically realized.
        let invalid = self
            .voices
            .iter()
            .filter_map(|(voice_id, voice)| {
                (voice.is_virtual() && !voice.virtualizable()).then_some(*voice_id)
            })
            .collect::<Vec<_>>();
        for voice_id in invalid {
            let _ = self.remove_voice(voice_id);
        }

        debug_assert!(
            self.voices
                .values()
                .filter(|voice| voice.is_physical())
                .count()
                <= self.max_physical_voices
        );
    }

    fn diagnostics(&mut self) -> AudioDiagnostics {
        let output_ready = self.output_ready();
        self.rebalance_physical_voices();
        let physical_voices = self
            .voices
            .values()
            .filter(|voice| voice.is_physical())
            .count();
        AudioDiagnostics {
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            output_ready,
            active_voices: self.voices.len(),
            spatial_voices: self
                .voices
                .values()
                .filter(|voice| voice.spatial.is_some())
                .count(),
            physical_voices,
            virtual_voices: self.voices.len().saturating_sub(physical_voices),
            max_physical_voices: self.max_physical_voices,
            attenuated_voices: self
                .voices
                .values()
                .filter(|voice| voice.attenuation.is_some())
                .count(),
            obstructed_voices: self
                .voices
                .values()
                .filter(|voice| voice.acoustic.obstruction > 1.0e-3)
                .count(),
            occluded_voices: self
                .voices
                .values()
                .filter(|voice| voice.acoustic.occlusion > 0.5)
                .count(),
            spectrally_filtered_voices: self
                .voices
                .values()
                .filter(|voice| {
                    voice.acoustic.high_frequency_gain < 0.999
                        || voice.acoustic.low_pass_hz < 19_999.0
                })
                .count(),
            reverberant_voices: self
                .voices
                .values()
                .filter(|voice| voice.environment.is_wet())
                .count(),
            active_streams: self
                .voices
                .values()
                .filter(|voice| matches!(voice.source, VoiceSource::Stream { .. }))
                .count(),
            stream_buffered_frames: self
                .voices
                .values()
                .filter_map(|voice| voice.stream_stats.as_ref())
                .map(|stats| stats.buffered_frames())
                .sum(),
            stream_buffer_capacity_frames: self
                .voices
                .values()
                .filter_map(|voice| voice.stream_stats.as_ref())
                .map(|stats| stats.capacity_frames())
                .sum(),
            stream_underruns: self
                .voices
                .values()
                .filter_map(|voice| voice.stream_stats.as_ref())
                .map(|stats| stats.underruns())
                .sum(),
            stream_range_requests: self
                .voices
                .values()
                .filter_map(|voice| voice.stream_stats.as_ref())
                .map(|stats| stats.range_requests())
                .sum(),
            stream_compressed_bytes_fetched: self
                .voices
                .values()
                .filter_map(|voice| voice.stream_stats.as_ref())
                .map(|stats| stats.compressed_bytes_fetched())
                .sum(),
            stream_seek_operations: self
                .voices
                .values()
                .filter_map(|voice| voice.stream_stats.as_ref())
                .map(|stats| stats.seek_operations())
                .sum(),
            cached_clips: self.clips.len(),
            cached_bytes: self.cached_bytes,
            listener: self.listener,
            bus_gains: self
                .bus_gains
                .iter()
                .map(|(bus, gain)| (bus.as_str().to_owned(), *gain))
                .collect(),
        }
    }

    fn shutdown(&mut self) {
        for voice in self.voices.values() {
            if let Some(control) = voice.control.as_ref() {
                control.stop();
            }
        }
        self.voices.clear();
        self.clips.clear();
        self.cues.clear();
        self.cue_layers.clear();
        self.cue_meta.clear();
        self.cached_bytes = 0;
    }
}

fn audio_service(state: AudioRuntimeState) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    newengine_ulog_api::ulog::info!("audio service build: step='info-begin'");
    let info = AudioServiceInfo::playback_provider(NATIVE_AUDIO_PROVIDER_ROUTE);
    newengine_ulog_api::ulog::info!(
        "audio service build: step='info-done' methods={}",
        info.methods.len()
    );
    newengine_ulog_api::ulog::info!("audio service build: step='description-begin'");
    let description = engine_gateway_provider_service_description(
        NATIVE_AUDIO_SERVICE_ID,
        NATIVE_AUDIO_PROVIDER_ROUTE,
        AUDIO_BACKEND_CAPABILITY_ID,
        info.methods.iter().map(String::as_str),
    )
    .gateway(ENGINE_AUDIO_SERVICE_ID)
    .protocol(info.protocol.clone())
    .provider_abi(AUDIO_PROVIDER_ABI_ID)
    .features([
        "native-output",
        "rodio-cpal",
        "wav",
        "mp3",
        "vorbis",
        "flac",
        "2d-voices",
        "spatial-voices",
        "audio-buses",
        "clip-cache",
        "voice-budget",
        "voice-virtualization",
        "authored-attenuation",
        "physics-acoustic-state",
        "occlusion-aware-arbitration",
        "streaming-playback",
        "compressed-range-streaming",
        "seekable-streaming",
        "bounded-compressed-cache",
        "bounded-pcm-ring",
        "long-form-audio",
        "environment-zones",
        "portal-sends",
        "dynamic-reverb",
    ])
    .notes("First-party native audio provider; replaceable through engine.audio gateway routing.");
    newengine_ulog_api::ulog::info!("audio service build: step='description-done'");
    newengine_ulog_api::ulog::info!("audio service build: step='router-begin'");

    let service = JsonServiceRouter::with_state(NATIVE_AUDIO_SERVICE_ID, state)
        .describe_json(&description)
        .info(move || info.clone())
        .post_json(
            AUDIO_SERVICE_METHOD_PLAY_EVENT_JSON_V1,
            |state, event: AudioFeedbackEvent| state.play_feedback(event),
        )
        .blob(AUDIO_SERVICE_METHOD_INVOKE, |state, payload| {
            let event = match serde_json::from_slice::<AudioFeedbackEvent>(payload.as_slice()) {
                Ok(event) => event,
                Err(error) => return RResult::RErr(RString::from(error.to_string())),
            };
            ok_json(state.play_feedback(event))
        })
        .post_json_result(
            AUDIO_SERVICE_METHOD_PRELOAD_CLIP_JSON_V1,
            |state, request: AudioPreloadRequest| state.preload(request),
        )
        .post_json_result(
            AUDIO_SERVICE_METHOD_PRELOAD_CUE_JSON_V1,
            |state, request: AudioCuePreloadRequest| state.preload_cue(request),
        )
        .post_json_result(
            AUDIO_SERVICE_METHOD_PLAY_CUE_JSON_V1,
            |state, request: AudioCuePlayRequest| state.play_cue(request),
        )
        .post_json_result(
            AUDIO_SERVICE_METHOD_PLAY_CLIP_JSON_V1,
            |state, request: AudioPlayRequest| state.play_clip(request),
        )
        .post_json_result(
            AUDIO_SERVICE_METHOD_PLAY_STREAM_JSON_V1,
            |state, request: AudioStreamPlayRequest| state.play_stream(request),
        )
        .post_json(
            AUDIO_SERVICE_METHOD_STOP_VOICE_JSON_V1,
            |state, request: AudioStopVoiceRequest| state.stop_voice(request),
        )
        .post_json(
            AUDIO_SERVICE_METHOD_SET_VOICE_JSON_V1,
            |state, request: AudioVoiceUpdateRequest| state.update_voice(request),
        )
        .post_json(
            AUDIO_SERVICE_METHOD_SET_LISTENER_JSON_V1,
            |state, listener: AudioListenerState| state.set_listener(listener),
        )
        .post_json(
            AUDIO_SERVICE_METHOD_SET_BUS_GAIN_JSON_V1,
            |state, request: AudioBusGainRequest| state.set_bus_gain(request),
        )
        .get_json(AUDIO_SERVICE_METHOD_DIAGNOSTICS_JSON_V1, |state| {
            state.diagnostics()
        })
        .blob(AUDIO_SERVICE_METHOD_SHUTDOWN_V1, |state, _payload: Blob| {
            state.shutdown();
            ok_empty_blob()
        })
        .into_service_v1();
    newengine_ulog_api::ulog::info!("audio service build: step='router-done'");
    service
}

/// Registers the first-party native provider when an OS audio output is usable.
/// Failure is non-fatal: the semantic queue route remains active for headless,
/// servers, CI, and machines without a sound device.
/// Builds the native audio provider service without mutating the engine gateway registry.
///
/// First-party audio plugins own service registration through `HostApiV1::register_service_v1`.
/// Keeping construction separate from registration prevents startup-FSM re-entrancy and makes
/// the native backend replaceable like render/physics/input providers.
pub fn native_audio_provider_service(
    assets: AssetServiceClient,
) -> Result<newengine_plugin_api::ServiceV1Dyn<'static>, String> {
    newengine_ulog_api::ulog::info!("audio service factory: step='state-begin'");
    let state = AudioRuntimeState::open_default(assets)?;
    newengine_ulog_api::ulog::info!("audio service factory: step='state-done'");
    let service = audio_service(state);
    newengine_ulog_api::ulog::info!("audio service factory: step='service-done'");
    Ok(service)
}

pub fn register_native_audio_provider_best_effort(assets: AssetServiceClient) -> bool {
    newengine_ulog_api::ulog::info!("audio provider bootstrap: step='enter'");
    if audio_disabled_by_env() || headless_runtime() {
        newengine_ulog_api::ulog::info!(
            "audio provider skipped route='{}' reason='{}'",
            NATIVE_AUDIO_PROVIDER_ROUTE,
            if headless_runtime() {
                "headless"
            } else {
                "disabled-by-env"
            }
        );
        return false;
    }

    // Registration ownership is HostContext-scoped. A process-global one-shot guard would
    // survive transaction rollback and would incorrectly couple multiple Engine instances.
    if newengine_plugin_host::has_service(NATIVE_AUDIO_SERVICE_ID) {
        newengine_ulog_api::ulog::info!("audio provider bootstrap: step='already-registered'");
        return true;
    }

    newengine_ulog_api::ulog::info!("audio provider bootstrap: step='state-create-begin'");
    let state = match AudioRuntimeState::open_default(assets) {
        Ok(state) => state,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "audio provider unavailable route='{}' err='{}'; engine.audio fallback remains active",
                NATIVE_AUDIO_PROVIDER_ROUTE,
                error
            );
            return false;
        }
    };

    newengine_ulog_api::ulog::info!("audio provider bootstrap: step='state-create-done'");
    let service = audio_service(state);
    newengine_ulog_api::ulog::info!("audio provider bootstrap: step='service-build-done'");
    newengine_ulog_api::ulog::info!("audio provider bootstrap: step='gateway-register-begin'");
    match register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: ENGINE_AUDIO_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Audio,
        provider_service: NATIVE_AUDIO_SERVICE_ID,
        provider_route: NATIVE_AUDIO_PROVIDER_ROUTE,
        capability: AUDIO_BACKEND_CAPABILITY_ID,
        priority: NATIVE_AUDIO_PRIORITY,
        owner: NATIVE_AUDIO_OWNER,
        service,
    }) {
        Ok(()) => {
            newengine_ulog_api::ulog::info!(
                "audio provider registered gateway='{}' route='{}' priority={} formats='wav,mp3,ogg,flac' spatial=true device_init='async'",
                ENGINE_AUDIO_SERVICE_ID,
                NATIVE_AUDIO_PROVIDER_ROUTE,
                NATIVE_AUDIO_PRIORITY
            );
            true
        }
        Err(error) => {
            // Transactional publication leaves no partial live topology on failure; callers may
            // safely retry in the same HostContext after the owning transaction rolls back.
            newengine_ulog_api::ulog::warn!(
                "audio provider registration failed route='{}' err='{}'",
                NATIVE_AUDIO_PROVIDER_ROUTE,
                error
            );
            false
        }
    }
}

fn normalize_vfs_path(uri: &str) -> Result<String, String> {
    let reference = newengine_assets_api::parse_asset_reference(uri)
        .map_err(|error| format!("audio references must use VFS logical paths: {error}"))?;
    if reference.entry.is_some() {
        return Err(format!(
            "audio clip/cue reference '{}' must address a file, not an @entry",
            reference.canonical
        ));
    }
    Ok(reference.logical_path)
}

fn select_weighted_clip(cue: &SoundCue, unit: f32) -> Option<&SoundCueClip> {
    select_weighted_clips(&cue.clips, unit)
}

fn select_weighted_clips(clips: &[SoundCueClip], unit: f32) -> Option<&SoundCueClip> {
    let total = clips.iter().map(|clip| clip.weight.max(0.0)).sum::<f32>();
    if !(total.is_finite() && total > 0.0) {
        return None;
    }
    let mut cursor = unit.clamp(0.0, 0.999_999_94) * total;
    for clip in clips {
        cursor -= clip.weight.max(0.0);
        if cursor <= 0.0 {
            return Some(clip);
        }
    }
    clips.last()
}

fn embedded_yscd_clip_key(cue_reference: &str, clip_index: usize, codec: &str) -> String {
    let hash = stable_text_hash(cue_reference);
    let codec = codec.trim().trim_start_matches('.').to_ascii_lowercase();
    if codec.is_empty() {
        format!("__yscd/{hash:016x}/{clip_index:04}")
    } else {
        format!("__yscd/{hash:016x}/{clip_index:04}.{codec}")
    }
}

fn audio_bus_from_yscd(value: &str) -> Result<AudioBus, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "master" => Ok(AudioBus::Master),
        "music" => Ok(AudioBus::Music),
        "sfx" => Ok(AudioBus::Sfx),
        "ui" => Ok(AudioBus::Ui),
        "dialogue" => Ok(AudioBus::Dialogue),
        "ambience" => Ok(AudioBus::Ambience),
        other => Err(format!("YSCD cue has unsupported audio bus '{other}'")),
    }
}

fn sound_cue_spatial_policy_from_yscd(value: &str) -> Result<SoundCueSpatialPolicy, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "inherit" => Ok(SoundCueSpatialPolicy::Inherit),
        "non_spatial" | "nonspatial" | "2d" => Ok(SoundCueSpatialPolicy::NonSpatial),
        "spatial" | "3d" => Ok(SoundCueSpatialPolicy::Spatial),
        other => Err(format!("YSCD cue has unsupported spatial_policy '{other}'")),
    }
}

fn audio_attenuation_from_yscd(
    authored: &newengine_asset_format_nef8::YscdAttenuation,
) -> Result<AudioAttenuationSettings, String> {
    let curve = match authored.curve.trim().to_ascii_lowercase().as_str() {
        "linear" => newengine_audio_api::AudioAttenuationCurve::Linear,
        "smoothstep" => newengine_audio_api::AudioAttenuationCurve::Smoothstep,
        "inverse" => newengine_audio_api::AudioAttenuationCurve::Inverse,
        "exponential" => newengine_audio_api::AudioAttenuationCurve::Exponential,
        "custom" => newengine_audio_api::AudioAttenuationCurve::Custom,
        other => {
            return Err(format!(
                "YSCD cue has unsupported attenuation curve '{other}'"
            ))
        }
    };
    Ok(AudioAttenuationSettings {
        min_distance: authored.min_distance,
        max_distance: authored.max_distance,
        curve,
        rolloff: authored.rolloff,
        curve_points: authored.curve_points.clone(),
    }
    .sanitized())
}

#[inline]
fn sample_range(range: [f32; 2], unit: f32) -> f32 {
    range[0] + (range[1] - range[0]) * unit.clamp(0.0, 1.0)
}

#[inline]
fn unit_f32(value: u64) -> f32 {
    ((value >> 40) as u32 as f32) / ((1u32 << 24) as f32)
}

#[inline]
fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = value;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

fn stable_text_hash(text: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[inline]
fn sanitize_position(position: [f32; 3]) -> [f32; 3] {
    position.map(|component| {
        if component.is_finite() {
            component
        } else {
            0.0
        }
    })
}

#[inline]
fn distance3(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[inline]
fn should_seek_materialized_voice(position: Duration) -> bool {
    position >= Duration::from_millis(MIN_MATERIALIZE_SEEK_MS)
}

fn max_physical_voices_from_env() -> usize {
    std::env::var("NEWENGINE_AUDIO_MAX_PHYSICAL_VOICES")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map(|voices| voices.clamp(1, MAX_CONFIGURED_PHYSICAL_VOICES))
        .unwrap_or(DEFAULT_MAX_PHYSICAL_VOICES)
}

fn feedback_tone(event_id: &str) -> (f32, u64) {
    match event_id {
        "ui.open" => (660.0, 55),
        "ui.close" => (440.0, 50),
        "ui.navigate" => (520.0, 30),
        "ui.confirm" => (780.0, 70),
        "ui.back" => (390.0, 55),
        "ui.rebind" => (880.0, 85),
        "ui.error" => (220.0, 120),
        _ => (500.0, 35),
    }
}

fn cache_limit_bytes_from_env() -> usize {
    std::env::var("NEWENGINE_AUDIO_CACHE_MB")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map(|mb| mb.clamp(8, 2048).saturating_mul(1024 * 1024))
        .unwrap_or(DEFAULT_CLIP_CACHE_LIMIT_BYTES)
}

#[inline]
fn headless_runtime() -> bool {
    env_flag("NEWENGINE_HEADLESS")
}

#[inline]
fn audio_disabled_by_env() -> bool {
    env_flag("NEWENGINE_AUDIO_DISABLED")
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materialization_suppresses_near_zero_decoder_seek() {
        assert!(!should_seek_materialized_voice(Duration::ZERO));
        assert!(!should_seek_materialized_voice(Duration::from_millis(1)));
        assert!(!should_seek_materialized_voice(Duration::from_millis(49)));
        assert!(should_seek_materialized_voice(Duration::from_millis(50)));
        assert!(should_seek_materialized_voice(Duration::from_secs(1)));
    }

    #[test]
    fn feedback_tones_are_bounded() {
        for event in [
            "ui.open",
            "ui.close",
            "ui.navigate",
            "ui.confirm",
            "ui.back",
            "ui.rebind",
            "ui.error",
        ] {
            let (hz, ms) = feedback_tone(event);
            assert!((80.0..=4_000.0).contains(&hz));
            assert!((10..=500).contains(&ms));
        }
    }

    #[test]
    fn audio_paths_are_vfs_only() {
        assert_eq!(
            normalize_vfs_path("shared/audio/test.wav").unwrap(),
            "shared/audio/test.wav"
        );
        assert!(normalize_vfs_path("C:/audio/test.wav").is_err());
        assert!(normalize_vfs_path("../audio/test.wav").is_err());
        assert!(normalize_vfs_path("shared/audio/clip.wav@entry").is_err());
        assert!(normalize_vfs_path("shared/audio/weapon/rifle/rifle.yscd@fire").is_err());
    }

    #[test]
    fn yscd_metadata_maps_to_audio_runtime_semantics() {
        assert_eq!(audio_bus_from_yscd("sfx").unwrap(), AudioBus::Sfx);
        assert_eq!(
            sound_cue_spatial_policy_from_yscd("spatial").unwrap(),
            SoundCueSpatialPolicy::Spatial
        );
        let attenuation =
            audio_attenuation_from_yscd(&newengine_asset_format_nef8::YscdAttenuation {
                min_distance: 2.0,
                max_distance: 140.0,
                curve: "inverse".to_owned(),
                rolloff: 0.75,
                curve_points: Vec::new(),
            })
            .unwrap();
        assert_eq!(attenuation.min_distance, 2.0);
        assert_eq!(attenuation.max_distance, 140.0);
        assert_eq!(
            attenuation.curve,
            newengine_audio_api::AudioAttenuationCurve::Inverse
        );
        assert_eq!(attenuation.rolloff, 0.75);
    }

    #[test]
    fn yscd_embedded_clip_keys_are_stable_and_codec_suffixed() {
        let a = embedded_yscd_clip_key("shared/audio/rifle.yscd@fire", 0, "wav");
        let b = embedded_yscd_clip_key("shared/audio/rifle.yscd@fire", 0, "wav");
        assert_eq!(a, b);
        assert!(a.starts_with("__yscd/"));
        assert!(a.ends_with(".wav"));
    }

    #[test]
    fn weighted_selection_is_deterministic() {
        let cue = SoundCue {
            clips: vec![
                newengine_audio_api::SoundCueClip {
                    clip: newengine_audio_api::AudioClipRef::new("a.wav"),
                    weight: 1.0,
                    gain: 1.0,
                    pitch: 1.0,
                },
                newengine_audio_api::SoundCueClip {
                    clip: newengine_audio_api::AudioClipRef::new("b.wav"),
                    weight: 3.0,
                    gain: 1.0,
                    pitch: 1.0,
                },
            ],
            ..SoundCue::default()
        }
        .sanitized()
        .unwrap();
        assert_eq!(select_weighted_clip(&cue, 0.0).unwrap().clip.uri, "a.wav");
        assert_eq!(select_weighted_clip(&cue, 0.9).unwrap().clip.uri, "b.wav");
    }

    #[test]
    fn voice_budget_rank_prefers_priority_then_audibility_then_distance() {
        let mut ranks = vec![
            VoiceRank {
                voice_id: 1,
                priority: 10,
                audibility: 0.9,
                distance: 1.0,
                already_physical: false,
            },
            VoiceRank {
                voice_id: 2,
                priority: 20,
                audibility: 0.1,
                distance: 100.0,
                already_physical: false,
            },
            VoiceRank {
                voice_id: 3,
                priority: 10,
                audibility: 0.95,
                distance: 50.0,
                already_physical: false,
            },
        ];
        sort_voice_ranks(&mut ranks);
        assert_eq!(
            ranks.iter().map(|rank| rank.voice_id).collect::<Vec<_>>(),
            vec![2, 3, 1]
        );
    }

    #[test]
    fn voice_budget_selection_never_exceeds_hard_cap() {
        let ranks = (0..100_u64)
            .map(|voice_id| VoiceRank {
                voice_id,
                priority: (voice_id % 5) as i32,
                audibility: 1.0,
                distance: voice_id as f32,
                already_physical: false,
            })
            .collect::<Vec<_>>();
        let selected = select_physical_voice_ids(ranks, 16);
        assert_eq!(selected.len(), 16);
    }

    #[test]
    fn dynamic_spectral_filter_updates_in_place_and_attenuates_high_frequency_energy() {
        let samples = (0..512)
            .map(|index| if index % 2 == 0 { 1.0 } else { -1.0 })
            .collect::<Vec<_>>();
        let buffer = rodio::buffer::SamplesBuffer::new(
            ChannelCount::new(1).expect("mono"),
            SampleRate::new(48_000).expect("sample rate"),
            samples,
        );
        let control = SpectralFilterControl::new(AudioAcousticState::clear());
        let mut source = DynamicSpectralSource::new(buffer, control.clone());
        let clear = source.by_ref().take(128).collect::<Vec<_>>();
        let clear_rms =
            (clear.iter().map(|sample| sample * sample).sum::<f32>() / clear.len() as f32).sqrt();

        let concrete = AudioAcousticState {
            obstruction: 1.0,
            occlusion: 1.0,
            transmission_gain: 0.1,
            high_frequency_gain: 0.08,
            low_pass_hz: 1_100.0,
        };
        control.set_acoustic(concrete);
        assert!((control.low_pass_hz() - 1_100.0).abs() < 1.0e-3);
        assert!((control.high_frequency_gain() - 0.08).abs() < 1.0e-6);

        source.by_ref().take(64).for_each(drop);
        let filtered = source.by_ref().take(128).collect::<Vec<_>>();
        let filtered_rms = (filtered.iter().map(|sample| sample * sample).sum::<f32>()
            / filtered.len() as f32)
            .sqrt();
        assert!(clear_rms > 0.99);
        assert!(filtered_rms < clear_rms * 0.35);
    }

    #[test]
    fn dynamic_environment_reverb_updates_in_place_and_produces_room_tail() {
        let mut samples = vec![0.0_f32; 16_000];
        samples[0] = 1.0;
        samples[7_000] = 1.0;
        let buffer = rodio::buffer::SamplesBuffer::new(
            ChannelCount::new(1).expect("mono"),
            SampleRate::new(48_000).expect("sample rate"),
            samples,
        );
        let control = EnvironmentFilterControl::new(AudioEnvironmentState::clear());
        let mut source = DynamicEnvironmentSource::new(buffer, control.clone());

        let clear = source.by_ref().take(6_000).collect::<Vec<_>>();
        let clear_tail_energy = clear.iter().skip(1).map(|sample| sample.abs()).sum::<f32>();
        assert!(clear_tail_energy < 1.0e-6);

        control.set_environment(AudioEnvironmentState {
            source_send: AudioReverbSend::default(),
            listener_send: AudioReverbSend {
                gain: 0.7,
                preset: newengine_audio_api::AudioReverbPreset::room(),
            },
            portal_gain: 1.0,
        });
        let wet = source.by_ref().take(8_000).collect::<Vec<_>>();
        assert!(wet.iter().copied().all(f32::is_finite));
        let wet_tail_energy = wet
            .iter()
            .skip(1_550)
            .map(|sample| sample.abs())
            .sum::<f32>();
        assert!(wet_tail_energy > 0.01);
        assert!(
            wet.iter()
                .map(|sample| sample.abs())
                .fold(0.0_f32, f32::max)
                < 4.0
        );
    }

    #[test]
    fn acoustic_transmission_participates_in_voice_budget_audibility() {
        let clear = 0.8_f32 * AudioAcousticState::clear().transmission_gain;
        let occluded = 0.8_f32
            * AudioAcousticState {
                obstruction: 1.0,
                occlusion: 1.0,
                transmission_gain: 0.2,
                high_frequency_gain: 0.2,
                low_pass_hz: 1_200.0,
            }
            .sanitized()
            .transmission_gain;
        assert!(occluded < clear);
        let mut ranks = vec![
            VoiceRank {
                voice_id: 1,
                priority: 10,
                audibility: occluded,
                distance: 2.0,
                already_physical: true,
            },
            VoiceRank {
                voice_id: 2,
                priority: 10,
                audibility: clear,
                distance: 8.0,
                already_physical: false,
            },
        ];
        sort_voice_ranks(&mut ranks);
        assert_eq!(ranks[0].voice_id, 2);
    }

    #[test]
    fn virtual_timeline_advances_in_source_time_and_wraps_loops() {
        let now = Instant::now();
        let voice = VoiceEntry {
            control: None,
            source: VoiceSource::Clip {
                uri: "shared/audio/test.wav".to_owned(),
                source_duration: Some(Duration::from_secs(2)),
            },
            bus: AudioBus::Sfx,
            gain: 1.0,
            speed: 2.0,
            looping: true,
            spatial: None,
            attenuation: None,
            acoustic: AudioAcousticState::clear(),
            environment: AudioEnvironmentState::clear(),
            stream_stats: None,
            concurrency_group: String::new(),
            priority: 0,
            paused: false,
            virtual_source_position: Duration::from_millis(250),
            virtual_since: Some(now - Duration::from_millis(500)),
        };
        // 250ms source base + 500ms wall time * 2x speed = 1250ms source time.
        let position = voice.current_source_position(now);
        assert!((position.as_secs_f32() - 1.25).abs() < 0.02);

        let wrapped = VoiceEntry {
            virtual_source_position: Duration::from_millis(1750),
            virtual_since: Some(now - Duration::from_millis(500)),
            ..voice
        };
        // 1750ms + 1000ms source advance wraps over a 2s loop to ~750ms.
        let position = wrapped.current_source_position(now);
        assert!((position.as_secs_f32() - 0.75).abs() < 0.02);
    }

    #[test]
    fn attenuation_distance_reduces_physical_audibility() {
        let attenuation = AudioAttenuationSettings {
            min_distance: 0.0,
            max_distance: 100.0,
            curve: newengine_audio_api::AudioAttenuationCurve::Linear,
            ..Default::default()
        };
        assert!(attenuation.gain_at_distance(10.0) > attenuation.gain_at_distance(90.0));
        assert_eq!(attenuation.gain_at_distance(100.0), 0.0);
    }
}
