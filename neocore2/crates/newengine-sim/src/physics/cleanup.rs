#![forbid(unsafe_op_in_unsafe_fn)]

use hashbrown::HashSet;
use joltc_sys as sys;
use newengine_ecs::World;

use super::jolt::stable_entity_key;
use super::types::{PhysicsBody, PhysicsCtx};

pub fn physics_cleanup_bodies(world: &mut World, _frame: super::super::SimFrame) {
    if world.resource::<PhysicsCtx>().is_none() {
        return;
    }

    let mut live: HashSet<u64> = HashSet::new();
    for (id, _) in world.query::<PhysicsBody>() {
        live.insert(stable_entity_key(id));
    }

    let mut to_remove: Vec<(u64, sys::JPC_BodyID)> = Vec::new();
    {
        let ctx = world.resource::<PhysicsCtx>().expect("physics ctx must exist");
        if ctx.bodies.is_empty() {
            return;
        }
        for (&k, &body_id) in ctx.bodies.iter() {
            if !live.contains(&k) {
                to_remove.push((k, body_id));
            }
        }
    }

    if to_remove.is_empty() {
        return;
    }
    to_remove.sort_unstable_by_key(|(k, _)| *k);

    {
        let ctx = world.resource_mut::<PhysicsCtx>().expect("physics ctx must exist");
        let mut pw_guard = ctx.world.lock().ok();
        let Some(pw) = pw_guard.as_deref_mut() else { return; };

        let system = pw.system_raw();
        let body_iface = unsafe { sys::JPC_PhysicsSystem_GetBodyInterface(system) };
        if body_iface.is_null() {
            return;
        }

        for (k, body_id) in to_remove {
            ctx.bodies.remove(&k);
            unsafe {
                sys::JPC_BodyInterface_RemoveBody(body_iface, body_id);
                sys::JPC_BodyInterface_DestroyBody(body_iface, body_id);
            }
        }
    }
}