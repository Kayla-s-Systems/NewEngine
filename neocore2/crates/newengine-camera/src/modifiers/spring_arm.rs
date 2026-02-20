#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_math::{Quat, Vec3};

use crate::stack::{CameraModifier, CameraStackInput, ModifierOutput};
use crate::util::exp_smooth;
use crate::{CameraRig, Projection};

/// Optional collision resolver for spring arm.
///
/// The engine can provide an implementation backed by a physics query.
/// The resolver is expected to return a camera position that is safe to use.
pub trait SpringArmCollision: Send + Sync {
    fn resolve(&self, origin_ws: Vec3, desired_ws: Vec3, radius: f32) -> Vec3;
}

/// Third-person spring arm.
///
/// Assumes the stack starts from an anchor pose (character pivot/head).
/// Produces a camera position behind the anchor and a look-at rotation.
///
/// Order recommendation:
/// - SpringArm
/// - ADS FOV
/// - Sway / Recoil / Shake / TAA
pub struct SpringArm {
    /// Arm length behind the anchor (meters).
    pub length: f32,
    /// Local-space socket offset applied to the anchor before arm direction.
    pub socket_offset_ls: Vec3,
    /// Local-space look-at offset applied to the anchor when computing rotation.
    pub look_at_offset_ls: Vec3,

    /// Position smoothing speed in 1/sec.
    pub pos_smooth: f32,
    /// Rotation smoothing speed in 1/sec.
    pub rot_smooth: f32,

    /// Collision sphere radius.
    pub collision_radius: f32,
    /// Optional collision resolver.
    pub collision: Option<Arc<dyn SpringArmCollision>>,

    smoothed_rot: Quat,
    smoothed_pos: Vec3,
    initialized: bool,
}

impl SpringArm {
    #[inline]
    pub fn new(length: f32) -> Self {
        Self {
            length: length.max(0.0),
            socket_offset_ls: Vec3::ZERO,
            look_at_offset_ls: Vec3::ZERO,
            pos_smooth: 18.0,
            rot_smooth: 18.0,
            collision_radius: 0.15,
            collision: None,
            smoothed_rot: Quat::IDENTITY,
            smoothed_pos: Vec3::ZERO,
            initialized: false,
        }
    }

    #[inline]
    pub fn with_collision(mut self, radius: f32, collision: Arc<dyn SpringArmCollision>) -> Self {
        self.collision_radius = radius.max(0.0);
        self.collision = Some(collision);
        self
    }

    #[inline]
    pub fn with_offsets(mut self, socket_offset_ls: Vec3, look_at_offset_ls: Vec3) -> Self {
        self.socket_offset_ls = socket_offset_ls;
        self.look_at_offset_ls = look_at_offset_ls;
        self
    }
}

impl Default for SpringArm {
    fn default() -> Self {
        Self::new(3.0).with_offsets(Vec3::new(0.25, 1.55, 0.0), Vec3::new(0.0, 1.55, 0.0))
    }
}

impl CameraModifier for SpringArm {
    fn apply(&mut self, rig: &CameraRig, _proj: &Projection, input: &CameraStackInput) -> ModifierOutput {
        let dt = input.dt.max(0.0);

        // Anchor is the current rig pose.
        let anchor_pos = rig.position;
        let anchor_rot = rig.rotation;

        let socket_pos = anchor_pos + anchor_rot * self.socket_offset_ls;
        let look_at = anchor_pos + anchor_rot * self.look_at_offset_ls;

        // Behind the anchor, along anchor forward.
        let desired = socket_pos - (anchor_rot * Vec3::Z * -1.0) * self.length;

        let desired = if let Some(c) = &self.collision {
            c.resolve(socket_pos, desired, self.collision_radius)
        } else {
            desired
        };

        // Initialize on the first tick to avoid a snap from zero.
        if !self.initialized {
            self.smoothed_pos = desired;
            self.smoothed_rot = crate::rig::CameraRig::from_look_at(desired, look_at, Vec3::Y).rotation;
            self.initialized = true;
        }

        self.smoothed_pos = exp_smooth_vec3(self.smoothed_pos, desired, self.pos_smooth, dt);

        let target_rot = crate::rig::CameraRig::from_look_at(self.smoothed_pos, look_at, Vec3::Y).rotation;
        self.smoothed_rot = exp_smooth_quat(self.smoothed_rot, target_rot, self.rot_smooth, dt);

        let mut out = ModifierOutput::default();
        out.pose.dpos_ws = self.smoothed_pos - rig.position;

        // Output local-space delta so stack composition stays correct.
        out.pose.drot_ls = rig.rotation.conjugate() * self.smoothed_rot;
        out
    }
}

#[inline]
fn exp_smooth_vec3(current: Vec3, target: Vec3, speed: f32, dt: f32) -> Vec3 {
    let k = exp_smooth(speed, dt);
    current + (target - current) * k
}

#[inline]
fn exp_smooth_quat(current: Quat, target: Quat, speed: f32, dt: f32) -> Quat {
    let k = exp_smooth(speed, dt);
    current.slerp(target, k).normalize_or_identity()
}
