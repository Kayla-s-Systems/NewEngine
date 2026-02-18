#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_bounds::{Aabb, Bounds, BoundsKind, Sphere};
use newengine_ecs::{EntityId, World};
use newengine_math::collections_prelude::NeKey;
use newengine_transform::{propagate_transforms, GlobalTransform, Parent, Transform, TransformDirty};

/// Cached scene bounds (union of all `Bounds` world-space data).
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneBounds {
    pub aabb: Option<Aabb>,
    pub sphere: Option<Sphere>,
}

/// AAA cache for derived scene state.
///
/// We cache "last processed tick" per stage to avoid doing expensive work every frame.
#[derive(Clone, Copy, Debug)]
pub struct SceneDerivedCache {
    pub last_transform_tick: u64,
    pub last_bounds_tick: u64,
    pub last_union_tick: u64,
}

impl Default for SceneDerivedCache {
    #[inline]
    fn default() -> Self {
        Self {
            last_transform_tick: 0,
            last_bounds_tick: 0,
            last_union_tick: 0,
        }
    }
}

/// Reusable scratch buffers for bounds update and union computations.
#[derive(Default)]
struct SceneBoundsScratch {
    ids: Vec<EntityId>,
}

#[inline]
fn ensure_resources(world: &mut World) {
    if world.resource::<SceneDerivedCache>().is_none() {
        world.insert_resource(SceneDerivedCache::default());
    }
    if world.resource::<SceneBounds>().is_none() {
        world.insert_resource(SceneBounds::default());
    }
    if world.resource::<SceneBoundsScratch>().is_none() {
        world.insert_resource(SceneBoundsScratch::default());
    }
}

#[inline]
fn aabb_to_sphere(aabb: Aabb) -> Sphere {
    let center = aabb.center();
    let he = aabb.half_extents();
    Sphere {
        center,
        radius: he.length(),
    }
}

#[inline]
fn any_transform_inputs_dirty(world: &World, since_tick: u64) -> bool {
    if world.query::<TransformDirty>().next().is_some() {
        return true;
    }
    if world.query_changed::<Transform>(since_tick).next().is_some() {
        return true;
    }
    if world.query_added::<Transform>(since_tick).next().is_some() {
        return true;
    }
    if world.query_changed::<Parent>(since_tick).next().is_some() {
        return true;
    }
    if world.query_added::<Parent>(since_tick).next().is_some() {
        return true;
    }
    false
}

#[inline]
fn any_bounds_inputs_dirty(world: &World, since_tick: u64) -> bool {
    if world.query_changed::<GlobalTransform>(since_tick).next().is_some() {
        return true;
    }
    if world.query_added::<GlobalTransform>(since_tick).next().is_some() {
        return true;
    }
    if world.query_changed::<Bounds>(since_tick).next().is_some() {
        return true;
    }
    if world.query_added::<Bounds>(since_tick).next().is_some() {
        return true;
    }
    false
}

#[inline]
fn update_bounds_from_global_transform(world: &mut World) {
    // Move scratch out to avoid borrow conflicts with world queries/gets.
    let mut scratch = {
        let s = world
            .resource_mut::<SceneBoundsScratch>()
            .expect("SceneBoundsScratch must exist");
        core::mem::take(s)
    };

    scratch.ids.clear();
    scratch.ids.extend(world.query2_ids::<GlobalTransform, Bounds>());
    scratch.ids.sort_unstable_by_key(|id| id.data().as_ffi());
    scratch.ids.dedup();

    for id in scratch.ids.iter().copied() {
        let m = match world.get::<GlobalTransform>(id) {
            Some(g) => g.0,
            None => continue,
        };

        let src = match world.get::<Bounds>(id) {
            Some(v) => *v,
            None => continue,
        };

        let mut world_sphere = src.local_sphere;
        world_sphere.center = m.transform_point3(src.local_sphere.center);

        let sx = m.x_axis.truncate().length();
        let sy = m.y_axis.truncate().length();
        let sz = m.z_axis.truncate().length();
        let s = sx.max(sy).max(sz);
        world_sphere.radius = src.local_sphere.radius * s;

        let mut world_aabb = src.local_aabb.transformed(m);
        if src.kind == BoundsKind::Sphere {
            // Equivalent to newengine_bounds::sphere_to_aabb, but kept local to avoid using crate-private API.
            let r = newengine_math::Vec3::splat(world_sphere.radius);
            world_aabb = Aabb::new(world_sphere.center - r, world_sphere.center + r);
        }

        if let Some(dst) = world.get_mut::<Bounds>(id) {
            let changed = dst.world_aabb != world_aabb || dst.world_sphere != world_sphere;
            if changed {
                dst.world_aabb = world_aabb;
                dst.world_sphere = world_sphere;
                world.mark_changed::<Bounds>(id);
            }
        }
    }

    *world
        .resource_mut::<SceneBoundsScratch>()
        .expect("SceneBoundsScratch must exist") = scratch;
}

/// Computes union world AABB for all entities that have `Bounds`.
///
/// Deterministic and allocation-free.
#[inline]
pub fn scene_world_bounds(world: &World) -> Option<Aabb> {
    let mut it = world.query::<Bounds>();
    let (_id0, b0) = it.next()?;
    let mut acc = b0.world_aabb;
    for (_id, b) in it {
        acc = acc.union(&b.world_aabb);
    }
    Some(acc)
}

/// Computes union world AABB for the provided entities (selection).
///
/// Caller controls determinism via iterator ordering.
#[inline]
pub fn selection_world_bounds(world: &World, entities: impl Iterator<Item=EntityId>) -> Option<Aabb> {
    let mut it = entities.filter_map(|id| world.get::<Bounds>(id).map(|b| b.world_aabb));
    let first = it.next()?;
    let mut acc = first;
    for a in it {
        acc = acc.union(&a);
    }
    Some(acc)
}

/// Updates derived scene state (AAA):
/// - propagates `Transform` -> `GlobalTransform`/`WorldPose` (dirty-gated)
/// - updates `Bounds` world_* from `GlobalTransform` (dirty-gated)
/// - caches union bounds as `SceneBounds` (dirty-gated)
#[inline]
pub fn update_scene_world(world: &mut World) {
    ensure_resources(world);

    let tick_now = world.tick();

    // Stage 1: transforms.
    let mut ran_transforms = false;
    {
        let since = world
            .resource::<SceneDerivedCache>()
            .expect("SceneDerivedCache must exist")
            .last_transform_tick;

        if since == 0 || any_transform_inputs_dirty(world, since) {
            propagate_transforms(world);
            ran_transforms = true;

            world.resource_mut::<SceneDerivedCache>()
                .expect("SceneDerivedCache must exist")
                .last_transform_tick = tick_now;
        }
    }

    // Stage 2: bounds derived from GlobalTransform.
    let mut ran_bounds = false;
    {
        let since = world
            .resource::<SceneDerivedCache>()
            .expect("SceneDerivedCache must exist")
            .last_bounds_tick;

        let dirty = since == 0 || ran_transforms || any_bounds_inputs_dirty(world, since);
        if dirty {
            update_bounds_from_global_transform(world);
            ran_bounds = true;

            world.resource_mut::<SceneDerivedCache>()
                .expect("SceneDerivedCache must exist")
                .last_bounds_tick = tick_now;
        }
    }

    // Stage 3: cache union.
    {
        let since = world
            .resource::<SceneDerivedCache>()
            .expect("SceneDerivedCache must exist")
            .last_union_tick;

        let dirty = since == 0
            || ran_bounds
            || world.query_changed::<Bounds>(since).next().is_some()
            || world.query_added::<Bounds>(since).next().is_some();

        if dirty {
            let aabb = scene_world_bounds(world);
            let sphere = aabb.map(aabb_to_sphere);

            let sb = world.resource_mut::<SceneBounds>().expect("SceneBounds must exist");
            sb.aabb = aabb;
            sb.sphere = sphere;

            world.resource_mut::<SceneDerivedCache>()
                .expect("SceneDerivedCache must exist")
                .last_union_tick = tick_now;
        }
    }
}

/// Returns cached scene bounds.
#[inline]
pub fn scene_bounds_cached(world: &World) -> SceneBounds {
    world.resource::<SceneBounds>().copied().unwrap_or_default()
}
