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

const MAX_REVERB_SEND_GAIN: f32 = 1.0;
const MAX_REVERB_EARLY_GAIN: f32 = 1.0;
const MAX_EARLY_REFLECTION_TAP_GAIN: f32 = 1.0;

#[inline]
fn bounded_reverb_send_gain(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, MAX_REVERB_SEND_GAIN)
    } else {
        0.0
    }
}

#[inline]
fn normalized_reverb_send_gains(source: f32, listener: f32) -> [f32; 2] {
    let source = bounded_reverb_send_gain(source);
    let listener = bounded_reverb_send_gain(listener);
    let total = source + listener;
    if total > MAX_REVERB_SEND_GAIN {
        let scale = MAX_REVERB_SEND_GAIN / total;
        [source * scale, listener * scale]
    } else {
        [source, listener]
    }
}

#[inline]
fn bounded_early_reflection_tap_gain(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, MAX_EARLY_REFLECTION_TAP_GAIN)
    } else {
        0.0
    }
}

impl ReverbSendSnapshot {
    fn from_send(send: AudioReverbSend) -> Self {
        let send = send.sanitized();
        Self {
            gain: bounded_reverb_send_gain(send.gain),
            early_reflections: send.early_reflections,
            early_reflections_gain: send
                .preset
                .early_reflections_gain
                .clamp(0.0, MAX_REVERB_EARLY_GAIN),
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
            gain: bounded_reverb_send_gain(f32::from_bits(
                self.gain_bits.load(Ordering::Relaxed),
            )),
            early_reflections: self.early_field.snapshot(),
            early_reflections_gain: f32::from_bits(self.early_bits.load(Ordering::Relaxed))
                .clamp(0.0, MAX_REVERB_EARLY_GAIN),
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
