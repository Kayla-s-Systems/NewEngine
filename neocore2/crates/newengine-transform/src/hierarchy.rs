#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
use newengine_transform_api::EntityHandle;
use newengine_transform_api::{Children, Parent, Transform, TransformDirty};

#[inline]
fn handle(id: EntityId) -> EntityHandle {
    EntityHandle::from(id)
}

#[inline]
fn resolve_transform_entity(world: &World, entity: EntityHandle) -> Option<EntityId> {
    world
        .query::<Transform>()
        .find(|(id, _)| id.stable_u64() == entity.stable_id)
        .map(|(id, _)| id)
}

/// Attaches `child` under `parent` and keeps `Children` lists consistent.
///
/// Notes:
/// - Does not automatically prevent cycles. Cycle handling is done in propagation.
/// - If `parent` doesn't exist, parent link is removed (child becomes root).
#[inline]
pub fn set_parent(world: &mut World, child: EntityId, parent: Option<EntityId>) -> bool {
    if !world.exists(child) {
        return false;
    }

    if parent == Some(child) {
        return false;
    }

    let child_handle = handle(child);

    // Remove from previous parent's children list.
    let prev_parent = world.get::<Parent>(child).map(|p| p.0);
    if let Some(pp) = prev_parent.and_then(|h| resolve_transform_entity(world, h)) {
        if let Some(ch) = world.get_mut::<Children>(pp) {
            if let Some(pos) = ch.0.iter().position(|&e| e == child_handle) {
                ch.0.swap_remove(pos);
                world.mark_changed::<Children>(pp);
            }
        }
    }

    // Update parent component.
    match parent {
        Some(p) if world.exists(p) => {
            let parent_handle = handle(p);
            let _ = world.insert(child, Parent(parent_handle));

            // Ensure parent has children list.
            if world.get::<Children>(p).is_none() {
                let _ = world.insert(p, Children::default());
            }

            if let Some(ch) = world.get_mut::<Children>(p) {
                if !ch.0.contains(&child_handle) {
                    ch.0.push(child_handle);
                    world.mark_changed::<Children>(p);
                }
            }
        }
        _ => {
            let _ = world.remove::<Parent>(child);
        }
    }

    // Parent topology affects world-space evaluation.
    // We mark the child dirty so runtimes can cheaply gate derived updates.
    let _ = world.insert(child, TransformDirty);

    true
}
