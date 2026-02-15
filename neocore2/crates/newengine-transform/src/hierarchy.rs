#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};

use crate::{Children, Parent, TransformDirty};

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

    let old_parent = world.get::<Parent>(child).map(|p| p.0);

    if let Some(op) = old_parent {
        if let Some(ch) = world.get_mut::<Children>(op) {
            ch.0.retain(|&e| e != child);
        }
    }

    match parent {
        Some(p) => {
            if !world.exists(p) {
                let _ = world.remove::<Parent>(child);
                let _ = world.insert(child, TransformDirty);
                return true;
            }

            let _ = world.insert(child, Parent(p));

            if world.get::<Children>(p).is_none() {
                let _ = world.insert(p, Children::default());
            }
            if let Some(ch) = world.get_mut::<Children>(p) {
                if !ch.0.iter().any(|&e| e == child) {
                    ch.0.push(child);
                }
            }
        }
        None => {
            let _ = world.remove::<Parent>(child);
        }
    }

    let _ = world.insert(child, TransformDirty);
    true
}