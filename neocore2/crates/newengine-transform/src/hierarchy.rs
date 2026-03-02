#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};

/// Attaches `child` under `parent` and keeps `Children` lists consistent.
///
/// Notes:
/// - Does not automatically prevent cycles. Cycle handling is done in propagation.
/// - If `parent` doesn't exist, parent link is removed (child becomes root).
#[inline]
pub fn set_parent(world: &mut World, child: EntityId, parent: Option<EntityId>) -> bool {
    // Delegate to API-level helper to keep semantics unified across crates.
    newengine_transform_api::set_parent(world, child, parent)
}
