#![forbid(unsafe_op_in_unsafe_fn)]

use glam::{Quat, Vec2, Vec3};

use crate::rig::CameraRig;

#[derive(Clone, Copy, Debug, Default)]
pub struct CameraInput {
    pub look_delta: Vec2,
    pub move_axis: Vec3, // x=right, y=up, z=forward
    pub speed_mul: f32,
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

        let dx = input.look_delta.x;
        let dy = input.look_delta.y;

        if dx.is_finite() {
            self.yaw += dx * self.look_sens;
        }
        if dy.is_finite() {
            self.pitch += dy * self.look_sens;
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