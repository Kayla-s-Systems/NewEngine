#![forbid(unsafe_op_in_unsafe_fn)]

use glam::{Quat, Vec2};

use crate::stack::{CameraModifier, CameraStackInput, ModifierOutput};
use crate::{CameraRig, Projection};

/// Deterministic recoil modifier.
///
/// Call `kick()` to add an impulse in radians (pitch, yaw).
/// The modifier will smoothly recover back to zero.
pub struct Recoil {
    pub enabled: bool,
    /// Recovery frequency (bigger = faster return).
    pub recovery_frequency: f32,
    /// Maximum recoil angle magnitude.
    pub max_angle: f32,

    vel: Vec2,
    offset: Vec2,
    pending_kick: Vec2,
}

impl Default for Recoil {
    fn default() -> Self {
        Self {
            enabled: true,
            recovery_frequency: 18.0,
            max_angle: 1.25,
            vel: Vec2::ZERO,
            offset: Vec2::ZERO,
            pending_kick: Vec2::ZERO,
        }
    }
}

impl Recoil {
    /// Adds an instantaneous recoil impulse.
    ///
    /// `pitch` positive means camera rotates up (looking higher).
    #[inline]
    pub fn kick(&mut self, pitch: f32, yaw: f32) {
        if pitch.is_finite() && yaw.is_finite() {
            self.pending_kick += Vec2::new(pitch, yaw);
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.vel = Vec2::ZERO;
        self.offset = Vec2::ZERO;
        self.pending_kick = Vec2::ZERO;
    }
}

impl CameraModifier for Recoil {
    fn apply(&mut self, _rig: &CameraRig, _proj: &Projection, input: &CameraStackInput) -> ModifierOutput {
        if !self.enabled {
            self.pending_kick = Vec2::ZERO;
            return ModifierOutput::default();
        }

        let dt = input.dt.max(0.0);

        // Apply pending kick as instantaneous velocity change.
        if self.pending_kick.length_squared() > 0.0 {
            self.offset += self.pending_kick;
            self.pending_kick = Vec2::ZERO;
        }

        // Critically damped spring to zero.
        let w = self.recovery_frequency.max(0.001);
        let k = w * w;
        let c = 2.0 * w;

        let acc = (Vec2::ZERO - self.offset) * k - self.vel * c;
        self.vel += acc * dt;
        self.offset += self.vel * dt;

        // Clamp.
        let len = self.offset.length();
        if self.max_angle > 0.0 && len > self.max_angle {
            self.offset = self.offset * (self.max_angle / len);
            self.vel = Vec2::ZERO;
        }

        // Convert to rotation delta.
        let pitch = self.offset.x;
        let yaw = self.offset.y;
        let drot = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);

        let mut out = ModifierOutput::default();
        out.pose.drot_ls = drot;
        out
    }
}
