#![forbid(unsafe_op_in_unsafe_fn)]

/// Simulation frame data.
#[derive(Clone, Copy, Debug)]
pub struct SimFrame {
    pub dt: f32,
    pub fixed_tick: u64,
}

impl SimFrame {
    #[inline]
    pub fn new(dt: f32, fixed_tick: u64) -> Self {
        Self { dt, fixed_tick }
    }
}
