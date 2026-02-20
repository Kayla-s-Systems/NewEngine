#![forbid(unsafe_op_in_unsafe_fn)]

// kept for type-level coherence in downstream systems
use newengine_ecs::{EntityId, World};
use newengine_math::{EulerRot, Quat, Vec3};
use newengine_scene::update_scene_world;
use newengine_transform::Transform;

use crate::{
    AngularVelocity, CameraInputComp, CameraRigComp, CharacterMotor, CommandBuffer, MotorInput,
    OrbitCameraMotor, SimFrame, Velocity,
};

/// Applies `MotorInput` to `CharacterMotor` and writes `Transform`/`Velocity` updates.
pub fn sys_character_motor(world: &World, frame: SimFrame, cmd: &mut CommandBuffer) {
    let dt = frame.dt;
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }

    let ids: Vec<EntityId> = world.query2_ids::<CharacterMotor, MotorInput>().collect();
    for id in ids {
        let Some(mut motor) = world.get::<CharacterMotor>(id).copied() else {
            continue;
        };
        let Some(input) = world.get::<MotorInput>(id).copied() else {
            continue;
        };

        let speed_mul = if input.speed_mul.is_finite() && input.speed_mul > 0.0 {
            input.speed_mul
        } else {
            1.0
        };

        if input.look_active {
            if input.look_delta.x.is_finite() {
                motor.yaw += input.look_delta.x * motor.look_sens;
            }
            if input.look_delta.y.is_finite() {
                motor.pitch += input.look_delta.y * motor.look_sens;
            }
        }
        motor.pitch = motor.pitch.clamp(-motor.pitch_limit, motor.pitch_limit);

        // Update orientation.
        if let Some(t) = world.get::<Transform>(id).copied() {
            let mut next = t;
            next.rotation = Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0);
            cmd.insert(id, next);
        }

        // Convert input axes to world velocity. Convention: forward is -Z.
        let local = Vec3::new(input.move_axis.x, input.move_axis.y, -input.move_axis.z);
        let len = local.length();
        let vel = if len > 1e-6 {
            let dir = local / len;
            let rot = world
                .get::<Transform>(id)
                .map(|t| t.rotation)
                .unwrap_or(Quat::IDENTITY);
            (rot * dir) * (motor.move_speed * speed_mul)
        } else {
            Vec3::ZERO
        };

        cmd.insert(id, Velocity(vel));
        cmd.insert(id, motor);
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

/// Updates derived scene data (world pose, bounds, cached scene bounds).
pub fn sys_scene_derived(world: &World, _frame: SimFrame, cmd: &mut CommandBuffer) {
    // `update_scene_world` mutates the world, so we execute it as a command.
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
