#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_bounds::{
    propagate_world_bounds, union_world_bounds, Aabb, BoundingSphere, WorldBounds,
};
use newengine_ecs::{EntityId, World};
use newengine_transform::{propagate_transforms, GlobalTransform};

/// Cached scene bounds (union of all `WorldBounds`).
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneBounds {
    pub aabb: Option<Aabb>,
    pub sphere: Option<BoundingSphere>,
}

/// Computes union world bounds for all entities that have `WorldBounds`.
///
/// This is renderer-agnostic and editor-agnostic.
#[inline]
pub fn scene_world_bounds(world: &World) -> Option<Aabb> {
    let entities = world.query::<WorldBounds>().map(|(id, _)| id);
    union_world_bounds(world, entities)
}

/// Updates derived scene state:
/// - propagates `Transform` -> `GlobalTransform`/`WorldPose`
/// - propagates `LocalBounds` -> `WorldBounds`
/// - caches the union bounds as a `SceneBounds` resource
#[inline]
pub fn update_scene_world(world: &mut World) {
    propagate_transforms(world);

    propagate_world_bounds(world, |w: &World, id: EntityId| {
        w.get::<GlobalTransform>(id).map(|g| g.0)
    });

    let aabb = scene_world_bounds(world);
    let sphere = aabb.map(|a| a.to_sphere());
    world.insert_resource(SceneBounds { aabb, sphere });
}

/// Computes union world bounds for the provided entities.
#[inline]
pub fn selection_world_bounds(
    world: &World,
    entities: impl Iterator<Item=EntityId>,
) -> Option<Aabb> {
    union_world_bounds(world, entities)
}