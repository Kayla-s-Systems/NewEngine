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
