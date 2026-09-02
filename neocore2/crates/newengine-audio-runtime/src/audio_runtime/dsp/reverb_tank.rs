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
            // Discrete reflection taps are separate authored arrivals. Bound each arrival at
            // unity but do not normalize the entire time-separated field as if all taps were
            // coherent at one instant; final coincident peaks are handled by the master limiter.
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
                early_taps[index] = filtered * bounded_early_reflection_tap_gain(tap.gain);
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

