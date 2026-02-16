#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Quat, Vec2, Vec3};

use crate::rig::CameraRig;

#[derive(Clone, Copy, Debug, Default)]
pub struct CameraInput {
    /// Whether look input should be applied (e.g. RMB held).
    pub look_active: bool,
    pub look_delta: Vec2,

    /// Generic motion axes.
    /// FreeFly: x=right, y=up, z=forward.
    /// Orbit: x=pan right, y=pan up, z=dolly (positive -> forward).
    pub move_axis: Vec3,

    pub speed_mul: f32,

    /// Mouse wheel delta (positive -> zoom in).
    pub zoom_delta: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct FreeFlyController {
    pub yaw: f32,
    pub pitch: f32,

    pub look_sens: f32,
    pub move_speed: f32,

    pub pitch_limit: f32,
}

impl Default for FreeFlyController {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            look_sens: 0.0025,
            move_speed: 6.0,
            pitch_limit: 1.54,
        }
    }
}

impl FreeFlyController {
    #[inline]
    pub fn apply(&mut self, rig: &mut CameraRig, input: CameraInput, dt: f32) {
        let speed_mul = if input.speed_mul.is_finite() && input.speed_mul > 0.0 {
            input.speed_mul
        } else {
            1.0
        };

        if input.look_active {
            let dx = input.look_delta.x;
            let dy = input.look_delta.y;

            if dx.is_finite() {
                self.yaw += dx * self.look_sens;
            }
            if dy.is_finite() {
                self.pitch += dy * self.look_sens;
            }
        }

        self.pitch = self.pitch.clamp(-self.pitch_limit, self.pitch_limit);

        let rot_yaw = Quat::from_rotation_y(self.yaw);
        let rot_pitch = Quat::from_rotation_x(self.pitch);
        rig.rotation = rot_yaw * rot_pitch;

        let local = Vec3::new(input.move_axis.x, input.move_axis.y, -input.move_axis.z);
        let len = local.length();
        if len > 1e-6 && dt.is_finite() && dt > 0.0 {
            let dir = local / len;
            let delta = dir * (self.move_speed * speed_mul * dt);
            rig.translate_local(delta);
        }
    }
}

/// Orbit controller suitable for editor AND for gameplay (e.g. strategy/ARPG camera).
#[derive(Clone, Copy, Debug)]
pub struct OrbitController {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,

    pub look_sens: f32,
    pub pan_speed: f32,
    pub dolly_speed: f32,

    pub pitch_limit: f32,
    pub min_distance: f32,
    pub max_distance: f32,
}

impl Default for OrbitController {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            distance: 3.0,
            look_sens: 0.003,
            pan_speed: 1.0,
            dolly_speed: 6.0,
            pitch_limit: 1.54,
            min_distance: 0.05,
            max_distance: 50_000.0,
        }
    }
}

impl OrbitController {
    #[inline]
    pub fn apply(&mut self, rig: &mut CameraRig, input: CameraInput, dt: f32) {
        let speed_mul = if input.speed_mul.is_finite() && input.speed_mul > 0.0 {
            input.speed_mul
        } else {
            1.0
        };

        if input.look_active {
            let dx = input.look_delta.x;
            let dy = input.look_delta.y;

            if dx.is_finite() {
                self.yaw += dx * self.look_sens;
            }
            if dy.is_finite() {
                self.pitch += dy * self.look_sens;
            }
        }

        self.pitch = self.pitch.clamp(-self.pitch_limit, self.pitch_limit);

        // Dolly via mouse wheel (zoom_delta) and via move_axis.z (e.g. middle-mouse drag).
        let mut dolly = 0.0f32;
        if input.zoom_delta.is_finite() {
            dolly += input.zoom_delta;
        }
        if input.move_axis.z.is_finite() {
            dolly += input.move_axis.z * dt;
        }
        if dolly.abs() > 1e-6 {
            let step = dolly * self.dolly_speed * speed_mul * 0.1;
            self.distance = (self.distance * (1.0 - step).clamp(0.02, 50.0))
                .clamp(self.min_distance, self.max_distance);
        }

        // Build rotation.
        let rot_yaw = Quat::from_rotation_y(self.yaw);
        let rot_pitch = Quat::from_rotation_x(self.pitch);
        let rot = rot_yaw * rot_pitch;

        // Pan in camera plane using move_axis.xy.
        let pan = Vec2::new(input.move_axis.x, input.move_axis.y);
        if pan.length_squared() > 1e-10 && dt.is_finite() && dt > 0.0 {
            // Pan scale depends on distance: feels correct both in editor and game.
            let scale = self.pan_speed * speed_mul * dt * self.distance.max(0.25);
            let right = rot * Vec3::X;
            let up = rot * Vec3::Y;
            self.target += right * (pan.x * scale) + up * (pan.y * scale);
        }

        // Rig position from spherical orbit.
        let back = rot * Vec3::Z; // +Z points backward in our convention.
        rig.position = self.target + back * self.distance;
        rig.rotation = rot;
    }
}

/// Universal controller selection.
/// Game can run with `None` and drive `CameraRig` directly.
#[derive(Clone, Copy, Debug)]
pub enum CameraController {
    None,
    FreeFly(FreeFlyController),
    Orbit(OrbitController),
}

impl Default for CameraController {
    #[inline]
    fn default() -> Self {
        CameraController::FreeFly(FreeFlyController::default())
    }
}

impl CameraController {
    #[inline]
    pub fn apply(&mut self, rig: &mut CameraRig, input: CameraInput, dt: f32) {
        match self {
            CameraController::None => {}
            CameraController::FreeFly(c) => c.apply(rig, input, dt),
            CameraController::Orbit(c) => c.apply(rig, input, dt),
        }
    }
}