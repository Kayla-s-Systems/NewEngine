#![forbid(unsafe_op_in_unsafe_fn)]

use glam::Mat4;
use newengine_ecs::{EntityId, World};
use slotmap::Key;

use crate::{sphere_to_aabb, Bounds, BoundsKind};

/// Reusable scratch buffers to avoid per-frame allocations.
#[derive(Default)]
struct BoundsUpdateFromMat4Scratch {
    ids: Vec<EntityId>,
}

#[inline]
fn ensure_scratch(world: &mut World) {
    if world.resource::<BoundsUpdateFromMat4Scratch>().is_none() {
        world.insert_resource(BoundsUpdateFromMat4Scratch::default());
    }
}

/// Updates `Bounds` for entities that have both `Mat4` and `Bounds`.
///
/// AAA contract:
/// - deterministic iteration (stable entity id order)
/// - no per-frame heap churn (scratch is a resource)
/// - mutations are tracked (only mark changed when derived data differs)
#[inline]
pub fn update_bounds_from_mat4_system(world: &mut World) {
    ensure_scratch(world);

    // Move scratch out to avoid borrow conflicts with world queries/gets.
    let mut scratch = {
        let s = world
            .resource_mut::<BoundsUpdateFromMat4Scratch>()
            .expect("BoundsUpdateFromMat4Scratch must exist");
        core::mem::take(s)
    };

    scratch.ids.clear();
    scratch.ids.extend(world.query2_ids::<Mat4, Bounds>());
    scratch.ids.sort_unstable_by_key(|id| id.data().as_ffi());
    scratch.ids.dedup();

    for id in scratch.ids.iter().copied() {
        let m = match world.get::<Mat4>(id) {
            Some(v) => *v,
            None => continue,
        };

        // Read current bounds (immutable) to compute derived values without holding a mutable borrow.
        let src = match world.get::<Bounds>(id) {
            Some(v) => *v,
            None => continue,
        };

        // Derive world sphere.
        let mut world_sphere = src.local_sphere;
        world_sphere.center = m.transform_point3(src.local_sphere.center);

        let sx = m.x_axis.truncate().length();
        let sy = m.y_axis.truncate().length();
        let sz = m.z_axis.truncate().length();
        let s = sx.max(sy).max(sz);
        world_sphere.radius = src.local_sphere.radius * s;

        // Derive world aabb.
        let mut world_aabb = src.local_aabb.transformed(m);
        if src.kind == BoundsKind::Sphere {
            world_aabb = sphere_to_aabb(world_sphere);
        }

        // Write back only if derived values changed.
        if let Some(dst) = world.get_mut::<Bounds>(id) {
            let changed = dst.world_aabb != world_aabb || dst.world_sphere != world_sphere;
            if changed {
                dst.world_aabb = world_aabb;
                dst.world_sphere = world_sphere;
                world.mark_changed::<Bounds>(id);
            }
        }
    }

    // Put scratch back (preserve capacities).
    *world
        .resource_mut::<BoundsUpdateFromMat4Scratch>()
        .expect("BoundsUpdateFromMat4Scratch must exist") = scratch;
}
