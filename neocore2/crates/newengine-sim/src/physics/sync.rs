#![forbid(unsafe_op_in_unsafe_fn)]

use glam::{Quat, Vec3};
use joltc_sys as sys;
use newengine_ecs::{EntityId, World};
use newengine_transform::Transform;

use super::jolt::stable_entity_key;
use super::types::{PhysicsBody, PhysicsCtx, PhysicsPose, PhysicsStepState, RigidBody, RigidBodyKind};

pub fn physics_sync_transforms(world: &mut World, _frame: super::super::SimFrame) {
    if world.resource::<PhysicsCtx>().is_none() {
        return;
    }

    let alpha = world
        .resource::<PhysicsStepState>()
        .map(|s| s.alpha)
        .unwrap_or(1.0);

    let mut dyn_read: Vec<(u64, EntityId, sys::JPC_BodyID)> = Vec::new();
    for (id, pb) in world.query::<PhysicsBody>() {
        let Some(rb) = world.get::<RigidBody>(id).copied() else { continue; };
        if rb.kind != RigidBodyKind::Dynamic {
            continue;
        }
        dyn_read.push((stable_entity_key(id), id, pb.id));
    }
    dyn_read.sort_unstable_by_key(|(k, _, _)| *k);

    let mut dyn_out: Vec<(EntityId, Vec3, Quat)> = Vec::new();
    {
        let ctx = world.resource_mut::<PhysicsCtx>().expect("physics ctx must exist");
        let mut pw_guard = ctx.world.lock().ok();
        let Some(pw) = pw_guard.as_deref_mut() else { return; };

        let system = pw.system_raw();
        let body_iface = unsafe { sys::JPC_PhysicsSystem_GetBodyInterface(system) };
        if body_iface.is_null() {
            return;
        }

        for (_, id, body_id) in dyn_read {
            let mut pos = sys::JPC_RVec3 { x: 0.0, y: 0.0, z: 0.0, _w: 0.0 };
            let mut rot = sys::JPC_Quat { x: 0.0, y: 0.0, z: 0.0, w: 1.0 };

            unsafe { sys::JPC_BodyInterface_GetPositionAndRotation(body_iface, body_id, &mut pos, &mut rot) };

            dyn_out.push((id, Vec3::new(pos.x, pos.y, pos.z), Quat::from_xyzw(rot.x, rot.y, rot.z, rot.w)));
        }
    }

    for (id, p, r) in dyn_out {
        let pose = match world.get::<PhysicsPose>(id).copied() {
            Some(mut pose) => {
                pose.prev_pos = pose.curr_pos;
                pose.prev_rot = pose.curr_rot;
                pose.curr_pos = p;
                pose.curr_rot = r;
                pose
            }
            None => PhysicsPose { prev_pos: p, prev_rot: r, curr_pos: p, curr_rot: r },
        };

        let _ = world.insert(id, pose);

        if let Some(t) = world.get_mut::<Transform>(id) {
            t.position = pose.prev_pos.lerp(pose.curr_pos, alpha);
            t.rotation = pose.prev_rot.slerp(pose.curr_rot, alpha).normalize();
        }
    }
}