#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{wrap_pi, Quat, Vec2, Vec3};

use crate::rig::CameraRig;

#[derive(Clone, Copy, Debug, Default)]
pub struct CameraControlInput {
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

impl CameraControlInput {
    #[inline]
    pub const fn idle() -> Self {
        Self {
            look_active: false,
            look_delta: Vec2::ZERO,
            move_axis: Vec3::ZERO,
            speed_mul: 1.0,
            zoom_delta: 0.0,
        }
    }
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
    /// Synchronize controller angles from the current rig rotation.
    ///
    /// Required when external code modifies `rig.rotation` (e.g. mode switch) to avoid a snap on
    /// the next `apply()`.
    #[inline]
    pub fn sync_from_rig(&mut self, rig: &CameraRig) {
        // Convention: forward is -Z.
        let fwd = rig.rotation * (-Vec3::Z);

        let fy = fwd.y.clamp(-1.0, 1.0);
        let pitch = fy.asin();

        // yaw = atan2(x, -z) so that forward (0,0,-1) => yaw=0
        // Sign convention:
        // forward is -Z. With right-handed +Y-up math, a positive yaw rotates the camera to the left,
        // so `fwd.x` becomes negative for positive yaw. To recover the same yaw later fed into
        // `Quat::from_rotation_y(yaw)`, we must invert X when extracting yaw from forward.
        let yaw = (-fwd.x).atan2(-fwd.z);

        self.yaw = wrap_pi(yaw);
        self.pitch = pitch.clamp(-self.pitch_limit, self.pitch_limit);
    }

    #[inline]
    pub fn apply(&mut self, rig: &mut CameraRig, input: CameraControlInput, dt: f32) {
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

        // Prevent unbounded growth (precision loss in `sin/cos` over long sessions).
        self.yaw = wrap_pi(self.yaw);

        self.pitch = self.pitch.clamp(-self.pitch_limit, self.pitch_limit);

        let rot_yaw = Quat::from_rotation_y(self.yaw);
        let rot_pitch = Quat::from_rotation_x(self.pitch);
        rig.rotation = (rot_yaw * rot_pitch).normalize_or_identity();

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
    /// Synchronize orbit controller state from the current rig pose.
    ///
    /// This is critical for editor workflows where the rig can be manipulated externally (e.g.
    /// switching between controllers, framing, gizmo focus). Without syncing, the first `apply()`
    /// after re-activation can reconstruct `rig.position` from stale `(target, yaw, pitch, distance)`
    /// and cause a visible teleport.
    #[inline]
    pub fn sync_from_rig(&mut self, rig: &CameraRig) {
        // Extract yaw/pitch from rig rotation using the same convention as FreeFly.
        let fwd = rig.rotation * (-Vec3::Z);

        let fy = fwd.y.clamp(-1.0, 1.0);
        let pitch = fy.asin();
        // NOTE: keep the same yaw sign convention as `FreeFlyController`.
        let yaw = (-fwd.x).atan2(-fwd.z);

        self.yaw = wrap_pi(yaw);
        self.pitch = pitch.clamp(-self.pitch_limit, self.pitch_limit);

        // Keep current distance, but recompute target so that rig stays exactly in place.
        self.distance = self.distance.clamp(self.min_distance, self.max_distance);

        // IMPORTANT:
        // Recompute `target` using the *reconstructed* rotation (the same one `apply()` will use).
        // Even tiny numeric / branch differences between extracting yaw/pitch from `rig.rotation`
        // and rebuilding a quaternion can cause a snap (often ~2*distance) on the first orbit frame.
        // Using the reconstructed rotation guarantees that:
        //   target + (rot*Z)*distance == rig.position
        // once `apply()` runs.
        let rot_yaw = Quat::from_rotation_y(self.yaw);
        let rot_pitch = Quat::from_rotation_x(self.pitch);
        let rot = (rot_yaw * rot_pitch).normalize_or_identity();

        let back = rot * Vec3::Z; // +Z is backward.
        self.target = rig.position - back * self.distance;
    }

    #[inline]
    pub fn apply(&mut self, rig: &mut CameraRig, input: CameraControlInput, dt: f32) {
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

        // Prevent unbounded growth (precision loss in `sin/cos` over long sessions).
        self.yaw = wrap_pi(self.yaw);

        self.pitch = self.pitch.clamp(-self.pitch_limit, self.pitch_limit);

        // Wheel zoom must stay predictable across tiny props and huge scenes alike.
        // The previous implementation multiplied distance by an unclamped scene-scaled step,
        // which could collapse the orbit radius in a single wheel tick on large levels.
        //
        // Policy:
        // - exponential zoom response (stable near zero, no sign-flip / overshoot);
        // - bounded per-frame response, independent from raw scene radius spikes;
        // - optional move_axis.z support remains additive for future orbit drag-dolly flows.
        let mut zoom_units = 0.0f32;
        if input.zoom_delta.is_finite() {
            zoom_units += input.zoom_delta;
        }
        if input.move_axis.z.is_finite() {
            zoom_units += input.move_axis.z * dt;
        }
        if zoom_units.abs() > 1e-6 {
            let bounded_speed_mul = speed_mul.clamp(0.25, 2.0);
            let bounded_zoom = zoom_units.clamp(-4.0, 4.0);
            let response = (0.12 * self.dolly_speed.max(0.05).sqrt() * bounded_speed_mul.sqrt())
                .clamp(0.05, 0.20);
            let zoom_factor = (-bounded_zoom * response).exp();
            self.distance = (self.distance * zoom_factor).clamp(self.min_distance, self.max_distance);
        }

        // Build rotation.
        let rot_yaw = Quat::from_rotation_y(self.yaw);
        let rot_pitch = Quat::from_rotation_x(self.pitch);
        let rot = (rot_yaw * rot_pitch).normalize_or_identity();

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
