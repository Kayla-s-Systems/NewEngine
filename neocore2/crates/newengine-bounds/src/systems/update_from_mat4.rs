use glam::Mat4;

use newengine_ecs::{QueryMut, World};

use crate::{sphere_to_aabb, Bounds};

/// Updates world-space bounds from a `glam::Mat4` transform component.
///
/// Expected component layout:
/// - `Mat4`: world transform
/// - `Bounds`: local bounds + derived world bounds
pub fn update_bounds_from_mat4_system(world: &mut World) {
    let mut q = QueryMut::<(&Mat4, &mut Bounds)>::new(world);
    for (m, b) in q.iter_mut() {
        b.world_aabb = b.local_aabb.transformed(*m);
        b.world_sphere.center = m.transform_point3(b.local_sphere.center);

        // Conservative update: scale radius by maximal axis scale.
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
