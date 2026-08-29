#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{CameraControlInput, CameraRig};

use crate::{CameraRigComp, Intent, IntentSink, OrbitCameraMotor};

#[inline]
fn sanitize_dt(dt: f32) -> Option<f32> {
    if dt.is_finite() && dt > 0.0 {
        Some(dt)
    } else {
        None
    }
}

/// Deterministic orbit-camera controller bridge.
///
/// Mutates only controller-local state and emits semantic intents for ECS updates.
#[inline]
pub fn run_orbit_camera_controller(
    entity: newengine_ecs::EntityId,
    mut motor: OrbitCameraMotor,
    mut rig: CameraRig,
    input: CameraControlInput,
    dt: f32,
    out: &mut impl IntentSink,
) {
    let Some(dt) = sanitize_dt(dt) else {
        return;
    };

    motor.controller.apply(&mut rig, input, dt);

    out.emit(Intent::SetCameraRig {
        entity,
        value: CameraRigComp(rig),
    });
    out.emit(Intent::SetOrbitCameraMotor {
        entity,
        value: motor,
    });
}
