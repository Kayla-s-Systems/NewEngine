#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Quat, Vec2};

use crate::stack::{CameraModifier, CameraStackInput, ModifierOutput};
use crate::{CameraRig, Projection};

/// Weapon sway / aim sway driven by look delta.
///
/// Implementation: critically damped spring in angle space.
pub struct WeaponSway {
    /// Angle strength per pixel.
    pub strength: f32,
    /// Spring frequency (bigger = snappier).
    pub frequency: f32,
    /// Maximum sway angle in radians.
    pub max_angle: f32,

    /// Multiplier applied when aiming (ADS). Typical: 0.2..0.6.
    pub aim_multiplier: f32,

    vel: Vec2,
    offset: Vec2,
}

impl WeaponSway {
    #[inline]
    pub fn new(strength: f32, frequency: f32, max_angle: f32) -> Self {
        Self {
            strength,
            frequency: frequency.max(0.001),
            max_angle: max_angle.max(0.0),
            aim_multiplier: 0.35,
            vel: Vec2::ZERO,
            offset: Vec2::ZERO,
        }
    }
}

impl Default for WeaponSway {
    fn default() -> Self {
        Self::new(0.0025, 10.0, 0.25)
    }
}

impl CameraModifier for WeaponSway {
    fn apply(
        &mut self,
        _rig: &CameraRig,
        _proj: &Projection,
        input: &CameraStackInput,
    ) -> ModifierOutput {
        let dt = input.dt.max(0.0);

        let mut intensity = input.intensity;
        if !intensity.is_finite() {
            intensity = 1.0;
        }
        intensity = intensity.clamp(0.0, 4.0);

        let target = input.look_delta * (self.strength * intensity);
        let aim_mul = if input.is_aiming {
            self.aim_multiplier.clamp(0.0, 1.0)
        } else {
            1.0
        };

        let target = target * aim_mul;

        let w = self.frequency;
        let k = w * w;
        let c = 2.0 * w;

        // x'' + 2*w*x' + w^2*(x - target) = 0
        let acc = (target - self.offset) * k - self.vel * c;
        self.vel += acc * dt;
        self.offset += self.vel * dt;

        // Clamp offset.
        if self.max_angle > 0.0 {
            let len = self.offset.length();
            if len > self.max_angle {
                self.offset *= self.max_angle / len;
                self.vel = Vec2::ZERO;
            }
        }

        // Convention: positive look_delta.x should yaw right; positive y should pitch down.
        let yaw = -self.offset.x;
        let pitch = -self.offset.y;
        let drot = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);

        let mut out = ModifierOutput::default();
        out.pose.drot_ls = drot;
        out
    }
}
