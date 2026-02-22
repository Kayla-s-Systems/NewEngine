#![forbid(unsafe_op_in_unsafe_fn)]

// kept for type-level coherence in downstream systems
use newengine_ecs::{EntityId, World};
use newengine_math::{EulerRot, Quat};
use newengine_scene::update_scene_world;
use newengine_transform::Transform;

use crate::{
    step_character_motor,
    AngularVelocity, CameraInputComp, CameraRigComp, CharacterMotor, CommandBuffer, MotorInput,
    OrbitCameraMotor, SimFrame, Velocity,
};

/// Applies `MotorInput` to `CharacterMotor` and writes `Transform`/`Velocity` updates.
pub fn sys_character_motor(world: &World, frame: SimFrame, cmd: &mut CommandBuffer) {
    let dt = frame.dt;
    if !(dt.is_finite() && dt > 0.0) {
        return;
    }

    let ids: Vec<EntityId> = world.query2_ids::<CharacterMotor, MotorInput>().collect();
    for id in ids {
        let Some(motor) = world.get::<CharacterMotor>(id).copied() else {
            continue;
        };
        let Some(input) = world.get::<MotorInput>(id).copied() else {
            continue;
        };

        let current = world.get::<Transform>(id).copied();
        let current_rot = current.map(|t| t.rotation).unwrap_or(Quat::IDENTITY);

        let Some(step) = step_character_motor(motor, input, current_rot, dt) else {
            continue;
        };

        if let Some(t) = current {
            let mut next = t;
            next.rotation = step.rotation;
            cmd.insert(id, next);
        }

        cmd.insert(id, Velocity(step.velocity_ws));
        cmd.insert(id, step.motor);
    }
}


/// Applies orbit controller input to `CameraRigComp`.
pub fn sys_orbit_camera(world: &World, frame: SimFrame, cmd: &mut CommandBuffer) {
    let dt = frame.dt;
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }

    let ids: Vec<EntityId> = world.query2_ids::<OrbitCameraMotor, CameraRigComp>().collect();
    for id in ids {
        let Some(mut motor) = world.get::<OrbitCameraMotor>(id).copied() else {
            continue;
        };
        let Some(mut rig) = world.get::<CameraRigComp>(id).copied() else {
            continue;
        };

        // Gather input. If missing, apply with defaults (no movement).
        let input = world
            .get::<CameraInputComp>(id)
            .map(|c| c.0)
            .unwrap_or_default();

        motor.controller.apply(&mut rig.0, input, dt);

        cmd.insert(id, rig);
        cmd.insert(id, motor);
    }
}

/// Copies `CameraRigComp` to `Transform`.
pub fn sys_camera_rig_to_transform(world: &World, _frame: SimFrame, cmd: &mut CommandBuffer) {
    let ids: Vec<EntityId> = world.query2_ids::<CameraRigComp, Transform>().collect();
    for id in ids {
        let Some(rig) = world.get::<CameraRigComp>(id).copied() else {
            continue;
        };
        let Some(t) = world.get::<Transform>(id).copied() else {
            continue;
        };
        let mut next = t;
        next.position = rig.0.position;
        next.rotation = rig.0.rotation;
        cmd.insert(id, next);
    }
}

/// Integrates velocities into transforms.
pub fn sys_integrate_velocities(world: &World, frame: SimFrame, cmd: &mut CommandBuffer) {
    let dt = frame.dt;
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }

    // Translation.
    let ids: Vec<EntityId> = world.query2_ids::<Transform, Velocity>().collect();
    for id in ids {
        let Some(t) = world.get::<Transform>(id).copied() else {
            continue;
        };
        let Some(v) = world.get::<Velocity>(id).copied() else {
            continue;
        };
        let mut next = t;
        next.position += v.0 * dt;
        cmd.insert(id, next);
    }

    // Rotation.
    let ids: Vec<EntityId> = world.query2_ids::<Transform, AngularVelocity>().collect();
    for id in ids {
        let Some(t) = world.get::<Transform>(id).copied() else {
            continue;
        };
        let Some(w) = world.get::<AngularVelocity>(id).copied() else {
            continue;
        };
        let d = w.0 * dt;
        if !(d.is_finite() && d.length_squared() > 1e-12) {
            continue;
        }
        let dq = Quat::from_euler(EulerRot::YXZ, d.y, d.x, d.z);
        let mut next = t;
        next.rotation = (next.rotation * dq).normalize_or_identity();
        cmd.insert(id, next);
    }
}

/// Updates derived scene data (_world pose, bounds, cached scene bounds).
pub fn sys_scene_derived(_world: &World, _frame: SimFrame, cmd: &mut CommandBuffer) {
    // `update_scene_world` mutates the _world, so we execute it as a command.
    // This preserves deterministic ordering while allowing parallelism in earlier batches.
    cmd.push(Box::new(SceneDerivedCmd {
        _phantom: core::marker::PhantomData,
    }));
}

struct SceneDerivedCmd {
    _phantom: core::marker::PhantomData<()>,
}

impl crate::Command for SceneDerivedCmd {
    #[inline]
    fn apply(self: Box<Self>, world: &mut World) {
        update_scene_world(world);
    }
}
