#[path = "dsp_types.rs"]
mod dsp_types;
use dsp_types::{CachedClip, EmbeddedYscdClipLocator, YscdRuntimeLayer, YscdRuntimeMeta};

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
    early_reflections: AudioEarlyReflectionField,
    early_reflections_gain: f32,
    early_reflections_high_frequency_gain: f32,
    early_reflection_direction: [f32; 3],
    pre_delay_ms: f32,
    early_reflections_spread_ms: f32,
    decay_seconds: f32,
    damping: f32,
    diffusion: f32,
}

impl ReverbSendSnapshot {
    fn from_send(send: AudioReverbSend) -> Self {
        let send = send.sanitized();
        Self {
            gain: send.gain,
            early_reflections: send.early_reflections,
            early_reflections_gain: send.preset.early_reflections_gain,
            early_reflections_high_frequency_gain: send
                .preset
                .early_reflections_high_frequency_gain,
            early_reflection_direction: send.early_reflection_direction,
            pre_delay_ms: send.preset.pre_delay_ms,
            early_reflections_spread_ms: send.preset.early_reflections_spread_ms,
            decay_seconds: send.preset.decay_seconds,
            damping: send.preset.damping,
            diffusion: send.preset.diffusion,
        }
    }
}

const EARLY_REFLECTION_TAP_WORDS: usize = 7;
const EARLY_REFLECTION_FIELD_WORDS: usize =
    1 + AUDIO_MAX_EARLY_REFLECTION_TAPS * EARLY_REFLECTION_TAP_WORDS;

/// Fixed-layout atomics keep environment updates lock-free on the audio callback. Word 0 is count;
/// each tap stores delay, gain, HF gain, XYZ direction, and order.
#[derive(Clone, Debug)]
struct EarlyReflectionFieldControl {
    words: Arc<[AtomicU32; EARLY_REFLECTION_FIELD_WORDS]>,
}

impl EarlyReflectionFieldControl {
    fn new(field: AudioEarlyReflectionField) -> Self {
        let control = Self {
            words: Arc::new(std::array::from_fn(|_| AtomicU32::new(0))),
        };
        control.set(field);
        control
    }

    fn set(&self, field: AudioEarlyReflectionField) {
        let field = field.sanitized();
        for (index, tap) in field.taps.iter().enumerate() {
            let base = 1 + index * EARLY_REFLECTION_TAP_WORDS;
            self.words[base].store(tap.delay_ms.to_bits(), Ordering::Relaxed);
            self.words[base + 1].store(tap.gain.to_bits(), Ordering::Relaxed);
            self.words[base + 2].store(tap.high_frequency_gain.to_bits(), Ordering::Relaxed);
            self.words[base + 3].store(tap.direction[0].to_bits(), Ordering::Relaxed);
            self.words[base + 4].store(tap.direction[1].to_bits(), Ordering::Relaxed);
            self.words[base + 5].store(tap.direction[2].to_bits(), Ordering::Relaxed);
            self.words[base + 6].store(u32::from(tap.order), Ordering::Relaxed);
        }
        // Publish count last so the callback cannot observe a new count with stale tap words.
        self.words[0].store(u32::from(field.count), Ordering::Release);
    }

    fn snapshot(&self) -> AudioEarlyReflectionField {
        let count = self.words[0]
            .load(Ordering::Acquire)
            .min(AUDIO_MAX_EARLY_REFLECTION_TAPS as u32) as u8;
        let mut field = AudioEarlyReflectionField::empty();
        field.count = count;
        for index in 0..usize::from(count) {
            let base = 1 + index * EARLY_REFLECTION_TAP_WORDS;
            field.taps[index] = AudioEarlyReflectionTap {
                delay_ms: f32::from_bits(self.words[base].load(Ordering::Relaxed)),
                gain: f32::from_bits(self.words[base + 1].load(Ordering::Relaxed)),
                high_frequency_gain: f32::from_bits(self.words[base + 2].load(Ordering::Relaxed)),
                direction: [
                    f32::from_bits(self.words[base + 3].load(Ordering::Relaxed)),
                    f32::from_bits(self.words[base + 4].load(Ordering::Relaxed)),
                    f32::from_bits(self.words[base + 5].load(Ordering::Relaxed)),
                ],
                order: self.words[base + 6].load(Ordering::Relaxed) as u8,
            };
        }
        field.sanitized()
    }
}

#[derive(Clone, Debug)]
struct ReverbSendControl {
    gain_bits: Arc<AtomicU32>,
    early_field: EarlyReflectionFieldControl,
    early_bits: Arc<AtomicU32>,
    early_hf_bits: Arc<AtomicU32>,
    early_direction_bits: [Arc<AtomicU32>; 3],
    pre_delay_bits: Arc<AtomicU32>,
    early_spread_bits: Arc<AtomicU32>,
    decay_bits: Arc<AtomicU32>,
    damping_bits: Arc<AtomicU32>,
    diffusion_bits: Arc<AtomicU32>,
}

impl ReverbSendControl {
    fn new(send: AudioReverbSend) -> Self {
        let snapshot = ReverbSendSnapshot::from_send(send);
        Self {
            gain_bits: Arc::new(AtomicU32::new(snapshot.gain.to_bits())),
            early_field: EarlyReflectionFieldControl::new(snapshot.early_reflections),
            early_bits: Arc::new(AtomicU32::new(snapshot.early_reflections_gain.to_bits())),
            early_hf_bits: Arc::new(AtomicU32::new(
                snapshot.early_reflections_high_frequency_gain.to_bits(),
            )),
            early_direction_bits: snapshot
                .early_reflection_direction
                .map(|value| Arc::new(AtomicU32::new(value.to_bits()))),
            pre_delay_bits: Arc::new(AtomicU32::new(snapshot.pre_delay_ms.to_bits())),
            early_spread_bits: Arc::new(AtomicU32::new(
                snapshot.early_reflections_spread_ms.to_bits(),
            )),
            decay_bits: Arc::new(AtomicU32::new(snapshot.decay_seconds.to_bits())),
            damping_bits: Arc::new(AtomicU32::new(snapshot.damping.to_bits())),
            diffusion_bits: Arc::new(AtomicU32::new(snapshot.diffusion.to_bits())),
        }
    }

    fn set(&self, send: AudioReverbSend) {
        let snapshot = ReverbSendSnapshot::from_send(send);
        self.gain_bits
            .store(snapshot.gain.to_bits(), Ordering::Relaxed);
        self.early_field.set(snapshot.early_reflections);
        self.early_bits
            .store(snapshot.early_reflections_gain.to_bits(), Ordering::Relaxed);
        self.early_hf_bits.store(
            snapshot.early_reflections_high_frequency_gain.to_bits(),
            Ordering::Relaxed,
        );
        for (bits, value) in self
            .early_direction_bits
            .iter()
            .zip(snapshot.early_reflection_direction)
        {
            bits.store(value.to_bits(), Ordering::Relaxed);
        }
        self.pre_delay_bits
            .store(snapshot.pre_delay_ms.to_bits(), Ordering::Relaxed);
        self.early_spread_bits.store(
            snapshot.early_reflections_spread_ms.to_bits(),
            Ordering::Relaxed,
        );
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
            early_reflections: self.early_field.snapshot(),
            early_reflections_gain: f32::from_bits(self.early_bits.load(Ordering::Relaxed))
                .clamp(0.0, 2.0),
            early_reflections_high_frequency_gain: f32::from_bits(
                self.early_hf_bits.load(Ordering::Relaxed),
            )
            .clamp(0.0, 1.0),
            early_reflection_direction: std::array::from_fn(|index| {
                f32::from_bits(self.early_direction_bits[index].load(Ordering::Relaxed))
            }),
            pre_delay_ms: f32::from_bits(self.pre_delay_bits.load(Ordering::Relaxed))
                .clamp(0.0, 250.0),
            early_reflections_spread_ms: f32::from_bits(
                self.early_spread_bits.load(Ordering::Relaxed),
            )
            .clamp(0.0, 250.0),
            decay_seconds: f32::from_bits(self.decay_bits.load(Ordering::Relaxed))
                .clamp(0.05, 20.0),
            damping: f32::from_bits(self.damping_bits.load(Ordering::Relaxed)).clamp(0.0, 1.0),
            diffusion: f32::from_bits(self.diffusion_bits.load(Ordering::Relaxed)).clamp(0.0, 1.0),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct DirectPathSnapshot {
    gain: f32,
    high_frequency_gain: f32,
    low_pass_hz: f32,
    extra_delay_ms: f32,
}

impl DirectPathSnapshot {
    fn from_response(response: AudioDirectPathResponse) -> Self {
        let response = response.sanitized();
        Self {
            gain: response.gain,
            high_frequency_gain: response.high_frequency_gain,
            low_pass_hz: response.low_pass_hz,
            extra_delay_ms: response.extra_delay_ms,
        }
    }
}

#[derive(Clone, Debug)]
struct DirectPathControl {
    gain_bits: Arc<AtomicU32>,
    high_frequency_gain_bits: Arc<AtomicU32>,
    low_pass_bits: Arc<AtomicU32>,
    delay_bits: Arc<AtomicU32>,
}

impl DirectPathControl {
    fn new(response: AudioDirectPathResponse) -> Self {
        let snapshot = DirectPathSnapshot::from_response(response);
        Self {
            gain_bits: Arc::new(AtomicU32::new(snapshot.gain.to_bits())),
            high_frequency_gain_bits: Arc::new(AtomicU32::new(
                snapshot.high_frequency_gain.to_bits(),
            )),
            low_pass_bits: Arc::new(AtomicU32::new(snapshot.low_pass_hz.to_bits())),
            delay_bits: Arc::new(AtomicU32::new(snapshot.extra_delay_ms.to_bits())),
        }
    }

    fn set(&self, response: AudioDirectPathResponse) {
        let snapshot = DirectPathSnapshot::from_response(response);
        self.gain_bits
            .store(snapshot.gain.to_bits(), Ordering::Relaxed);
        self.high_frequency_gain_bits
            .store(snapshot.high_frequency_gain.to_bits(), Ordering::Relaxed);
        self.low_pass_bits
            .store(snapshot.low_pass_hz.to_bits(), Ordering::Relaxed);
        self.delay_bits
            .store(snapshot.extra_delay_ms.to_bits(), Ordering::Relaxed);
    }

    fn snapshot(&self) -> DirectPathSnapshot {
        DirectPathSnapshot {
            gain: f32::from_bits(self.gain_bits.load(Ordering::Relaxed)).clamp(0.0, 1.0),
            high_frequency_gain: f32::from_bits(
                self.high_frequency_gain_bits.load(Ordering::Relaxed),
            )
            .clamp(0.0, 1.0),
            low_pass_hz: f32::from_bits(self.low_pass_bits.load(Ordering::Relaxed))
                .clamp(80.0, 20_000.0),
            extra_delay_ms: f32::from_bits(self.delay_bits.load(Ordering::Relaxed))
                .clamp(0.0, 500.0),
        }
    }
}

#[derive(Clone, Debug)]
struct EnvironmentFilterControl {
    source: ReverbSendControl,
    listener: ReverbSendControl,
    direct: DirectPathControl,
}

impl EnvironmentFilterControl {
    fn new(environment: AudioEnvironmentState) -> Self {
        let environment = environment.sanitized();
        Self {
            source: ReverbSendControl::new(environment.source_send),
            listener: ReverbSendControl::new(environment.listener_send),
            direct: DirectPathControl::new(environment.direct_path),
        }
    }

    #[inline]
    fn set_environment(&self, environment: AudioEnvironmentState) {
        let environment = environment.sanitized();
        self.source.set(environment.source_send);
        self.listener.set(environment.listener_send);
        self.direct.set(environment.direct_path);
    }
}

struct DirectPathProcessor {
    history: Vec<f32>,
    low_state: Vec<f32>,
    write_index: usize,
    channels: usize,
    sample_rate: f32,
}

impl DirectPathProcessor {
    fn new(sample_rate: SampleRate, channels: ChannelCount) -> Self {
        let channels = usize::from(channels.get()).max(1);
        let sample_rate = sample_rate.get() as f32;
        let max_delay_frames = (sample_rate * 0.5).ceil() as usize + 2;
        Self {
            history: vec![0.0; max_delay_frames * channels],
            low_state: vec![0.0; channels],
            write_index: 0,
            channels,
            sample_rate,
        }
    }

    fn reset(&mut self) {
        self.history.fill(0.0);
        self.low_state.fill(0.0);
        self.write_index = 0;
    }

    fn process(&mut self, input: f32, params: DirectPathSnapshot) -> f32 {
        if self.history.is_empty() {
            return input * params.gain;
        }
        let channel = self.write_index % self.channels;
        let delay_frames = ((params.extra_delay_ms * 0.001 * self.sample_rate).round() as usize)
            .min((self.sample_rate * 0.5) as usize);
        let delayed = if delay_frames == 0 {
            input
        } else {
            let offset = delay_frames
                .saturating_mul(self.channels)
                .min(self.history.len().saturating_sub(self.channels));
            let read_index = (self.write_index + self.history.len() - offset) % self.history.len();
            self.history[read_index]
        };
        self.history[self.write_index] = input;
        self.write_index = (self.write_index + 1) % self.history.len();

        let cutoff = params.low_pass_hz.min(self.sample_rate * 0.49).max(1.0);
        let alpha =
            (1.0 - (-std::f32::consts::TAU * cutoff / self.sample_rate).exp()).clamp(0.0, 1.0);
        let low = self.low_state[channel] + alpha * (delayed - self.low_state[channel]);
        self.low_state[channel] = low;
        let filtered = low + (delayed - low) * params.high_frequency_gain;
        filtered * params.gain
    }
}

const REVERB_FDN_LINES: usize = 4;
const REVERB_FDN_DELAY_SECONDS: [f32; REVERB_FDN_LINES] = [0.0297, 0.0371, 0.0411, 0.0437];

struct ReverbDelayLine {
    history: Vec<f32>,
    write_index: usize,
    damped_feedback: Vec<f32>,
    channels: usize,
    base_delay_frames: usize,
    channel_spread_frames: usize,
}

impl ReverbDelayLine {
    fn new(sample_rate: f32, channels: usize, base_delay_seconds: f32, line_index: usize) -> Self {
        let base_delay_frames = (sample_rate * base_delay_seconds).round().max(1.0) as usize;
        // A small per-channel offset decorrelates stereo/multichannel tails without modulation.
        let channel_spread_frames = (sample_rate * 0.00061 * (line_index as f32 + 1.0))
            .round()
            .max(1.0) as usize;
        let max_delay_frames = base_delay_frames
            + channel_spread_frames.saturating_mul(channels.saturating_sub(1))
            + 8;
        Self {
            history: vec![0.0; max_delay_frames * channels],
            write_index: 0,
            damped_feedback: vec![0.0; channels],
            channels,
            base_delay_frames,
            channel_spread_frames,
        }
    }

    fn reset(&mut self) {
        self.history.fill(0.0);
        self.damped_feedback.fill(0.0);
        self.write_index = 0;
    }

    #[inline]
    fn channel(&self) -> usize {
        self.write_index % self.channels
    }

    #[inline]
    fn delay_frames(&self) -> usize {
        self.base_delay_frames + self.channel() * self.channel_spread_frames
    }

    #[inline]
    fn read(&self) -> f32 {
        let offset = self
            .delay_frames()
            .saturating_mul(self.channels)
            .min(self.history.len().saturating_sub(self.channels));
        let index = (self.write_index + self.history.len() - offset) % self.history.len();
        self.history[index]
    }

    fn damped_read(&mut self, damping: f32) -> f32 {
        let channel = self.channel();
        let tap = self.read();
        // Higher authored damping means less high-frequency energy survives each loop.
        let alpha = (0.04 + (1.0 - damping.clamp(0.0, 1.0)) * 0.92).clamp(0.04, 0.96);
        let damped = self.damped_feedback[channel] + alpha * (tap - self.damped_feedback[channel]);
        self.damped_feedback[channel] = damped;
        damped
    }

    #[inline]
    fn write(&mut self, value: f32) {
        self.history[self.write_index] = value;
        self.write_index = (self.write_index + 1) % self.history.len();
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ReverbComponents {
    early: f32,
    early_taps: [f32; AUDIO_MAX_EARLY_REFLECTION_TAPS],
    late: f32,
}

struct ReverbTank {
    early_history: Vec<f32>,
    early_low_state: Vec<f32>,
    explicit_early_low_state: Vec<[f32; AUDIO_MAX_EARLY_REFLECTION_TAPS]>,
    early_write_index: usize,
    lines: Vec<ReverbDelayLine>,
    channels: usize,
    sample_rate: f32,
}

impl ReverbTank {
    fn new(sample_rate: SampleRate, channels: ChannelCount) -> Self {
        Self::new_with_late_field(sample_rate, channels, true)
    }

    /// Per-voice early renderer. It keeps the exact authored/discrete arrival history but owns no
    /// feedback delay network; the late field is injected into a room-keyed shared bus instead.
    fn new_early_only(sample_rate: SampleRate, channels: ChannelCount) -> Self {
        Self::new_with_late_field(sample_rate, channels, false)
    }

    fn new_with_late_field(
        sample_rate: SampleRate,
        channels: ChannelCount,
        with_late_field: bool,
    ) -> Self {
        let channels = usize::from(channels.get()).max(1);
        let sample_rate = sample_rate.get() as f32;
        // Pre-delay/explicit early paths stay per voice. FDN buffers are optional so the shared
        // room-bus path does not allocate four feedback lines per physical voice.
        let max_early_frames = (sample_rate * 0.52).ceil() as usize + 8;
        let lines = if with_late_field {
            REVERB_FDN_DELAY_SECONDS
                .iter()
                .enumerate()
                .map(|(index, delay)| ReverbDelayLine::new(sample_rate, channels, *delay, index))
                .collect()
        } else {
            Vec::new()
        };
        Self {
            early_history: vec![0.0; max_early_frames * channels],
            early_low_state: vec![0.0; channels],
            explicit_early_low_state: vec![[0.0; AUDIO_MAX_EARLY_REFLECTION_TAPS]; channels],
            early_write_index: 0,
            lines,
            channels,
            sample_rate,
        }
    }

    fn reset(&mut self) {
        self.early_history.fill(0.0);
        self.early_low_state.fill(0.0);
        self.explicit_early_low_state
            .fill([0.0; AUDIO_MAX_EARLY_REFLECTION_TAPS]);
        self.early_write_index = 0;
        for line in &mut self.lines {
            line.reset();
        }
    }

    #[inline]
    fn read_early_delay(&self, delay_frames: usize) -> f32 {
        if self.early_history.is_empty() {
            return 0.0;
        }
        let offset = delay_frames
            .saturating_mul(self.channels)
            .min(self.early_history.len().saturating_sub(self.channels));
        let index =
            (self.early_write_index + self.early_history.len() - offset) % self.early_history.len();
        self.early_history[index]
    }

    fn process_components(&mut self, input: f32, params: ReverbSendSnapshot) -> ReverbComponents {
        if self.early_history.is_empty() {
            return ReverbComponents::default();
        }
        let channel = self.early_write_index % self.channels;
        let pre_delay_frames = ((params.pre_delay_ms * 0.001 * self.sample_rate).round() as usize)
            .min((self.sample_rate * 0.25) as usize);
        let predelayed = if pre_delay_frames == 0 {
            input
        } else {
            self.read_early_delay(pre_delay_frames)
        };
        let cutoff = 4_000.0_f32.min(self.sample_rate * 0.49);
        let early_alpha = 1.0 - (-std::f32::consts::TAU * cutoff / self.sample_rate).exp();
        let mut early_taps = [0.0_f32; AUDIO_MAX_EARLY_REFLECTION_TAPS];
        let mut early = 0.0_f32;
        if params.early_reflections.is_empty() {
            let spread_frames =
                (params.early_reflections_spread_ms * 0.001 * self.sample_rate).round() as usize;
            let channel_offset = (self.sample_rate * channel as f32 * 0.00031).round() as usize;
            let fractions = [0.0_f32, 0.31, 0.67, 1.0];
            let weights = [1.0_f32, 0.72, 0.53, 0.39];
            let mut weight_sum = 0.0_f32;
            for (fraction, weight) in fractions.into_iter().zip(weights) {
                let spread = (spread_frames as f32 * fraction).round() as usize;
                let delay = pre_delay_frames
                    .saturating_add(spread)
                    .saturating_add(channel_offset)
                    .max(1);
                early += self.read_early_delay(delay) * weight;
                weight_sum += weight;
            }
            if weight_sum > 0.0 {
                early /= weight_sum;
            }
            let early_low = self.early_low_state[channel]
                + early_alpha * (early - self.early_low_state[channel]);
            self.early_low_state[channel] = early_low;
            early = (early_low
                + (early - early_low)
                    * params.early_reflections_high_frequency_gain.clamp(0.0, 1.0))
                * params.early_reflections_gain;
        } else {
            for (index, tap) in params.early_reflections.active().iter().enumerate() {
                let delay_frames = ((tap.delay_ms * 0.001 * self.sample_rate).round() as usize)
                    .min((self.sample_rate * 0.5) as usize);
                let delayed = if delay_frames == 0 {
                    input
                } else {
                    self.read_early_delay(delay_frames)
                };
                let previous = self.explicit_early_low_state[channel][index];
                let low = previous + early_alpha * (delayed - previous);
                self.explicit_early_low_state[channel][index] = low;
                let filtered = low + (delayed - low) * tap.high_frequency_gain.clamp(0.0, 1.0);
                early_taps[index] = filtered * tap.gain;
                early += early_taps[index];
            }
        }
        self.early_history[self.early_write_index] = input;
        self.early_write_index = (self.early_write_index + 1) % self.early_history.len();

        let late = if self.lines.len() == REVERB_FDN_LINES {
            let damping = params.damping.clamp(0.0, 1.0);
            let diffusion = params.diffusion.clamp(0.0, 1.0);
            let mut delayed = [0.0_f32; REVERB_FDN_LINES];
            for (index, line) in self.lines.iter_mut().enumerate() {
                delayed[index] = line.damped_read(damping);
            }

            // Orthonormal 4x4 Hadamard feedback matrix. Blending from the direct line state
            // toward the matrix makes authored diffusion control echo density instead of merely
            // changing tap gains, while keeping the feedback network energy-bounded.
            let hadamard = [
                (delayed[0] + delayed[1] + delayed[2] + delayed[3]) * 0.5,
                (delayed[0] - delayed[1] + delayed[2] - delayed[3]) * 0.5,
                (delayed[0] + delayed[1] - delayed[2] - delayed[3]) * 0.5,
                (delayed[0] - delayed[1] - delayed[2] + delayed[3]) * 0.5,
            ];
            let injection = [1.0_f32, 0.79, -0.67, 0.53];
            for index in 0..REVERB_FDN_LINES {
                let line = &mut self.lines[index];
                let loop_signal = delayed[index] + (hadamard[index] - delayed[index]) * diffusion;
                let delay_seconds = line.delay_frames() as f32 / self.sample_rate;
                let feedback = 0.001_f32
                    .powf(delay_seconds / params.decay_seconds.max(0.05))
                    .clamp(0.0, 0.985);
                line.write(predelayed * injection[index] * 0.36 + loop_signal * feedback);
            }

            // Signed output matrix decorrelates neighboring channels while preserving bounded
            // late energy. Mono uses the positive row; stereo alternates signs across lines.
            let signs = if channel & 1 == 0 {
                [1.0_f32, 1.0, 1.0, 1.0]
            } else {
                [1.0_f32, -1.0, 1.0, -1.0]
            };
            delayed
                .iter()
                .zip(signs)
                .map(|(sample, sign)| sample * sign)
                .sum::<f32>()
                * 0.25
                * (0.50 + diffusion * 0.30)
        } else {
            0.0
        };
        ReverbComponents {
            early,
            early_taps,
            late,
        }
    }

    #[inline]
    fn process(&mut self, input: f32, params: ReverbSendSnapshot) -> f32 {
        let components = self.process_components(input, params);
        components.early + components.late
    }
}

#[derive(Clone, Copy, Debug)]
struct SpatialMixSnapshot {
    emitter_position: [f32; 3],
    left_ear: [f32; 3],
    right_ear: [f32; 3],
}

#[derive(Clone, Debug)]
struct SpatialMixControl {
    emitter_bits: [Arc<AtomicU32>; 3],
    left_ear_bits: [Arc<AtomicU32>; 3],
    right_ear_bits: [Arc<AtomicU32>; 3],
}

impl SpatialMixControl {
    fn new(emitter_position: [f32; 3], left_ear: [f32; 3], right_ear: [f32; 3]) -> Self {
        Self {
            emitter_bits: emitter_position.map(|value| Arc::new(AtomicU32::new(value.to_bits()))),
            left_ear_bits: left_ear.map(|value| Arc::new(AtomicU32::new(value.to_bits()))),
            right_ear_bits: right_ear.map(|value| Arc::new(AtomicU32::new(value.to_bits()))),
        }
    }

    fn set_emitter_position(&self, value: [f32; 3]) {
        store_atomic_vec3(&self.emitter_bits, value);
    }

    fn set_ears(&self, left: [f32; 3], right: [f32; 3]) {
        store_atomic_vec3(&self.left_ear_bits, left);
        store_atomic_vec3(&self.right_ear_bits, right);
    }

    fn snapshot(&self) -> SpatialMixSnapshot {
        SpatialMixSnapshot {
            emitter_position: load_atomic_vec3(&self.emitter_bits),
            left_ear: load_atomic_vec3(&self.left_ear_bits),
            right_ear: load_atomic_vec3(&self.right_ear_bits),
        }
    }
}

#[inline]
fn store_atomic_vec3(bits: &[Arc<AtomicU32>; 3], value: [f32; 3]) {
    for (slot, value) in bits.iter().zip(value) {
        slot.store(value.to_bits(), Ordering::Relaxed);
    }
}

#[inline]
fn load_atomic_vec3(bits: &[Arc<AtomicU32>; 3]) -> [f32; 3] {
    std::array::from_fn(|index| f32::from_bits(bits[index].load(Ordering::Relaxed)))
}

/// Direction-only speaker pan for the direct field. Distance energy is deliberately absent here:
/// authored `AudioAttenuationSettings` is evaluated once in `voice_output_gain`/materialization.
/// Applying another inverse-distance law in the spatializer caused spatial voices to be attenuated
/// twice and could make otherwise healthy physical voices effectively inaudible.
fn direct_stereo_gains(spatial: SpatialMixSnapshot) -> [f32; 2] {
    let listener_center = [
        (spatial.left_ear[0] + spatial.right_ear[0]) * 0.5,
        (spatial.left_ear[1] + spatial.right_ear[1]) * 0.5,
        (spatial.left_ear[2] + spatial.right_ear[2]) * 0.5,
    ];
    let listener_to_emitter = [
        spatial.emitter_position[0] - listener_center[0],
        spatial.emitter_position[1] - listener_center[1],
        spatial.emitter_position[2] - listener_center[2],
    ];
    reflection_stereo_gains(listener_to_emitter, spatial)
}

/// Equal-power speaker pan from a world-space arrival vector. Ear separation defines listener
/// right; zero/unknown direction remains centered. This is intentional speaker spatialization,
/// not an HRTF/binaural claim.
fn reflection_stereo_gains(direction: [f32; 3], spatial: SpatialMixSnapshot) -> [f32; 2] {
    let right = [
        spatial.right_ear[0] - spatial.left_ear[0],
        spatial.right_ear[1] - spatial.left_ear[1],
        spatial.right_ear[2] - spatial.left_ear[2],
    ];
    let right_len = (right[0] * right[0] + right[1] * right[1] + right[2] * right[2]).sqrt();
    let dir_len =
        (direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2])
            .sqrt();
    if right_len <= 1.0e-5 || dir_len <= 1.0e-5 || !right_len.is_finite() || !dir_len.is_finite() {
        return [1.0, 1.0];
    }
    let pan = ((direction[0] * right[0] + direction[1] * right[1] + direction[2] * right[2])
        / (dir_len * right_len))
        .clamp(-1.0, 1.0);
    [(1.0 - pan).sqrt(), (1.0 + pan).sqrt()]
}

fn spatialize_early_components(
    left: ReverbComponents,
    right: ReverbComponents,
    params: ReverbSendSnapshot,
    spatial: SpatialMixSnapshot,
) -> [f32; 2] {
    if params.early_reflections.is_empty() {
        let gains = reflection_stereo_gains(params.early_reflection_direction, spatial);
        return [left.early * gains[0], right.early * gains[1]];
    }
    let mut output = [0.0_f32; 2];
    for (index, tap) in params.early_reflections.active().iter().enumerate() {
        let gains = reflection_stereo_gains(tap.direction, spatial);
        output[0] += left.early_taps[index] * gains[0];
        output[1] += right.early_taps[index] * gains[1];
    }
    output
}
/// Spatial voice renderer that keeps one decode/timeline while giving direct, early and late
/// acoustic fields independent spatial laws. Input must be mono; output is interleaved stereo.
struct DynamicSpatialEnvironmentSource<I> {
    input: I,
    environment_control: EnvironmentFilterControl,
    spatial_control: SpatialMixControl,
    source_tank: ReverbTank,
    listener_tank: ReverbTank,
    direct_path: DirectPathProcessor,
    source_params: ReverbSendSnapshot,
    listener_params: ReverbSendSnapshot,
    direct_params: DirectPathSnapshot,
    spatial: SpatialMixSnapshot,
    late_binding: Option<RoomBusVoiceBinding>,
    pending_right: Option<f32>,
    control_countdown: u8,
}

impl<I> DynamicSpatialEnvironmentSource<I>
where
    I: Source<Item = f32>,
{
    #[cfg(test)]
    fn new(
        input: I,
        environment_control: EnvironmentFilterControl,
        spatial_control: SpatialMixControl,
    ) -> Self {
        Self::new_with_late_binding(input, environment_control, spatial_control, None)
    }

    fn new_with_late_binding(
        input: I,
        environment_control: EnvironmentFilterControl,
        spatial_control: SpatialMixControl,
        late_binding: Option<RoomBusVoiceBinding>,
    ) -> Self {
        debug_assert_eq!(input.channels().get(), 1);
        let sample_rate = input.sample_rate();
        let stereo = ChannelCount::new(2).expect("stereo channel count");
        let mono = ChannelCount::new(1).expect("mono channel count");
        let source_params = environment_control.source.snapshot();
        let listener_params = environment_control.listener.snapshot();
        let direct_params = environment_control.direct.snapshot();
        let spatial = spatial_control.snapshot();
        let per_voice_late = late_binding.is_none()
            && (source_params.gain > 1.0e-4 || listener_params.gain > 1.0e-4);
        Self {
            input,
            environment_control,
            spatial_control,
            source_tank: if per_voice_late {
                ReverbTank::new(sample_rate, stereo)
            } else {
                ReverbTank::new_early_only(sample_rate, stereo)
            },
            listener_tank: if per_voice_late {
                ReverbTank::new(sample_rate, stereo)
            } else {
                ReverbTank::new_early_only(sample_rate, stereo)
            },
            direct_path: DirectPathProcessor::new(sample_rate, mono),
            source_params,
            listener_params,
            direct_params,
            spatial,
            late_binding,
            pending_right: None,
            control_countdown: 0,
        }
    }

    fn refresh_controls(&mut self) {
        if self.control_countdown == 0 {
            self.source_params = self.environment_control.source.snapshot();
            self.listener_params = self.environment_control.listener.snapshot();
            self.direct_params = self.environment_control.direct.snapshot();
            self.spatial = self.spatial_control.snapshot();
            self.control_countdown = 63;
        } else {
            self.control_countdown -= 1;
        }
    }

    fn reset_state(&mut self) {
        self.source_tank.reset();
        self.listener_tank.reset();
        self.direct_path.reset();
        self.pending_right = None;
        self.control_countdown = 0;
    }
}

impl<I> Iterator for DynamicSpatialEnvironmentSource<I>
where
    I: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(right) = self.pending_right.take() {
            return Some(right);
        }
        let dry = self.input.next()?;
        self.refresh_controls();
        if let Some(binding) = self.late_binding.as_ref() {
            binding.inject(dry, self.source_params.gain, self.listener_params.gain, 1);
        }
        let direct = self.direct_path.process(dry, self.direct_params);
        // Tanks are stereo internally: feed the same mono frame twice so channel-dependent early
        // offsets and signed FDN rows create a decorrelated diffuse field without a second decode.
        let source_left = self.source_tank.process_components(dry, self.source_params);
        let source_right = self.source_tank.process_components(dry, self.source_params);
        let listener_left = self
            .listener_tank
            .process_components(dry, self.listener_params);
        let listener_right = self
            .listener_tank
            .process_components(dry, self.listener_params);

        let direct_gain = direct_stereo_gains(self.spatial);
        let source_early = spatialize_early_components(
            source_left,
            source_right,
            self.source_params,
            self.spatial,
        );
        let listener_early = spatialize_early_components(
            listener_left,
            listener_right,
            self.listener_params,
            self.spatial,
        );
        let left = direct * direct_gain[0]
            + source_early[0] * self.source_params.gain
            + listener_early[0] * self.listener_params.gain
            + source_left.late * self.source_params.gain
            + listener_left.late * self.listener_params.gain;
        let right = direct * direct_gain[1]
            + source_early[1] * self.source_params.gain
            + listener_early[1] * self.listener_params.gain
            + source_right.late * self.source_params.gain
            + listener_right.late * self.listener_params.gain;
        self.pending_right = Some(right);
        Some(left)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl<I> Source for DynamicSpatialEnvironmentSource<I>
where
    I: Source<Item = f32>,
{
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).expect("stereo channel count")
    }

    fn sample_rate(&self) -> SampleRate {
        self.input.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.input.try_seek(pos)?;
        self.reset_state();
        Ok(())
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
    direct_path: DirectPathProcessor,
    source_params: ReverbSendSnapshot,
    listener_params: ReverbSendSnapshot,
    direct_params: DirectPathSnapshot,
    late_binding: Option<RoomBusVoiceBinding>,
    control_countdown: u8,
}

impl<I> DynamicEnvironmentSource<I>
where
    I: Source<Item = f32>,
{
    #[cfg(test)]
    fn new(input: I, control: EnvironmentFilterControl) -> Self {
        Self::new_with_late_binding(input, control, None)
    }

    fn new_with_late_binding(
        input: I,
        control: EnvironmentFilterControl,
        late_binding: Option<RoomBusVoiceBinding>,
    ) -> Self {
        let sample_rate = input.sample_rate();
        let channels = input.channels();
        let source_params = control.source.snapshot();
        let listener_params = control.listener.snapshot();
        let direct_params = control.direct.snapshot();
        let per_voice_late = late_binding.is_none()
            && (source_params.gain > 1.0e-4 || listener_params.gain > 1.0e-4);
        Self {
            input,
            control,
            source_tank: if per_voice_late {
                ReverbTank::new(sample_rate, channels)
            } else {
                ReverbTank::new_early_only(sample_rate, channels)
            },
            listener_tank: if per_voice_late {
                ReverbTank::new(sample_rate, channels)
            } else {
                ReverbTank::new_early_only(sample_rate, channels)
            },
            direct_path: DirectPathProcessor::new(sample_rate, channels),
            source_params,
            listener_params,
            direct_params,
            late_binding,
            control_countdown: 0,
        }
    }

    fn refresh_controls(&mut self) {
        if self.control_countdown == 0 {
            self.source_params = self.control.source.snapshot();
            self.listener_params = self.control.listener.snapshot();
            self.direct_params = self.control.direct.snapshot();
            self.control_countdown = 63;
        } else {
            self.control_countdown -= 1;
        }
    }

    fn reset_environment_state(&mut self) {
        self.source_tank.reset();
        self.listener_tank.reset();
        self.direct_path.reset();
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
        if let Some(binding) = self.late_binding.as_ref() {
            binding.inject(
                dry,
                self.source_params.gain,
                self.listener_params.gain,
                usize::from(self.input.channels().get()),
            );
        }
        let direct = self.direct_path.process(dry, self.direct_params);
        let source_wet =
            self.source_tank.process(dry, self.source_params) * self.source_params.gain;
        let listener_wet =
            self.listener_tank.process(dry, self.listener_params) * self.listener_params.gain;
        // The direct alternate path owns portal/diffraction delay and spectral loss. Reverb
        // sends remain independent indirect energy and preserve their own room history.
        Some(direct + source_wet + listener_wet)
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

#[cfg(test)]
#[path = "dsp_tests.rs"]
mod dsp_tests;
