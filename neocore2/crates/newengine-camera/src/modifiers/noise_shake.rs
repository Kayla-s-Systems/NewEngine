#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Quat, Vec3};

use crate::stack::{CameraModifier, CameraStackInput, ModifierOutput};
use crate::{CameraRig, Projection};

/// Continuous deterministic camera shake.
///
/// This is designed for gameplay (explosions, impacts, ambient rumble).
/// The noise is hash-based (no allocation, no rand dependency).
pub struct NoiseShake {
    pub enabled: bool,
    /// World-space position amplitude.
    pub amplitude_pos: Vec3,
    /// Local-space euler amplitude in radians (pitch, yaw, roll).
    pub amplitude_rot: Vec3,
    pub frequency: f32,
    pub time: f32,
}

impl Default for NoiseShake {
    fn default() -> Self {
        Self {
            enabled: false,
            amplitude_pos: Vec3::ZERO,
            amplitude_rot: Vec3::ZERO,
            frequency: 12.0,
            time: 0.0,
        }
    }
}

impl CameraModifier for NoiseShake {
    fn apply(
        &mut self,
        _rig: &CameraRig,
        _proj: &Projection,
        input: &CameraStackInput,
    ) -> ModifierOutput {
        if !self.enabled {
            return ModifierOutput::default();
        }

        let dt = input.dt.max(0.0);
        self.time = (self.time + dt).min(1.0e9);
        let t = self.time * self.frequency.max(0.001);

        let mut intensity = input.intensity;
        if !intensity.is_finite() {
            intensity = 1.0;
        }
        intensity = intensity.clamp(0.0, 8.0);

        let s = input.seed;

        let n1 = hash_noise_1d(s ^ 0xA1, t);
        let n2 = hash_noise_1d(s ^ 0xB2, t + 17.0);
        let n3 = hash_noise_1d(s ^ 0xC3, t + 31.0);
        let pos = Vec3::new(
            n1 * self.amplitude_pos.x * intensity,
            n2 * self.amplitude_pos.y * intensity,
            n3 * self.amplitude_pos.z * intensity,
        );

        let r1 = hash_noise_1d(s ^ 0xD4, t + 7.0);
        let r2 = hash_noise_1d(s ^ 0xE5, t + 19.0);
        let r3 = hash_noise_1d(s ^ 0xF6, t + 43.0);
        let rot = Vec3::new(
            r1 * self.amplitude_rot.x * intensity,
            r2 * self.amplitude_rot.y * intensity,
            r3 * self.amplitude_rot.z * intensity,
        );

        let drot = Quat::from_rotation_z(rot.z)
            * Quat::from_rotation_y(rot.y)
            * Quat::from_rotation_x(rot.x);

        let mut out = ModifierOutput::default();
        out.pose.dpos_ws = pos;
        out.pose.drot_ls = drot;
        out
    }
}

#[inline]
fn hash_noise_1d(seed: u64, x: f32) -> f32 {
    // Hash (seed, x_bits) into a float in [-1, 1].
    let xb = x.to_bits() as u64;
    let mut v = seed ^ xb.wrapping_mul(0x9E3779B97F4A7C15);
    v ^= v >> 30;
    v = v.wrapping_mul(0xBF58476D1CE4E5B9);
    v ^= v >> 27;
    v = v.wrapping_mul(0x94D049BB133111EB);
    v ^= v >> 31;

    // Map u32 to [0,1] then to [-1,1].
    let u = (v as u32) as f32 / (u32::MAX as f32);
    u * 2.0 - 1.0
}
