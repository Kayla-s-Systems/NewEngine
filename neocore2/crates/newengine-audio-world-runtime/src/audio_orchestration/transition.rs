#[derive(Clone, Copy, Debug)]
pub(super) struct SampleTransition {
    pub(super) from: f32,
    pub(super) target: f32,
    pub(super) start_sample: u64,
    pub(super) end_sample: u64,
}

impl SampleTransition {
    #[inline]
    pub(super) fn new(from: f32, target: f32, start_sample: u64, duration_samples: u64) -> Self {
        Self {
            from,
            target,
            start_sample,
            end_sample: start_sample.saturating_add(duration_samples),
        }
    }

    /// Returns `(value, finished)` at the requested sample using deterministic f64 interpolation
    /// before narrowing back to f32. This is shared by snapshots, RTPCs and instance gain ramps so
    /// all sample-domain automation obeys the same boundary semantics.
    #[inline]
    pub(super) fn evaluate(self, sample: u64) -> (f32, bool) {
        if sample <= self.start_sample {
            return (self.from, false);
        }
        if sample >= self.end_sample {
            return (self.target, true);
        }
        let elapsed = sample - self.start_sample;
        let duration = self.end_sample - self.start_sample;
        let t = elapsed as f64 / duration.max(1) as f64;
        (
            (f64::from(self.from) + (f64::from(self.target) - f64::from(self.from)) * t) as f32,
            false,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_boundaries_are_exact() {
        let transition = SampleTransition::new(0.0, 1.0, 100, 200);
        assert_eq!(transition.evaluate(100), (0.0, false));
        assert_eq!(transition.evaluate(200), (0.5, false));
        assert_eq!(transition.evaluate(300), (1.0, true));
    }
}
