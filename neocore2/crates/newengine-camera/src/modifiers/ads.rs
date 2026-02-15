#![forbid(unsafe_op_in_unsafe_fn)]

use crate::stack::{CameraModifier, CameraStackInput, ModifierOutput};
use crate::{CameraRig, Projection};

/// Aim Down Sights modifier.
///
/// This modifier blends the vertical FOV towards `fovy_aim` when `input.is_aiming` is true.
///
/// The modifier outputs an additive delta relative to the current perspective FOV.
pub struct AdsFov {
    /// Hip-fire vertical FOV (radians).
    pub fovy_hip: f32,
    /// Aim vertical FOV (radians).
    pub fovy_aim: f32,
    /// Blend speed in 1/sec (exponential smoothing).
    pub speed: f32,

    alpha: f32,
}

impl AdsFov {
    #[inline]
    pub fn new(fovy_hip: f32, fovy_aim: f32, speed: f32) -> Self {
        Self {
            fovy_hip: fovy_hip.clamp(0.01, 3.12),
            fovy_aim: fovy_aim.clamp(0.01, 3.12),
            speed: speed.max(0.001),
            alpha: 0.0,
        }
    }
}

impl Default for AdsFov {
    fn default() -> Self {
        Self::new(60.0_f32.to_radians(), 45.0_f32.to_radians(), 16.0)
    }
}

impl CameraModifier for AdsFov {
    fn apply(&mut self, _rig: &CameraRig, proj: &Projection, input: &CameraStackInput) -> ModifierOutput {
        let dt = input.dt.max(0.0);
        let target = if input.is_aiming { 1.0 } else { 0.0 };

        // Exponential smoothing with a deterministic time constant.
        let k = 1.0 - (-self.speed * dt).exp();
        self.alpha = (self.alpha + (target - self.alpha) * k).clamp(0.0, 1.0);

        let desired = self.fovy_hip + (self.fovy_aim - self.fovy_hip) * self.alpha;

        let current = match proj {
            Projection::Perspective(p) => p.fovy,
            _ => return ModifierOutput::default(),
        };

        let mut out = ModifierOutput::default();
        out.proj.fovy_add = desired - current;
        out
    }
}
