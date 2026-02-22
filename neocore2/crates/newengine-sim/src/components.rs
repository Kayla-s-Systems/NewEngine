#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{CameraInput, CameraRig, OrbitController};
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
pub struct CameraInputComp(pub CameraInput);
