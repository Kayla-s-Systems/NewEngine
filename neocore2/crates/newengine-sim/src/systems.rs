#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
// kept for type-level coherence in downstream systems
use newengine_math::prelude::NeKey;
use newengine_math::{EulerRot, Quat};
use newengine_scene::update_scene_world;
use newengine_transform_api::{read_entity_world_pose_local_chain, Transform};

use crate::{
    step_character_motor, step_follow_camera, AngularVelocity, CameraInputComp, CameraRigComp,
    CharacterMotor, CommandBuffer, FollowTargetCameraController, FollowTargetCameraMotor,
    MotorInput, OrbitCameraMotor, SimFrame, TransformCommandBufferExt,
    Velocity,
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

        if current.is_some() {
            cmd.transform_set_local_rotation(id, step.rotation);
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

    let ids: Vec<EntityId> = world
        .query2_ids::<OrbitCameraMotor, CameraRigComp>()
        .collect();
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

/// Follow target controller for camera entities.
///
/// This system updates `CameraRigComp` (world pose) based on the target entity transform chain.
/// The resulting rig is then copied into `Transform` by `sys_camera_rig_to_transform`.
pub fn sys_camera_follow(world: &World, frame: SimFrame, cmd: &mut CommandBuffer) {
    let dt = frame.dt;
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }

    let ids: Vec<EntityId> = world
        .query2_ids::<FollowTargetCameraController, CameraRigComp>()
        .collect();

    for id in ids {
        let Some(ctrl) = world.get::<FollowTargetCameraController>(id).copied() else {
            continue;
        };
        let Some(mut rig) = world.get::<CameraRigComp>(id).copied() else {
            continue;
        };

        // Target must exist and have a transform.
        let Some((target_pos, target_rot)) = read_entity_world_pose_local_chain(world, ctrl.target) else {
            continue;
        };

        let motor = world
            .get::<FollowTargetCameraMotor>(id)
            .copied()
            .unwrap_or_default();

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
            dt,
        ) else {
            continue;
        };

        rig.0.position = step.next_pos;
        rig.0.rotation = step.next_rot;

        cmd.insert(id, rig);
        cmd.insert(
            id,
            FollowTargetCameraMotor {
                vel_ws: step.next_vel,
            },
        );
    }
}


/// Copies `CameraRigComp` (world pose) into local `Transform` with parent-aware conversion.
pub fn sys_camera_rig_to_transform(world: &World, _frame: SimFrame, cmd: &mut CommandBuffer) {
    let ids: Vec<EntityId> = world.query2_ids::<CameraRigComp, Transform>().collect();
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
    ids.sort_unstable_by_key(|id| id.data().as_ffi());
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
                next_pos_ws = next_pos_ws + d;
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
