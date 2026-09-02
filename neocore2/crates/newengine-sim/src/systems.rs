#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
// kept for type-level coherence in downstream systems
use newengine_math::prelude::NeKey;
use newengine_math::{EulerRot, Quat};
use newengine_transform::{read_entity_world_pose_local_chain, Transform};

use crate::{
    run_character_motor_controller, run_follow_camera_controller, run_orbit_camera_controller,
    AngularVelocity, CameraControlInputComp, CameraRigComp, CharacterFacingTurnStepRequest,
    CharacterMotor, CommandBuffer, ControllerCtx, ControllerIntentQueue,
    FollowTargetCameraController, FollowTargetCameraMotor, Intent, IntentBuffer,
    IntentCommandBufferExt, MotorInput, OrbitCameraMotor, SimFrame, TransformCommandBufferExt,
    Velocity,
};

#[inline]
fn sort_ids(ids: &mut [EntityId]) {
    ids.sort_unstable_by_key(|id| id.data().as_ffi());
}

/// Applies `MotorInput` to `CharacterMotor` and emits semantic intents.
pub fn sys_character_motor(world: &World, frame: SimFrame, cmd: &mut CommandBuffer) {
    let dt = frame.dt;
    if !(dt.is_finite() && dt > 0.0) {
        return;
    }

    let mut ids: Vec<EntityId> = world.query2_ids::<CharacterMotor, MotorInput>().collect();
    sort_ids(&mut ids);

    let ctx = ControllerCtx::new(world, frame);
    let mut intents = IntentBuffer::new();

    for id in ids {
        let Some(motor) = world.get::<CharacterMotor>(id).copied() else {
            continue;
        };
        let Some(input) = world.get::<MotorInput>(id).copied() else {
            continue;
        };

        run_character_motor_controller(id, &ctx, motor, input, &mut intents);
        if world.get::<CharacterFacingTurnStepRequest>(id).is_some() {
            // Consume through the semantic intent queue. Controllers are read-only over ECS storage;
            // ApplyIntents owns the one-shot component removal after the facing step is authored.
            intents.push(Intent::ConsumeCharacterFacingTurnStepRequest { entity: id });
        }
    }

    if !intents.is_empty() {
        cmd.enqueue_intents(intents);
    }
}

/// Applies orbit controller input and emits semantic intents.
pub fn sys_orbit_camera(world: &World, frame: SimFrame, cmd: &mut CommandBuffer) {
    let dt = frame.dt;
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }

    let mut ids: Vec<EntityId> = world
        .query2_ids::<OrbitCameraMotor, CameraRigComp>()
        .collect();
    sort_ids(&mut ids);

    let mut intents = IntentBuffer::new();

    for id in ids {
        let Some(motor) = world.get::<OrbitCameraMotor>(id).copied() else {
            continue;
        };
        let Some(rig) = world.get::<CameraRigComp>(id).copied() else {
            continue;
        };

        let input = world
            .get::<CameraControlInputComp>(id)
            .map(|c| c.0)
            .unwrap_or_default();

        run_orbit_camera_controller(id, motor, rig.0, input, dt, &mut intents);
    }

    if !intents.is_empty() {
        cmd.enqueue_intents(intents);
    }
}

/// Follow target controller for camera entities.
///
/// This system emits rig/motor intents only. `Transform` is updated later in the dedicated apply
/// stage via `sys_camera_rig_to_transform`.
pub fn sys_camera_follow(world: &World, frame: SimFrame, cmd: &mut CommandBuffer) {
    let dt = frame.dt;
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }

    let mut ids: Vec<EntityId> = world
        .query2_ids::<FollowTargetCameraController, CameraRigComp>()
        .collect();
    sort_ids(&mut ids);

    let ctx = ControllerCtx::new(world, frame);
    let mut intents = IntentBuffer::new();

    for id in ids {
        let Some(ctrl) = world.get::<FollowTargetCameraController>(id).copied() else {
            continue;
        };
        if ctrl.render_cadence_only {
            continue;
        }
        let Some(rig) = world.get::<CameraRigComp>(id).copied() else {
            continue;
        };
        let motor = world
            .get::<FollowTargetCameraMotor>(id)
            .copied()
            .unwrap_or_default();

        run_follow_camera_controller(id, &ctx, ctrl, rig, motor, &mut intents);
    }

    if !intents.is_empty() {
        cmd.enqueue_intents(intents);
    }
}

/// Applies queued controller intents in a single, ordered stage.
pub fn sys_apply_controller_intents(world: &World, _frame: SimFrame, cmd: &mut CommandBuffer) {
    let Some(queue) = world.resource::<ControllerIntentQueue>() else {
        return;
    };
    if queue.is_empty() {
        return;
    }

    // Apply directly from the immutable queue view. Controller systems already deferred all
    // world mutation into `CommandBuffer`, so cloning the tiny intent vector here only adds an
    // allocator hit to every fixed tick and can inherit unrelated allocator contention.
    for intent in queue.iter() {
        intent.apply_to(cmd);
    }

    cmd.clear_controller_intents();
}

/// Copies `CameraRigComp` (world pose) into local `Transform` with parent-aware conversion.
pub fn sys_camera_rig_to_transform(world: &World, _frame: SimFrame, cmd: &mut CommandBuffer) {
    let mut ids: Vec<EntityId> = world.query2_ids::<CameraRigComp, Transform>().collect();
    sort_ids(&mut ids);

    for id in ids {
        let Some(rig) = world.get::<CameraRigComp>(id).copied() else {
            continue;
        };

        // Preserve local scale, but author position/rotation in world space.
        cmd.transform_set_world_pose(id, rig.0.position, rig.0.rotation);
    }
}

/// Integrates velocities into transforms.
///
/// Semantics:
/// - `Velocity` is world-space linear velocity (units/sec)
/// - `AngularVelocity` is local-space angular velocity (rad/sec)
///
/// For parented entities, translation is applied in world space and converted back to local space
/// deterministically via the current local transform chain.
pub fn sys_integrate_velocities(world: &World, frame: SimFrame, cmd: &mut CommandBuffer) {
    let dt = frame.dt;
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }

    // Collect candidates deterministically: (Transform+Velocity) U (Transform+AngularVelocity)
    let mut ids: Vec<EntityId> = world.query2_ids::<Transform, Velocity>().collect();
    ids.extend(world.query2_ids::<Transform, AngularVelocity>());
    sort_ids(&mut ids);
    ids.dedup();

    for id in ids {
        let Some((pos_ws, rot_ws)) = read_entity_world_pose_local_chain(world, id) else {
            continue;
        };

        let mut next_pos_ws = pos_ws;
        let mut next_rot_ws = rot_ws;

        if let Some(v) = world.get::<Velocity>(id).copied() {
            let d = v.0 * dt;
            if d.is_finite() {
                next_pos_ws += d;
            }
        }

        if let Some(w) = world.get::<AngularVelocity>(id).copied() {
            let d = w.0 * dt;
            if d.is_finite() && d.length_squared() > 1e-12 {
                let dq = Quat::from_euler(EulerRot::YXZ, d.y, d.x, d.z);
                next_rot_ws = (next_rot_ws * dq).normalize_or_identity();
            }
        }

        // Commit as a world-pose write (parent-aware), preserving local scale.
        cmd.transform_set_world_pose(id, next_pos_ws, next_rot_ws);
    }
}
