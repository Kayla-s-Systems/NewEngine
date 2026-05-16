#[derive(Clone, Copy, Debug)]
pub struct FixedStepClock {
    fixed_dt: f32,
    max_substeps: u32,
    accumulator: f32,
    tick: u64,
}

pub struct FixedStepDrain {
    fixed_dt: f32,
    remaining: u32,
    tick: u64,
}

impl FixedStepClock {
    #[inline]
    pub fn new(fixed_dt: f32, max_substeps: u32) -> Self {
        Self { fixed_dt: fixed_dt.clamp(1.0 / 240.0, 1.0 / 15.0), max_substeps: max_substeps.clamp(1, 16), accumulator: 0.0, tick: 0 }
    }

    #[inline]
    pub fn drain(&mut self, dt: f32) -> FixedStepDrain {
        self.accumulator += dt.clamp(0.0, self.fixed_dt * self.max_substeps as f32);
        let steps = (self.accumulator / self.fixed_dt).floor() as u32;
        let steps = steps.min(self.max_substeps);
        self.accumulator -= self.fixed_dt * steps as f32;
        let start_tick = self.tick;
        self.tick = self.tick.wrapping_add(steps as u64);
        FixedStepDrain { fixed_dt: self.fixed_dt, remaining: steps, tick: start_tick }
    }

    #[inline]
    pub fn tick(&self) -> u64 { self.tick }
}

impl Iterator for FixedStepDrain {
    type Item = (u64, f32);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 { return None; }
        let tick = self.tick;
        self.tick = self.tick.wrapping_add(1);
        self.remaining -= 1;
        Some((tick, self.fixed_dt))
    }
}
