#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
use newengine_transform_api::Transform;

use crate::components::Name;
use crate::guid::ensure_entity_guid;

/// Spawns an entity with `Name` and `Transform`.
#[inline]
pub fn spawn_named(world: &mut World, name: impl Into<String>) -> EntityId {
    let e = world.spawn();
    let _ = ensure_entity_guid(world, e);
    let _ = world.insert(e, Name(name.into()));
    let _ = world.insert(e, Transform::default());
    e
}

/// Attempts to read an entity name.
#[inline]
pub fn name_or<'a>(world: &'a World, id: EntityId, fallback: &'a str) -> &'a str {
    world
        .get::<Name>(id)
        .map(|n| n.as_str())
        .unwrap_or(fallback)
}
