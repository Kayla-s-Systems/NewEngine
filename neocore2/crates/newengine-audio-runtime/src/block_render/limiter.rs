#![forbid(unsafe_op_in_unsafe_fn)]

use rodio::{Sample, SampleRate};

const MASTER_OUTPUT_PEAK: f32 = 1.0;
const MASTER_LIMITER_RELEASE_SECONDS: f32 = 0.080;

#[derive(Clone, Copy, Debug)]
pub(super) struct OutputPeakLimiter {
    gain: f32,
    release_alpha: f32,
}

impl OutputPeakLimiter {
    pub(super) fn new(sample_rate: SampleRate) -> Self {
        let frames = (sample_rate.get() as f32 * MASTER_LIMITER_RELEASE_SECONDS).max(1.0);
        Self {
            gain: 1.0,
            release_alpha: 1.0 - (-1.0 / frames).exp(),
        }
    }

    #[inline]
    pub(super) fn process_frame(&mut self, frame: &mut [Sample]) {
        let peak = frame
            .iter()
            .copied()
            .filter(|sample| sample.is_finite())
            .map(f32::abs)
            .fold(0.0_f32, f32::max);
        let target = if peak > MASTER_OUTPUT_PEAK {
            (MASTER_OUTPUT_PEAK / peak).clamp(0.0, 1.0)
        } else {
            1.0
        };
        if target < self.gain {
            // Instant attack prevents the overloaded frame from ever reaching the device.
            self.gain = target;
        } else {
            self.gain += (1.0 - self.gain) * self.release_alpha;
        }
        for sample in frame {
            let finite = if sample.is_finite() { *sample } else { 0.0 };
            *sample = (finite * self.gain).clamp(-MASTER_OUTPUT_PEAK, MASTER_OUTPUT_PEAK);
        }
    }
}

