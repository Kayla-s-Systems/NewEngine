use glam::Vec3;

use crate::{Aabb, Sphere};

#[inline]
pub(crate) fn aabb_to_sphere(aabb: Aabb) -> Sphere {
    let center = aabb.center();
    let he = aabb.half_extents();
    let radius = he.length();
    Sphere { center, radius }
}

#[inline]
pub(crate) fn sphere_to_aabb(s: Sphere) -> Aabb {
    let r = Vec3::splat(s.radius);
    Aabb::new(s.center - r, s.center + r)
}
