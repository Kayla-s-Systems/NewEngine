#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Vec2;

use crate::stack::{CameraModifier, CameraStackInput, ModifierOutput};
use crate::{CameraRig, Projection};

/// Deterministic TAA jitter using a Halton (2,3) sequence.
///
/// Jitter is returned in pixels and applied in clip space by the stack.
pub struct TaaJitter {
    pub enabled: bool,
    /// Jitter scale in pixels (typically (0.5, 0.5) or (1.0, 1.0)).
    pub scale_px: Vec2,
    pub index: u32,
}

impl Default for TaaJitter {
    fn default() -> Self {
        Self {
            enabled: true,
            scale_px: Vec2::splat(0.5),
            index: 0,
        }
    }
}

impl TaaJitter {
    #[inline]
    pub fn reset(&mut self) {
        self.index = 0;
    }
}

impl CameraModifier for TaaJitter {
    fn apply(
        &mut self,
        _rig: &CameraRig,
        _proj: &Projection,
        _input: &CameraStackInput,
    ) -> ModifierOutput {
        if !self.enabled {
            return ModifierOutput::default();
        }

        let (hx, hy) = halton_2_3(self.index);
        self.index = self.index.wrapping_add(1);

        let base = Vec2::new(hx, hy) - Vec2::splat(0.5);
        let jitter = Vec2::new(base.x * self.scale_px.x, base.y * self.scale_px.y);

        let mut out = ModifierOutput::default();
        out.proj.jitter_px = jitter;
        out
    }
}

#[inline]
fn halton_2_3(i: u32) -> (f32, f32) {
    #[inline]
    fn halton(mut index: u32, base: u32) -> f32 {
        let mut f = 1.0f32;
        let mut r = 0.0f32;
        let b = base as f32;
        while index > 0 {
            f /= b;
            r += f * (index % base) as f32;
            index /= base;
        }
        r
    }

    // Use i+1 to avoid the zero sample.
    (halton(i + 1, 2), halton(i + 1, 3))
}
