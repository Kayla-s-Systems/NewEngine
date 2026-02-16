#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Quat, Vec3};

use crate::stack::{CameraModifier, CameraStackInput, ModifierOutput};
use crate::{CameraRig, Projection};

/// Slow sway for breathing/idle camera.
pub struct Sway {
    pub enabled: bool,
    pub amplitude_pos: Vec3,
    pub amplitude_rot: Vec3,
    pub frequency: f32,
    pub time: f32,
}

impl Default for Sway {
    fn default() -> Self {
        Self {
            enabled: false,
            amplitude_pos: Vec3::new(0.0, 0.01, 0.0),
            amplitude_rot: Vec3::new(0.006, 0.004, 0.006),
            frequency: 0.25,
            time: 0.0,
        }
    }
}

impl CameraModifier for Sway {
    fn apply(&mut self, rig: &CameraRig, _proj: &Projection, input: &CameraStackInput) -> ModifierOutput {
        if !self.enabled {
            return ModifierOutput::default();
        }

        let dt = input.dt.max(0.0);
        self.time = (self.time + dt * self.frequency.max(0.001)).min(1.0e9);

        let mut intensity = input.intensity;
        if !intensity.is_finite() {
            intensity = 1.0;
        }
        intensity = intensity.clamp(0.0, 4.0);

        let phase = self.time * std::f32::consts::TAU;
        let s = phase.sin();
        let c = phase.cos();

        let local_pos = Vec3::new(s, c.abs(), 0.0) * (self.amplitude_pos * intensity);
        let local_rot = Vec3::new(c, s * 0.5, -s) * (self.amplitude_rot * intensity);

        let dpos_ws = rig.rotation * local_pos;
        let drot = Quat::from_rotation_z(local_rot.z)
            * Quat::from_rotation_y(local_rot.y)
            * Quat::from_rotation_x(local_rot.x);

        let mut out = ModifierOutput::default();
        out.pose.dpos_ws = dpos_ws;
        out.pose.drot_ls = drot;
        out
    }
}
