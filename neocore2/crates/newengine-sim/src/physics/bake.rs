#![forbid(unsafe_op_in_unsafe_fn)]

use joltc_sys as sys;
use newengine_ecs::{EntityId, World};
use newengine_transform::Transform;

use super::jolt::{jolt_create_body, stable_entity_key};
use super::types::{Collider, PhysicsBody, PhysicsCtx, RigidBody};

pub fn physics_bake_bodies(world: &mut World, _frame: super::super::SimFrame) {
    if world.resource::<PhysicsCtx>().is_none() {
        return;
    }

    let mut todo: Vec<(u64, EntityId, Transform, RigidBody, Collider)> = Vec::new();
    for id in world.query2_ids::<RigidBody, Collider>() {
        if world.get::<PhysicsBody>(id).is_some() {
            continue;
        }
        let Some(t) = world.get::<Transform>(id).copied() else { continue; };
        let Some(rb) = world.get::<RigidBody>(id).copied() else { continue; };
        let Some(col) = world.get::<Collider>(id).copied() else { continue; };
        todo.push((stable_entity_key(id), id, t, rb, col));
    }
    todo.sort_unstable_by_key(|(k, _, _, _, _)| *k);

    let mut created: Vec<(u64, EntityId, sys::JPC_BodyID)> = Vec::new();
    {
        let ctx = world.resource_mut::<PhysicsCtx>().expect("physics ctx must exist");
        let mut pw_guard = ctx.world.lock().ok();
        let Some(pw) = pw_guard.as_deref_mut() else { return; };

        for (k, id, t, rb, col) in &todo {
            let Some(body_id) = jolt_create_body(pw, *id, t, *rb, *col) else { continue; };
            created.push((*k, *id, body_id));
        }
    }

    if created.is_empty() {
        return;
    }

    for &(_, id, body_id) in &created {
        let _ = world.insert(id, PhysicsBody { id: body_id });
    }

    {
        let ctx = world.resource_mut::<PhysicsCtx>().expect("physics ctx must exist");
        for &(k, _id, body_id) in &created {
            ctx.bodies.insert(k, body_id);
        }
    }
}