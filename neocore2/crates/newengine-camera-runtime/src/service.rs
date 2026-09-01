#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::CameraRig;
use newengine_core::host_events::CursorState;
use newengine_ecs::{EntityId, World};
use newengine_input_actions_api::move_mask as input_move;
use newengine_math::{EulerRot, Mat3, Quat, Vec2, Vec3, wrap_pi};
use newengine_sim::{
    CameraRigComp, CharacterMotor, FollowTargetCameraController, FollowTargetCameraMotor,
    MotorInput,
};
use newengine_transform::{
    read_entity_world_pose_local_chain, write_entity_local_from_world_pose_local_chain,
};

use crate::constraints::{
    CameraSpringArmCollisionWorld, CameraSpringArmConfig, constrain_spring_arm_offset_ls,
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
struct GameplayCameraRunnerHistory {
    target: EntityId,
    runner: GameplayCameraRunnerKind,
    initialized: bool,
}

impl Default for GameplayCameraRunnerHistory {
    fn default() -> Self {
        Self {
            target: EntityId::default(),
            runner: GameplayCameraRunnerKind::FirstPerson,
            initialized: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct GameplayFirstPersonCameraState {
    target: EntityId,
    stable_eye_anchor_ws: Vec3,
    aim_alpha: f32,
    recoil_pitch_radians: f32,
    recoil_yaw_radians: f32,
    last_shot_sequence: u64,
    initialized: bool,
}

impl Default for GameplayFirstPersonCameraState {
    #[inline]
    fn default() -> Self {
        Self {
            target: EntityId::default(),
            stable_eye_anchor_ws: Vec3::ZERO,
            aim_alpha: 0.0,
            recoil_pitch_radians: 0.0,
            recoil_yaw_radians: 0.0,
            last_shot_sequence: 0,
            initialized: false,
        }
    }
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
    collision_velocity: f32,
    collision_blocked: bool,
    catch_up_offset_ls: Vec3,
    catch_up_velocity_ls: Vec3,
    catch_up_active: bool,
    look_rotation: Quat,
    look_initialized: bool,
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
            collision_velocity: 0.0,
            collision_blocked: false,
            catch_up_offset_ls: Vec3::ZERO,
            catch_up_velocity_ls: Vec3::ZERO,
            catch_up_active: false,
            look_rotation: Quat::IDENTITY,
            look_initialized: false,
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
    pub collision_velocity: f32,
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
fn gameplay_zoom_limits(config: CameraRuntimeServiceConfig) -> Option<(f32, f32)> {
    match config.runner {
        GameplayCameraRunnerKind::FirstPerson => None,
        GameplayCameraRunnerKind::ThirdPersonAim => Some((
            config.third_person_aim_zoom_min,
            config.third_person_aim_zoom_max,
        )),
        GameplayCameraRunnerKind::ThirdPersonFollow => Some((
            config.third_person_follow_zoom_min,
            config.third_person_follow_zoom_max,
        )),
        GameplayCameraRunnerKind::ThirdPersonOrbit => Some((
            config.third_person_orbit_zoom_min,
            config.third_person_orbit_zoom_max,
        )),
    }
}

#[inline]
fn orbit_angles_from_camera(
    pivot_ws: Vec3,
    camera_ws: Vec3,
    pitch_min_radians: f32,
    pitch_max_radians: f32,
) -> Option<(f32, f32)> {
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
    let pitch_min = pitch_min_radians.clamp(-89.0_f32.to_radians(), 88.0_f32.to_radians());
    let pitch_max =
        pitch_max_radians.clamp(pitch_min + 1.0_f32.to_radians(), 89.0_f32.to_radians());
    let pitch = (-dir.y).asin().clamp(pitch_min, pitch_max);
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
fn smooth_gameplay_zoom(current: f32, target: f32, dt: f32, smooth_time_seconds: f32) -> f32 {
    if !target.is_finite() {
        return current;
    }
    if !current.is_finite() || !(dt.is_finite() && dt > 0.0) {
        return target;
    }
    let smooth_time_seconds = if smooth_time_seconds.is_finite() {
        smooth_time_seconds.clamp(0.001, 5.0)
    } else {
        0.09
    };
    let alpha = (1.0 - (-dt / smooth_time_seconds).exp()).clamp(0.0, 1.0);
    let next = current + (target - current) * alpha;
    if (target - next).abs() <= 1.0e-4 {
        target
    } else {
        next
    }
}

#[inline]
fn step_collision_distance_response(
    current: f32,
    velocity: f32,
    target: f32,
    dt: f32,
    release_frequency_hz: f32,
    damping_ratio: f32,
    hysteresis_m: f32,
) -> (f32, f32) {
    if !target.is_finite() || target <= 0.0 {
        return (current, 0.0);
    }
    if !current.is_finite() || current <= 0.0 {
        return (target, 0.0);
    }
    let hysteresis_m = if hysteresis_m.is_finite() {
        hysteresis_m.clamp(0.0, 0.25)
    } else {
        0.005
    };
    let error = target - current;
    if error.abs() <= hysteresis_m {
        // Contact-triangle seams and query quantization are not camera motion. Latch the previous
        // safe distance and discard residual spring energy inside the authored deadband.
        return (current, 0.0);
    }
    if target < current {
        // Collision push-in is a safety constraint: never spring through geometry. Only pull-back
        // after free space reappears is damped, matching the reference collision response model.
        return (target, 0.0);
    }
    if !(dt.is_finite() && dt > 0.0) {
        return (target, 0.0);
    }

    let frequency_hz = if release_frequency_hz.is_finite() {
        release_frequency_hz.clamp(0.01, 60.0)
    } else {
        1.6
    };
    let damping_ratio = if damping_ratio.is_finite() {
        damping_ratio.clamp(0.05, 4.0)
    } else {
        0.8
    };
    let velocity = if velocity.is_finite() { velocity } else { 0.0 };
    let dt = dt.min(0.05);
    let omega = core::f32::consts::TAU * frequency_hz;

    // Stable implicit damped-spring integration. It retains velocity between frames, unlike a
    // first-order exponential, while remaining well behaved after a render hitch.
    let f = 1.0 + 2.0 * dt * damping_ratio * omega;
    let omega_sq = omega * omega;
    let h_omega_sq = dt * omega_sq;
    let hh_omega_sq = dt * h_omega_sq;
    let inv_det = (f + hh_omega_sq).recip();
    let mut next = (f * current + dt * velocity + hh_omega_sq * target) * inv_det;
    let mut next_velocity = (velocity + h_omega_sq * (target - current)) * inv_det;

    // Collision recovery may approach the desired orbit distance but must never overshoot it and
    // create a one-frame cut-back on the following query.
    if !next.is_finite() || !next_velocity.is_finite() {
        return (target, 0.0);
    }
    next = next.clamp(current.min(target), current.max(target));
    if target - next <= 1.0e-4 {
        next = target;
        next_velocity = 0.0;
    }
    (next, next_velocity)
}

impl Default for GameplayCameraRunnerKind {
    #[inline]
    fn default() -> Self {
        Self::FirstPerson
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FirstPersonPresentationInput {
    /// Fixed-step grounded state. Bobbing is suppressed while airborne.
    pub grounded: bool,
    /// Horizontal player speed in metres per second.
    pub horizontal_speed: f32,
    /// Semantic aim intent. Camera runtime smooths this independently from weapon placement.
    pub aiming: bool,
    /// Monotonic weapon shot sequence; a changed value injects exactly one visual recoil impulse.
    pub shot_sequence: u64,
    pub recoil_pitch_radians: f32,
    pub recoil_pitch_random_radians: f32,
    pub recoil_yaw_radians: f32,
    pub recoil_yaw_bias_radians: f32,
    pub ads_recoil_multiplier: f32,
    pub recoil_recovery_hz: f32,
}

impl Default for FirstPersonPresentationInput {
    fn default() -> Self {
        Self {
            grounded: false,
            horizontal_speed: 0.0,
            aiming: false,
            shot_sequence: 0,
            recoil_pitch_radians: 0.0,
            recoil_pitch_random_radians: 0.0,
            recoil_yaw_radians: 0.0,
            recoil_yaw_bias_radians: 0.0,
            ads_recoil_multiplier: 0.78,
            recoil_recovery_hz: 7.5,
        }
    }
}

/// Provider-neutral first-person presentation safety metadata resolved by the gameplay/avatar
/// layer. Geometry fields remain available to presentation providers, but camera position is never
/// projected against the local owner's body: FPP owner visibility/topology owns that problem.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FirstPersonBodyBarrierInput {
    pub enabled: bool,
    pub head_center_offset_ls: Vec3,
    pub head_radius: f32,
    pub neck_top_offset_ls: Vec3,
    pub neck_bottom_offset_ls: Vec3,
    pub neck_radius: f32,
    pub chest_top_offset_ls: Vec3,
    pub chest_bottom_offset_ls: Vec3,
    pub chest_radius: f32,
    pub surface_padding: f32,
    pub downward_pitch_limit_radians: f32,
}

impl Default for FirstPersonBodyBarrierInput {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: false,
            head_center_offset_ls: Vec3::ZERO,
            head_radius: 0.0,
            neck_top_offset_ls: Vec3::ZERO,
            neck_bottom_offset_ls: Vec3::ZERO,
            neck_radius: 0.0,
            chest_top_offset_ls: Vec3::ZERO,
            chest_bottom_offset_ls: Vec3::ZERO,
            chest_radius: 0.0,
            surface_padding: 0.0,
            downward_pitch_limit_radians: 55.0_f32.to_radians(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CameraRuntimeServiceConfig {
    pub runner: GameplayCameraRunnerKind,
    pub first_person_eye_height: f32,
    /// Optional stable render-cadence eye anchor supplied by the active avatar provider.
    pub first_person_anchor_ws: Option<Vec3>,
    /// Optional ADS camera position resolved from the rendered weapon's rear sight. Camera runtime
    /// blends toward it with the same aim alpha used for FOV/recoil, while orientation stays input-owned.
    pub first_person_ads_anchor_ws: Option<Vec3>,
    /// Optional render-cadence body rotation. FPP position follows this body frame while look yaw/pitch
    /// remain input-owned; this prevents mouse look from orbiting the camera around the skull.
    pub first_person_body_rotation_ws: Option<Quat>,
    /// Small body-forward clearance from the eye center. It is body-owned and view-independent.
    pub first_person_forward_clearance: f32,
    /// Project-authored body-relative free-look envelope.
    pub first_person_body_yaw_limit_radians: f32,
    /// Project-authored gameplay lens values. Camera runtime executes them but does not choose them.
    pub first_person_fov_y_radians: f32,
    pub first_person_ads_fov_y_radians: f32,
    pub first_person_near: f32,
    pub first_person_collision_enabled: bool,
    pub first_person_collision_probe_radius: f32,
    pub first_person_collision_padding: f32,
    pub first_person_grounded_eye_deadband_m: f32,
    pub first_person_grounded_eye_time_constant_seconds: f32,
    pub first_person_camera_recoil_share: f32,
    pub first_person_aim_response_hz: f32,
    pub near_clip_enabled: bool,
    pub near_clip_first_person_max_distance: f32,
    pub near_clip_third_person_min_distance: f32,
    pub near_clip_third_person_max_distance: f32,
    pub near_clip_pull_in_distance: f32,
    pub near_clip_probe_radius: f32,
    pub near_clip_release_time_seconds: f32,
    pub near_clip_hysteresis_m: f32,
    pub third_person_follow_fov_y_radians: f32,
    pub third_person_follow_offset_ls: Vec3,
    pub third_person_follow_focus_offset_ls: Vec3,
    pub third_person_follow_smooth_time: f32,
    pub third_person_follow_max_speed: f32,
    pub third_person_follow_zoom_min: f32,
    pub third_person_follow_zoom_max: f32,
    pub third_person_aim_fov_y_radians: f32,
    pub third_person_aim_offset_ls: Vec3,
    pub third_person_aim_focus_offset_ls: Vec3,
    pub third_person_aim_smooth_time: f32,
    pub third_person_aim_max_speed: f32,
    pub third_person_aim_zoom_min: f32,
    pub third_person_aim_zoom_max: f32,
    pub third_person_orbit_fov_y_radians: f32,
    pub third_person_orbit_offset_ls: Vec3,
    pub third_person_orbit_focus_offset_ls: Vec3,
    pub third_person_orbit_smooth_time: f32,
    pub third_person_orbit_max_speed: f32,
    pub third_person_orbit_zoom_min: f32,
    pub third_person_orbit_zoom_max: f32,
    pub third_person_orbit_look_sensitivity_radians_per_pixel: f32,
    pub third_person_orbit_pitch_min_radians: f32,
    pub third_person_orbit_pitch_max_radians: f32,
    pub third_person_collision_enabled: bool,
    pub third_person_collision_probe_radius: f32,
    pub third_person_collision_padding: f32,
    pub third_person_collision_min_distance: f32,
    pub third_person_collision_release_frequency_hz: f32,
    pub third_person_collision_release_damping_ratio: f32,
    pub third_person_collision_distance_hysteresis: f32,
    pub third_person_look_at_collision_blend: f32,
    pub third_person_look_at_response_hz: f32,
    pub third_person_look_at_max_error_fov_fraction: f32,
    pub third_person_catch_up_enabled: bool,
    pub third_person_catch_up_frequency_hz: f32,
    pub third_person_catch_up_damping_ratio: f32,
    pub third_person_catch_up_max_distance_m: f32,
    pub third_person_catch_up_settle_distance_m: f32,
    pub zoom_wheel_exponent_per_step: f32,
    pub orbit_drag_zoom_exponent_per_pixel: f32,
    pub zoom_smooth_time_seconds: f32,
    /// Semantic render-cadence FPP presentation input. It never changes physical eye/ballistic aim.
    pub first_person_presentation: FirstPersonPresentationInput,
    /// Local-owner presentation safety metadata. The camera consumes the authored downward pitch
    /// bound but does not move itself against the owner envelope; world collision remains separate.
    pub first_person_body_barrier: FirstPersonBodyBarrierInput,
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
            first_person_ads_anchor_ws: None,
            first_person_body_rotation_ws: None,
            first_person_forward_clearance: 0.07,
            first_person_body_yaw_limit_radians: 65.0_f32.to_radians(),
            first_person_fov_y_radians: 68.0_f32.to_radians(),
            first_person_ads_fov_y_radians: 45.0_f32.to_radians(),
            first_person_near: 0.045,
            first_person_collision_enabled: true,
            first_person_collision_probe_radius: 0.055,
            first_person_collision_padding: 0.012,
            first_person_grounded_eye_deadband_m: 0.010,
            first_person_grounded_eye_time_constant_seconds: 0.060,
            first_person_camera_recoil_share: 0.42,
            first_person_aim_response_hz: 18.0,
            near_clip_enabled: true,
            near_clip_first_person_max_distance: 0.09,
            near_clip_third_person_min_distance: 0.05,
            near_clip_third_person_max_distance: 0.28,
            near_clip_pull_in_distance: 0.018,
            near_clip_probe_radius: 0.010,
            near_clip_release_time_seconds: 0.08,
            near_clip_hysteresis_m: 0.0025,
            third_person_follow_fov_y_radians: 64.0_f32.to_radians(),
            third_person_follow_offset_ls: Vec3::new(0.35, 1.65, 4.5),
            third_person_follow_focus_offset_ls: Vec3::new(0.0, 0.95, 0.0),
            third_person_follow_smooth_time: 0.08,
            third_person_follow_max_speed: 0.0,
            third_person_follow_zoom_min: 1.35,
            third_person_follow_zoom_max: 9.0,
            third_person_aim_fov_y_radians: 54.0_f32.to_radians(),
            third_person_aim_offset_ls: Vec3::new(0.55, 1.55, 2.2),
            third_person_aim_focus_offset_ls: Vec3::new(0.0, 1.25, 0.0),
            third_person_aim_smooth_time: 0.035,
            third_person_aim_max_speed: 0.0,
            third_person_aim_zoom_min: 1.10,
            third_person_aim_zoom_max: 4.50,
            third_person_orbit_fov_y_radians: 60.0_f32.to_radians(),
            third_person_orbit_offset_ls: Vec3::new(0.0, 0.0, 4.8),
            third_person_orbit_focus_offset_ls: Vec3::new(0.0, 0.95, 0.0),
            third_person_orbit_smooth_time: 0.06,
            third_person_orbit_max_speed: 0.0,
            third_person_orbit_zoom_min: 1.35,
            third_person_orbit_zoom_max: 10.0,
            third_person_orbit_look_sensitivity_radians_per_pixel: 0.0028,
            third_person_orbit_pitch_min_radians: -70.0_f32.to_radians(),
            third_person_orbit_pitch_max_radians: 45.0_f32.to_radians(),
            third_person_collision_enabled: true,
            third_person_collision_probe_radius: 0.18,
            third_person_collision_padding: 0.08,
            third_person_collision_min_distance: 0.75,
            third_person_collision_release_frequency_hz: 1.6,
            third_person_collision_release_damping_ratio: 0.8,
            third_person_collision_distance_hysteresis: 0.005,
            third_person_look_at_collision_blend: 0.70,
            third_person_look_at_response_hz: 14.0,
            third_person_look_at_max_error_fov_fraction: 0.12,
            third_person_catch_up_enabled: true,
            third_person_catch_up_frequency_hz: 2.4,
            third_person_catch_up_damping_ratio: 1.0,
            third_person_catch_up_max_distance_m: 8.0,
            third_person_catch_up_settle_distance_m: 0.006,
            zoom_wheel_exponent_per_step: 0.16,
            orbit_drag_zoom_exponent_per_pixel: 0.008,
            zoom_smooth_time_seconds: 0.09,
            first_person_presentation: FirstPersonPresentationInput::default(),
            first_person_body_barrier: FirstPersonBodyBarrierInput::default(),
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
mod near_clip;
mod sync;

#[cfg(test)]
mod tests;
