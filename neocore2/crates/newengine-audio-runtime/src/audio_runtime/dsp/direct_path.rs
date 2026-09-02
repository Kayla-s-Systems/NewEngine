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
