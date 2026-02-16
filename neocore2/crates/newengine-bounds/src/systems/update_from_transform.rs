use newengine_ecs::{QueryMut, World};

use crate::{sphere_to_aabb, Bounds};

/// Updates world-space bounds from `newengine_transform::Transform`.
///
/// Enabled by the `transform` crate feature.
pub fn update_bounds_from_transform_system(world: &mut World) {
    let mut q = QueryMut::<(&newengine_transform::Transform, &mut Bounds)>::new(world);
    for (t, b) in q.iter_mut() {
        let m = t.matrix();
        b.world_aabb = b.local_aabb.transformed(m);
        b.world_sphere.center = m.transform_point3(b.local_sphere.center);

        let sx = m.x_axis.truncate().length();
        let sy = m.y_axis.truncate().length();
        let sz = m.z_axis.truncate().length();
        let s = sx.max(sy).max(sz);
        b.world_sphere.radius = b.local_sphere.radius * s;

        if b.kind == crate::BoundsKind::Sphere {
            b.world_aabb = sphere_to_aabb(b.world_sphere);
        }
    }
}
