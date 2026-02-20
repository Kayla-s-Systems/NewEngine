#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Quat, Vec3};

use crate::stack::{CameraModifier, CameraStackInput, ModifierOutput};
use crate::{CameraRig, Projection};

/// Head bob driven by movement velocity.
///
/// This modifier is safe for both first-person and third-person cameras.
pub struct HeadBob {
    pub enabled: bool,
    /// World-space position amplitude.
    pub amplitude_pos: Vec3,
    /// Local-space rotation amplitude in radians.
    pub amplitude_rot: Vec3,
    /// Steps per second at velocity = 1.0.
    pub frequency: f32,
    /// Minimum speed to start bobbing.
    pub min_speed: f32,
    pub time: f32,
}

impl Default for HeadBob {
    fn default() -> Self {
        Self {
            enabled: false,
            amplitude_pos: Vec3::new(0.0, 0.015, 0.0),
            amplitude_rot: Vec3::new(0.01, 0.0, 0.01),
            frequency: 1.8,
            min_speed: 0.2,
            time: 0.0,
        }
    }
}

impl CameraModifier for HeadBob {
    fn apply(
        &mut self,
        rig: &CameraRig,
        _proj: &Projection,
        input: &CameraStackInput,
    ) -> ModifierOutput {
        if !self.enabled || !input.is_grounded {
            return ModifierOutput::default();
        }

        let dt = input.dt.max(0.0);
        let speed = input.velocity_ws.length();

        if !speed.is_finite() || speed < self.min_speed {
            return ModifierOutput::default();
        }

        let mut intensity = if input.intensity.is_finite() {
            input.intensity
        } else {
            1.0
        };
        intensity = intensity.clamp(0.0, 4.0);

        let freq = self.frequency.max(0.001) * speed;
        self.time = (self.time + dt * freq).min(1.0e9);

        let phase = self.time * std::f32::consts::TAU;

        // Classic walk bob: 2x vertical, 1x lateral
        let s1 = phase.sin();
        let s2 = (phase * 2.0).sin();

        // Explicit component-wise scaling (deterministic, no Vec3*Vec3 needed)
        let local_pos = Vec3::new(
            s1 * self.amplitude_pos.x * intensity,
            s2.abs() * self.amplitude_pos.y * intensity,
            0.0,
        );

        let local_rot = Vec3::new(
            s2 * self.amplitude_rot.x * intensity,
            0.0,
            -s1 * self.amplitude_rot.z * intensity,
        );

        let dpos_ws = rig.rotation * local_pos;

        let drot =
            Quat::from_rotation_z(local_rot.z)
                * Quat::from_rotation_y(local_rot.y)
                * Quat::from_rotation_x(local_rot.x);

        let mut out = ModifierOutput::default();
        out.pose.dpos_ws = dpos_ws;
        out.pose.drot_ls = drot;
        out
    }
}