#![forbid(unsafe_op_in_unsafe_fn)]

use glam::{Mat4, Vec3};
use newengine_ecs::{EntityId, World};

/// Axis-aligned bounding box in local or world space.
#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    #[inline]
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    #[inline]
    pub fn center(self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    #[inline]
    pub fn extents(self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    #[inline]
    pub fn union(self, other: Aabb) -> Aabb {
        Aabb {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    #[inline]
    pub fn is_valid(self) -> bool {
        self.min.x.is_finite()
            && self.min.y.is_finite()
            && self.min.z.is_finite()
            && self.max.x.is_finite()
            && self.max.y.is_finite()
            && self.max.z.is_finite()
            && (self.max.x >= self.min.x)
            && (self.max.y >= self.min.y)
            && (self.max.z >= self.min.z)
    }

    #[inline]
    pub fn to_sphere(self) -> BoundingSphere {
        let c = self.center();
        let r = (self.max - c).length();
        BoundingSphere {
            center: c,
            radius: r.max(0.000_001),
        }
    }
}

/// Bounding sphere.
#[derive(Clone, Copy, Debug)]
pub struct BoundingSphere {
    pub center: Vec3,
    pub radius: f32,
}

impl BoundingSphere {
    #[inline]
    pub fn is_valid(self) -> bool {
        self.center.x.is_finite()
            && self.center.y.is_finite()
            && self.center.z.is_finite()
            && self.radius.is_finite()
            && self.radius > 0.0
    }
}

/// Local-space bounds (typically mesh bounds, collider bounds, etc).
#[derive(Clone, Copy, Debug)]
pub struct LocalBounds(pub Aabb);

/// World-space bounds derived from LocalBounds * GlobalTransform.
#[derive(Clone, Copy, Debug)]
pub struct WorldBounds(pub Aabb);

#[derive(Clone, Copy, Debug, Default)]
pub struct BoundsDirty;

/// Transforms an AABB by a matrix (robust, via 8 corners).
#[inline]
pub fn transform_aabb(m: Mat4, a: Aabb) -> Aabb {
    let corners = [
        Vec3::new(a.min.x, a.min.y, a.min.z),
        Vec3::new(a.max.x, a.min.y, a.min.z),
        Vec3::new(a.min.x, a.max.y, a.min.z),
        Vec3::new(a.max.x, a.max.y, a.min.z),
        Vec3::new(a.min.x, a.min.y, a.max.z),
        Vec3::new(a.max.x, a.min.y, a.max.z),
        Vec3::new(a.min.x, a.max.y, a.max.z),
        Vec3::new(a.max.x, a.max.y, a.max.z),
    ];

    let mut mn = Vec3::splat(f32::INFINITY);
    let mut mx = Vec3::splat(f32::NEG_INFINITY);

    for c in corners {
        let p = (m * c.extend(1.0)).truncate();
        mn = mn.min(p);
        mx = mx.max(p);
    }

    Aabb::new(mn, mx)
}

/// Computes `WorldBounds` for all entities that have `LocalBounds` and `GlobalTransform`.
///
/// This is runtime-safe and editor-safe (same semantics).
///
/// Requirements:
/// - `newengine-transform::propagate_transforms()` must have produced `GlobalTransform`.
pub fn propagate_world_bounds(
    world: &mut World,
    mut get_global: impl FnMut(&World, EntityId) -> Option<Mat4>,
) {
    // Ensure WorldBounds exists.
    {
        let ids: Vec<EntityId> = world.query::<LocalBounds>().map(|(id, _)| id).collect();
        for id in ids {
            if world.get::<WorldBounds>(id).is_none() {
                let _ = world.insert(id, WorldBounds(Aabb::new(Vec3::ZERO, Vec3::ZERO)));
            }
        }
    }

    // Snapshot (immutable).
    let mut items: Vec<(EntityId, Aabb, Mat4)> = Vec::new();
    for (id, lb) in world.query::<LocalBounds>() {
        if let Some(g) = get_global(world, id) {
            items.push((id, lb.0, g));
        }
    }

    // Write-back.
    for (id, local, g) in items {
        let wb = transform_aabb(g, local);
        if let Some(w) = world.get_mut::<WorldBounds>(id) {
            w.0 = wb;
        }
        let _ = world.remove::<BoundsDirty>(id);
    }
}

/// Returns union world bounds for a set of entities.
#[inline]
pub fn union_world_bounds(world: &World, entities: impl Iterator<Item=EntityId>) -> Option<Aabb> {
    let mut out: Option<Aabb> = None;
    for e in entities {
        let b = world.get::<WorldBounds>(e)?.0;
        if !b.is_valid() {
            continue;
        }
        out = Some(match out {
            None => b,
            Some(acc) => acc.union(b),
        });
    }
    out
}