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
