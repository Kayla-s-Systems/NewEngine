#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;
use newengine_math::{Mat3, Quat, Vec3};

use crate::{
    CameraRigComp, ControllerCtx, FollowTargetCameraController, FollowTargetCameraMotor, Intent,
    IntentSink,
};

/// Output of a follow-camera motor step.
#[derive(Clone, Copy, Debug)]
pub struct FollowCameraStep {
    pub next_pos: Vec3,
    pub next_rot: Quat,
    pub next_vel: Vec3,
}

#[inline]
fn sanitize_dt(dt: f32) -> Option<f32> {
    if !dt.is_finite() || dt <= 0.0 {
        return None;
    }
    // Clamp to avoid numerical explosions on stalls/breakpoints.
    Some(dt.min(0.1))
}

#[inline]
fn sanitize_smooth_time(t: f32) -> f32 {
    if t.is_finite() && t > 0.0 {
        t.max(1.0e-4)
    } else {
        0.0
    }
}

#[inline]
fn sanitize_max_speed(v: f32) -> f32 {
    if v.is_finite() && v > 0.0 {
        v
    } else {
        0.0
    }
}

/// Deterministic SmoothDamp-like step for position.
///
/// Based on the critically-damped spring used in common engines.
#[inline]
fn smooth_damp_vec3(
    current: Vec3,
    target: Vec3,
    current_vel: Vec3,
    smooth_time: f32,
    max_speed: f32,
    dt: f32,
) -> (Vec3, Vec3) {
    if smooth_time <= 0.0 {
        return (target, Vec3::ZERO);
    }

    let omega = 2.0 / smooth_time;
    let x = omega * dt;
    // Stable rational approximation of exp(-x).
    let exp = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);

    let mut change = current - target;
    let original_to = target;

    // Clamp max speed.
    if max_speed > 0.0 {
        let max_change = max_speed * smooth_time;
        let len = change.length();
        if len > max_change {
            change = change / len * max_change;
        }
    }

    let target = current - change;
    let temp = (current_vel + change * omega) * dt;
    let mut next_vel = (current_vel - temp * omega) * exp;
    let mut next = target + (change + temp) * exp;

    // Prevent overshoot.
    let to_orig = original_to - current;
    let to_next = next - original_to;
    if to_orig.dot(to_next) > 0.0 {
        next = original_to;
        next_vel = (next - original_to) / dt;
    }

    (next, next_vel)
}

#[inline]
fn look_at_rotation_rh(eye: Vec3, center: Vec3, up: Vec3) -> Quat {
    let f = (center - eye).normalize_or_zero();
    if f.length_squared() < 1.0e-12 {
        return Quat::IDENTITY;
    }

    let s = f.cross(up).normalize_or_zero();
    let u = s.cross(f);

    // Match `Mat4::look_at_rh` basis: columns (s, u, -f)
    let m = Mat3::from_cols(s, u, -f);
    Quat::from_mat3(&m).normalize_or_identity()
}

/// Pure follow-camera motor step.
///
/// - `target_pos/rot`: target world pose.
/// - `offset_ls`: offset in target local space.
/// - `rot_offset`: additional rotation offset (only used when `follow_rotation` is true).
/// - `follow_rotation`: if false, camera will look at the target.
/// - `smooth_time/max_speed`: position smoothing params.
#[inline]
pub fn step_follow_camera(
    current_pos: Vec3,
    current_rot: Quat,
    target_pos: Vec3,
    target_rot: Quat,
    offset_ls: Vec3,
    rot_offset: Quat,
    follow_rotation: bool,
    current_vel: Vec3,
    smooth_time: f32,
    max_speed: f32,
    dt: f32,
) -> Option<FollowCameraStep> {
    let dt = sanitize_dt(dt)?;
    let smooth_time = sanitize_smooth_time(smooth_time);
    let max_speed = sanitize_max_speed(max_speed);

    let target_rot = target_rot.normalize_or_identity();
    let desired_pos = target_pos + target_rot * offset_ls;
    let (next_pos, next_vel) = smooth_damp_vec3(
        current_pos,
        desired_pos,
        current_vel,
        smooth_time,
        max_speed,
        dt,
    );

    let desired_rot = if follow_rotation {
        (target_rot * rot_offset).normalize_or_identity()
    } else {
        look_at_rotation_rh(next_pos, target_pos, Vec3::Y)
    };

    // Rotation smoothing: match position smoothing time-constant (exponential decay).
    let next_rot = if smooth_time <= 0.0 {
        desired_rot
    } else {
        let alpha = 1.0 - (-dt / smooth_time).exp();
        current_rot.normalize_or_identity().slerp(desired_rot, alpha)
    };

    Some(FollowCameraStep {
        next_pos,
        next_rot,
        next_vel,
    })
}

/// ECS-agnostic controller runner producing semantic intents.
#[inline]
pub fn run_follow_camera_controller(
    entity: EntityId,
    ctx: &ControllerCtx<'_>,
    ctrl: FollowTargetCameraController,
    rig: CameraRigComp,
    motor: FollowTargetCameraMotor,
    out: &mut impl IntentSink,
) {
    let Some((target_pos, target_rot)) = ctx.read_world_pose(ctrl.target) else {
        return;
    };

    let Some(step) = step_follow_camera(
        rig.0.position,
        rig.0.rotation,
        target_pos,
        target_rot,
        ctrl.offset_ls,
        ctrl.rot_offset,
        ctrl.follow_rotation,
        motor.vel_ws,
        ctrl.smooth_time,
        ctrl.max_speed,
        ctx.dt(),
    ) else {
        return;
    };

    out.emit(Intent::SetCameraRig {
        entity,
        value: CameraRigComp(newengine_camera::CameraRig {
            position: step.next_pos,
            rotation: step.next_rot,
        }),
    });
    out.emit(Intent::SetFollowTargetCameraMotor {
        entity,
        value: FollowTargetCameraMotor {
            vel_ws: step.next_vel,
        },
    });
}

/// Computes follow-controller parameters (`offset_ls`, `rot_offset`) that reproduce a desired
/// camera world pose for a given target world pose.
///
/// This is an editor/tooling helper that lets higher-level code keep a camera "attached" to a
/// target entity while still allowing manual navigation (Fly / Orbit). Instead of disabling the
/// follow controller (which would snap back on release), tools should update the controller
/// parameters to match the newly-authored camera pose.
///
/// - `offset_ls` is always computed.
/// - `rot_offset` is meaningful only when the follow controller is configured with
///   `follow_rotation = true`.
#[inline]
pub fn follow_params_from_pose(
    target_pos: Vec3,
    target_rot: Quat,
    camera_pos: Vec3,
    camera_rot: Quat,
) -> (Vec3, Quat) {
    let tr = target_rot.normalize_or_identity();
    let inv = tr.inverse();
    let offset_ls = inv * (camera_pos - target_pos);
    let rot_offset = (inv * camera_rot.normalize_or_identity()).normalize_or_identity();
    (offset_ls, rot_offset)
}
