use newengine_math::Vec3;

use crate::{Aabb, Sphere};

/// Converts an AABB into a conservative bounding sphere.
#[inline]
pub fn aabb_to_sphere(aabb: Aabb) -> Sphere {
    let center = aabb.center();
    let he = aabb.half_extents();
    let radius = he.length();
    Sphere { center, radius }
}

/// Converts a sphere into an AABB.
#[inline]
pub fn sphere_to_aabb(s: Sphere) -> Aabb {
    let r = Vec3::splat(s.radius);
    Aabb::new(s.center - r, s.center + r)
}
