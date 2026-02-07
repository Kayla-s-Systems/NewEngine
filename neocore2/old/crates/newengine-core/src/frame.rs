/// Frame timing snapshot.
///
/// The engine emits two kinds of frames:
///
/// - **Variable frame**: used for `update()` and `render()`.
///   `dt` is the real (clamped) wall-clock delta.
///
/// - **Fixed subframe**: emitted for each `fixed_update()` step.
///   `dt == fixed_dt`, `fixed_alpha == 0.0`, and `fixed_step_index` indicates
///   the substep within the current variable frame.
///
/// `fixed_alpha` is an interpolation factor in `[0..1)` for render smoothing.
/// It represents the remainder of the accumulator after processing fixed steps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// Monotonic variable-frame index.
    pub frame_index: u64,

    /// Delta time for this frame. For fixed subframes this equals `fixed_dt`.
    pub dt: f32,

    /// Fixed timestep size.
    pub fixed_dt: f32,

    /// Interpolation factor in `[0..1)` for render smoothing.
    /// Always `0.0` for fixed subframes.
    pub fixed_alpha: f32,

    /// Number of fixed substeps executed during this variable frame.
    pub fixed_step_count: u32,

    /// Index of the current fixed substep, in `[0..fixed_step_count)`.
    /// For variable frames this is always `0`.
    pub fixed_step_index: u32,

    /// Monotonic fixed-tick index.
    ///
    /// Increases by 1 for each `fixed_update()` call and never resets.
    /// For variable frames this is the value *after* processing all fixed steps
    /// for the frame.
    pub fixed_tick: u64,
}

impl Frame {
    /// Returns true if this frame represents a fixed substep.
    #[inline]
    pub fn is_fixed(&self) -> bool {
        self.dt == self.fixed_dt && self.fixed_alpha == 0.0 && self.fixed_step_count != 0
    }
}
