#![forbid(unsafe_op_in_unsafe_fn)]

/// Latched RMB free-fly capture with deterministic delta suppression.
///
/// Rationale:
/// - Pointer-lock / cursor grab transitions can inject synthetic deltas.
/// - Some backends can momentarily flap `button_down` during capture.
///
/// AAA policy:
/// - Capture toggles only on explicit press/release edges.
/// - Around toggles we suppress motion for a fixed number of frames.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlyRmbLatch {
    captured: bool,
    grace_frames: u8,
}

impl FlyRmbLatch {
    pub const DEFAULT_GRACE_FRAMES: u8 = 2;

    #[inline]
    pub fn is_captured(&self) -> bool {
        self.captured
    }

    /// Update capture state from input edges.
    ///
    /// Returns `(captured, changed)`.
    #[inline]
    pub fn update(&mut self, pressed: bool, released: bool, can_capture: bool) -> (bool, bool) {
        let prev = self.captured;

        if !self.captured {
            if pressed && can_capture {
                self.captured = true;
                self.grace_frames = Self::DEFAULT_GRACE_FRAMES;
            }
        } else if released {
            self.captured = false;
            self.grace_frames = Self::DEFAULT_GRACE_FRAMES;
        }

        (self.captured, self.captured != prev)
    }

    /// Force-cancel capture (e.g. Esc, focus loss).
    #[inline]
    pub fn cancel(&mut self) {
        if self.captured {
            self.captured = false;
            self.grace_frames = Self::DEFAULT_GRACE_FRAMES;
        }
    }

    /// Apply grace-frame suppression to per-frame motion.
    #[inline]
    pub fn suppress_motion_if_needed(
        &mut self,
        dx_px: &mut f32,
        dy_px: &mut f32,
        wheel_y: &mut f32,
    ) {
        if self.grace_frames == 0 {
            return;
        }
        self.grace_frames = self.grace_frames.saturating_sub(1);
        *dx_px = 0.0;
        *dy_px = 0.0;
        *wheel_y = 0.0;
    }
}
