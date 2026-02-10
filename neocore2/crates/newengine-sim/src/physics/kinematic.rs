#![forbid(unsafe_op_in_unsafe_fn)]

use glam::Vec3;
use joltc_sys as sys;
use newengine_ecs::World;
use newengine_physics_jolt::PhysicsWorld;
use newengine_transform::Transform;

use super::jolt::{jpc_quat, jpc_rvec3, stable_entity_key};
use super::types::{PhysicsBody, RigidBody, RigidBodyKind};

#[inline]
pub fn gather_kinematic_targets(world: &World) -> Vec<(u64, sys::JPC_BodyID, Transform)> {
    let mut out: Vec<(u64, sys::JPC_BodyID, Transform)> = Vec::new();

    for (id, pb) in world.query::<PhysicsBody>() {
        let Some(rb) = world.get::<RigidBody>(id).copied() else { continue; };
        if rb.kind != RigidBodyKind::Kinematic {
            continue;
        }
        let Some(t) = world.get::<Transform>(id).copied() else { continue; };
        out.push((stable_entity_key(id), pb.id, t));
    }

    out.sort_unstable_by_key(|(k, _, _)| *k);
    out
}

#[inline]
pub fn apply_kinematic_targets_locked(
    pw: &mut PhysicsWorld,
    targets: &[(u64, sys::JPC_BodyID, Transform)],
) {
    if targets.is_empty() {
        return;
    }

    let system = pw.system_raw();
    let body_iface = unsafe { sys::JPC_PhysicsSystem_GetBodyInterface(system) };
    if body_iface.is_null() {
        return;
    }

    for &(_, body_id, t) in targets {
        let pos = jpc_rvec3(t.position);
        let rot = jpc_quat(t.rotation);
        unsafe {
            sys::JPC_BodyInterface_SetPositionAndRotation(
                body_iface,
                body_id,
                pos,
                rot,
                sys::JPC_ACTIVATION_DONT_ACTIVATE,
            );
        }
    }
}

#[inline]
pub fn _unused(_: Vec3) {}