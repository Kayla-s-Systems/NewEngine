#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{CameraControlInput, CameraRig, OrbitController};
use newengine_ecs::EntityId;
use newengine_math::{Vec2, Vec3};

/// Linear velocity in world space (units/sec).
#[derive(Clone, Copy, Debug, Default)]
pub struct Velocity(pub Vec3);

/// Angular velocity in local space (rad/sec).
///
/// Conventions:
/// - x: pitch rate
/// - y: yaw rate
/// - z: roll rate
#[derive(Clone, Copy, Debug, Default)]
pub struct AngularVelocity(pub Vec3);

/// Input state for entity-local controllers (typically written by input/plugins).
#[derive(Clone, Copy, Debug, Default)]
pub struct MotorInput {
    /// Generic movement axes.
    /// Convention: x=right, y=up, z=forward.
    pub move_axis: Vec3,
    /// Look delta (mouse, stick).
    pub look_delta: Vec2,
    /// Whether look should affect yaw/pitch.
    pub look_active: bool,
    /// Additional speed multiplier (shift/sprint).
    pub speed_mul: f32,
    /// Mouse wheel / zoom delta.
    pub zoom_delta: f32,
    /// If true, the character body should turn toward the current view yaw.
    /// Used by aim/lock-on modes; free-look leaves the body independent.
    pub face_view: bool,
}

/// FPS / Free-fly style motor.
///
/// This is meant to be a small, deterministic building block.
/// Character collision/physics should live in a separate plugin/system.
#[derive(Clone, Copy, Debug)]
pub struct CharacterMotor {
    pub yaw: f32,
    pub pitch: f32,

    pub look_sens: f32,
    pub move_speed: f32,
    /// Maximum body yaw turn rate in radians/sec. View yaw remains unrestricted
    /// by this value; this only controls the visible/physical character facing.
    pub body_turn_speed: f32,

    pub pitch_limit: f32,

    /// Forward axis sign for converting local input to engine world convention.
    ///
    /// - +1.0: forward is +Z
    /// - -1.0: forward is -Z
    pub forward_sign_z: f32,
}

impl Default for CharacterMotor {
    #[inline]
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            look_sens: 0.0025,
            move_speed: 6.0,
            body_turn_speed: 8.5,
            pitch_limit: 1.54,
            forward_sign_z: -1.0,
        }
    }
}

/// ECS-bridge for `newengine-camera` orbit controller.
///
/// The camera crate is pure math; this component wires it to ECS entities.
#[derive(Clone, Copy, Debug)]
pub struct OrbitCameraMotor {
    pub controller: OrbitController,
}

impl Default for OrbitCameraMotor {
    #[inline]
    fn default() -> Self {
        Self {
            controller: OrbitController::default(),
        }
    }
}

/// Camera rig stored as a component.
#[derive(Clone, Copy, Debug, Default)]
pub struct CameraRigComp(pub CameraRig);

/// Camera input stored as a component (written by input/editor).
#[derive(Clone, Copy, Debug, Default)]
pub struct CameraControlInputComp(pub CameraControlInput);

/// Follow target controller for camera entities.
///
/// This is an ECS-level composition primitive:
/// - the controller stores *intent/params*
/// - the motor stores *state*
/// - the system performs the actual pose update deterministically
#[derive(Clone, Copy, Debug)]
pub struct FollowTargetCameraController {
    /// Target entity to follow.
    pub target: EntityId,
    /// Offset in the target's local space.
    pub offset_ls: Vec3,
    /// Optional additional rotation offset applied when `follow_rotation` is true.
    pub rot_offset: newengine_math::Quat,
    /// Stable look-at anchor in the target body's local space. This is intentionally
    /// independent from render/skeletal bone motion so animation cannot shake a gameplay camera.
    /// It is used only when `follow_rotation` is false.
    pub focus_offset_ls: Vec3,
    /// If true, camera rotation follows target rotation (plus `rot_offset`).
    /// If false, camera will look at the target.
    pub follow_rotation: bool,
    /// When true, the fixed-step simulation follow system must not author this camera.
    /// Gameplay cameras use render-cadence synchronization to avoid visible quantization/jitter.
    pub render_cadence_only: bool,
    /// Smoothing time constant (seconds). 0 => no smoothing.
    pub smooth_time: f32,
    /// Max speed clamp for position smoothing (units/sec). <=0 => unlimited.
    pub max_speed: f32,
}

impl Default for FollowTargetCameraController {
    #[inline]
    fn default() -> Self {
        Self {
            target: EntityId::default(),
            offset_ls: Vec3::new(0.0, 1.6, 4.0),
            rot_offset: newengine_math::Quat::IDENTITY,
            focus_offset_ls: Vec3::ZERO,
            follow_rotation: false,
            render_cadence_only: false,
            smooth_time: 0.12,
            max_speed: 0.0,
        }
    }
}

/// Motor state for [`FollowTargetCameraController`].
#[derive(Clone, Copy, Debug, Default)]
pub struct FollowTargetCameraMotor {
    /// Internal velocity used by the smooth damp step.
    pub vel_ws: Vec3,
}
