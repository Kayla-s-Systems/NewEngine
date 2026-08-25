#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::CameraRig;
use newengine_core::host_events::CursorState;
use newengine_ecs::{EntityId, World};
use newengine_input_actions_api::move_mask as input_move;
use newengine_math::{wrap_pi, EulerRot, Mat3, Quat, Vec2, Vec3};
use newengine_sim::{
    step_follow_camera, CameraRigComp, CharacterMotor, FollowTargetCameraController,
    FollowTargetCameraMotor, MotorInput,
};
use newengine_transform::{
    read_entity_world_pose_local_chain, write_entity_local_from_world_pose_local_chain,
};

use crate::constraints::{
    constrain_spring_arm_offset_ls, CameraSpringArmCollisionWorld, CameraSpringArmConfig,
};
use crate::manager::{CameraDirectorRequest, CameraManagerResource};
use crate::modes::{
    GameplayFirstPersonRunner, GameplayThirdPersonAimRunner, GameplayThirdPersonFollowRunner,
    GameplayThirdPersonOrbitRunner,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameplayCameraRunnerKind {
    FirstPerson,
    ThirdPersonFollow,
    ThirdPersonAim,
    ThirdPersonOrbit,
}

#[derive(Clone, Copy, Debug)]
struct GameplayThirdPersonCameraState {
    runner: GameplayCameraRunnerKind,
    target: EntityId,
    anchor_ws: Vec3,
    zoom_z: f32,
    orbit_yaw: f32,
    orbit_pitch: f32,
    orbit_pivot_offset_ws: Vec3,
    collision_distance: f32,
    last_pivot_ws: Vec3,
    last_focus_ws: Vec3,
    last_desired_camera_ws: Vec3,
    last_collision_target_distance: f32,
    initialized: bool,
}

impl Default for GameplayThirdPersonCameraState {
    #[inline]
    fn default() -> Self {
        Self {
            runner: GameplayCameraRunnerKind::ThirdPersonFollow,
            target: EntityId::default(),
            anchor_ws: Vec3::ZERO,
            zoom_z: 0.0,
            orbit_yaw: 0.0,
            orbit_pitch: 0.0,
            orbit_pivot_offset_ws: Vec3::ZERO,
            collision_distance: 0.0,
            last_pivot_ws: Vec3::ZERO,
            last_focus_ws: Vec3::ZERO,
            last_desired_camera_ws: Vec3::ZERO,
            last_collision_target_distance: 0.0,
            initialized: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GameplayCameraTelemetrySnapshot {
    pub runner: GameplayCameraRunnerKind,
    pub target: EntityId,
    pub initialized: bool,
    pub anchor_ws: Vec3,
    pub orbit_yaw: f32,
    pub orbit_pitch: f32,
    pub orbit_pivot_offset_ws: Vec3,
    pub zoom_z: f32,
    pub collision_distance: f32,
    pub pivot_ws: Vec3,
    pub focus_ws: Vec3,
    pub desired_camera_ws: Vec3,
    pub collision_target_distance: f32,
    pub rig_position_ws: Vec3,
    pub rig_rotation: Quat,
}

#[inline]
fn normalized_gameplay_zoom_steps(wheel_y: f32) -> f32 {
    if !wheel_y.is_finite() || wheel_y.abs() <= f32::EPSILON {
        return 0.0;
    }
    if wheel_y.abs() > 10.0 {
        (wheel_y / 120.0).clamp(-4.0, 4.0)
    } else {
        wheel_y.clamp(-4.0, 4.0)
    }
}

#[inline]
fn gameplay_zoom_limits(runner: GameplayCameraRunnerKind) -> Option<(f32, f32)> {
    match runner {
        GameplayCameraRunnerKind::FirstPerson => None,
        GameplayCameraRunnerKind::ThirdPersonAim => Some((1.10, 4.50)),
        GameplayCameraRunnerKind::ThirdPersonFollow => Some((1.35, 9.00)),
        GameplayCameraRunnerKind::ThirdPersonOrbit => Some((1.35, 10.0)),
    }
}

#[inline]
fn orbit_angles_from_camera(pivot_ws: Vec3, camera_ws: Vec3) -> Option<(f32, f32)> {
    let dir = (camera_ws - pivot_ws).normalize_or_zero();
    if !dir.is_finite() || dir.length_squared() <= 1.0e-10 {
        return None;
    }
    // A first-person eye is commonly almost directly above the third-person pivot.
    // Yaw is undefined at that pole: atan2(tiny_x, tiny_z) amplifies sub-pixel pose noise
    // into arbitrary left/right orbit angles. Let the caller inherit the gameplay view instead.
    let horizontal_sq = dir.x * dir.x + dir.z * dir.z;
    if horizontal_sq <= 0.05 * 0.05 {
        return None;
    }
    let yaw = dir.x.atan2(dir.z);
    let pitch = (-dir.y).asin().clamp(-1.35, 1.35);
    Some((wrap_pi(yaw), pitch))
}

#[inline]
fn orbit_look_at_rotation(eye: Vec3, center: Vec3) -> Quat {
    let forward = (center - eye).normalize_or_zero();
    if forward.length_squared() <= 1.0e-12 {
        return Quat::IDENTITY;
    }
    let right = forward.cross(Vec3::Y).normalize_or_zero();
    if right.length_squared() <= 1.0e-12 {
        return Quat::IDENTITY;
    }
    let up = right.cross(forward).normalize_or_zero();
    Quat::from_mat3(&Mat3::from_cols(right, up, -forward)).normalize_or_identity()
}

#[inline]
fn smooth_gameplay_zoom(current: f32, target: f32, dt: f32) -> f32 {
    if !target.is_finite() {
        return current;
    }
    if !current.is_finite() || !(dt.is_finite() && dt > 0.0) {
        return target;
    }
    let alpha = (1.0 - (-dt / 0.09).exp()).clamp(0.0, 1.0);
    let next = current + (target - current) * alpha;
    if (target - next).abs() <= 1.0e-4 {
        target
    } else {
        next
    }
}

#[inline]
fn smooth_collision_release(current: f32, target: f32, dt: f32) -> f32 {
    if !target.is_finite() || target <= 0.0 {
        return current;
    }
    if !current.is_finite() || current <= 0.0 {
        return target;
    }
    // Triangle seams/contact quantization can move the measured hit distance by a few
    // millimetres from frame to frame. Reacting asymmetrically (instant retract, soft release)
    // turns that harmless noise into visible camera sway. The spring arm already keeps 8 cm of
    // authored collision padding, so a 1 cm distance deadband is safely inside that margin.
    const COLLISION_DISTANCE_HYSTERESIS: f32 = 0.010;
    if (target - current).abs() <= COLLISION_DISTANCE_HYSTERESIS {
        return current;
    }
    // Meaningful collision retraction is immediate so the camera never clips into geometry.
    if target < current {
        return target;
    }
    if !(dt.is_finite() && dt > 0.0) {
        return target;
    }
    // Release is intentionally softer to avoid a hard pop when a wall leaves the arm.
    let alpha = (1.0 - (-dt / 0.12).exp()).clamp(0.0, 1.0);
    let next = current + (target - current) * alpha;
    if (target - next).abs() <= 1.0e-4 {
        target
    } else {
        next
    }
}

impl Default for GameplayCameraRunnerKind {
    #[inline]
    fn default() -> Self {
        Self::FirstPerson
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CameraRuntimeServiceConfig {
    pub runner: GameplayCameraRunnerKind,
    pub first_person_eye_height: f32,
    /// Optional animated eye-center anchor supplied by the active avatar provider.
    pub first_person_anchor_ws: Option<Vec3>,
    /// Forward clearance from the eye center. This keeps the near plane in front of face geometry.
    pub first_person_forward_clearance: f32,
    /// Visual character center relative to the PlayerActor root.
    /// ThirdPersonOrbit uses this exact point as both orbit pivot and look-at target.
    pub third_person_orbit_pivot_offset_ls: Vec3,
    /// Optional render-cadence interpolated target pose supplied by the engine presentation layer.
    pub third_person_render_position_ws: Option<Vec3>,
    pub third_person_render_rotation_ws: Option<Quat>,
    pub sprint_multiplier: f32,
}

impl Default for CameraRuntimeServiceConfig {
    #[inline]
    fn default() -> Self {
        Self {
            runner: GameplayCameraRunnerKind::FirstPerson,
            first_person_eye_height: 1.6,
            first_person_anchor_ws: None,
            first_person_forward_clearance: 0.055,
            third_person_orbit_pivot_offset_ls: Vec3::ZERO,
            third_person_render_position_ws: None,
            third_person_render_rotation_ws: None,
            sprint_multiplier: 2.0,
        }
    }
}

pub struct CameraRuntimeService;

mod input;
mod lifecycle;
mod sync;

#[cfg(test)]
mod tests;
