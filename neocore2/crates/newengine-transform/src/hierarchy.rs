#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
use newengine_transform_api::{Children, Parent, TransformDirty};

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

    // Remove from previous parent's children list.
    let prev_parent = world.get::<Parent>(child).map(|p| p.0);
    if let Some(pp) = prev_parent {
        if let Some(ch) = world.get_mut::<Children>(pp) {
            if let Some(pos) = ch.0.iter().position(|&e| e == child) {
                ch.0.swap_remove(pos);
                world.mark_changed::<Children>(pp);
            }
        }
    }

    // Update parent component.
    match parent {
        Some(p) if world.exists(p) => {
            let _ = world.insert(child, Parent(p));

            // Ensure parent has children list.
            if world.get::<Children>(p).is_none() {
                let _ = world.insert(p, Children::default());
            }

            if let Some(ch) = world.get_mut::<Children>(p) {
                if !ch.0.iter().any(|&e| e == child) {
                    ch.0.push(child);
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
