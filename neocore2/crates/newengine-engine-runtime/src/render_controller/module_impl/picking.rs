#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_bounds::Bounds;
use newengine_math::{Mat4, Vec3};
use newengine_primitives::Primitive;
use newengine_transform::GlobalTransform;

use crate::gameplay::display_visible_in_mode;

use super::RuntimeRenderController;

pub(super) fn handle_picking(
    this: &mut RuntimeRenderController,
    scene: &newengine_scene::Scene,
    viewproj: Mat4,
    vp_w: u32,
    vp_h: u32,
) {
    let (pick_seq, pick_x, pick_y) = this.bridges.viewport.read_pick_request();
    if pick_seq == this.frame.last_pick_seq {
        return;
    }
    this.frame.last_pick_seq = pick_seq;

    let world = scene.world();
    let picked = pick_entity(viewproj, vp_w, vp_h, pick_x, pick_y, world);
    // Rendering currently owns a scene read/write guard while this function runs.
    // Publishing through SceneBridge here would recursively acquire that lock.
    this.frame.pending_pick_selection = Some(picked);
}

#[inline]
fn pick_entity(
    viewproj: Mat4,
    vp_w: u32,
    vp_h: u32,
    x_px: f32,
    y_px: f32,
    world: &newengine_ecs::World,
) -> Option<newengine_ecs::EntityId> {
    if vp_w == 0 || vp_h == 0 {
        return None;
    }

    let inv = viewproj.inverse();
    let x = ((x_px + 0.5) / vp_w as f32) * 2.0 - 1.0;
    let y = 1.0 - ((y_px + 0.5) / vp_h as f32) * 2.0;
    let near = inv * newengine_math::Vec4::new(x, y, 0.0, 1.0);
    let far = inv * newengine_math::Vec4::new(x, y, 1.0, 1.0);
    let near3 = near.truncate() / near.w.max(1e-6);
    let far3 = far.truncate() / far.w.max(1e-6);

    let ray_o = near3;
    let mut ray_d = far3 - near3;
    let len2 = ray_d.length_squared();
    if len2 <= 1e-12 {
        return None;
    }
    ray_d *= len2.sqrt().recip();

    let mut best_t = f32::INFINITY;
    let mut best_e = None;

    for (entity, _primitive, global) in world.query2::<Primitive, GlobalTransform>() {
        if !display_visible_in_mode(world, entity, false) {
            continue;
        }

        let hit_t = world
            .get::<Bounds>(entity)
            .and_then(|bounds| {
                ray_aabb_intersection(ray_o, ray_d, bounds.world_aabb.min, bounds.world_aabb.max)
            })
            .or_else(|| fallback_transform_sphere_intersection(ray_o, ray_d, global));

        if let Some(t) = hit_t.filter(|t| *t > 0.0 && *t < best_t) {
            best_t = t;
            best_e = Some(entity);
        }
    }

    best_e
}

#[inline]
fn fallback_transform_sphere_intersection(
    ray_o: Vec3,
    ray_d: Vec3,
    global: &GlobalTransform,
) -> Option<f32> {
    let m = global.0;
    let center = Vec3::new(m.w_axis.x, m.w_axis.y, m.w_axis.z);
    let sx = Vec3::new(m.x_axis.x, m.x_axis.y, m.x_axis.z).length();
    let sy = Vec3::new(m.y_axis.x, m.y_axis.y, m.y_axis.z).length();
    let sz = Vec3::new(m.z_axis.x, m.z_axis.y, m.z_axis.z).length();
    let radius = 0.8660254 * sx.max(sy).max(sz).max(1e-3);
    let oc = ray_o - center;
    let b = oc.dot(ray_d);
    let c = oc.length_squared() - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let root = disc.sqrt();
    let near = -b - root;
    let far = -b + root;
    if near > 0.0 {
        Some(near)
    } else if far > 0.0 {
        Some(far)
    } else {
        None
    }
}

#[inline]
fn ray_aabb_intersection(ray_o: Vec3, ray_d: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let mut t_min = 0.0_f32;
    let mut t_max = f32::INFINITY;
    for (origin, direction, slab_min, slab_max) in [
        (ray_o.x, ray_d.x, min.x, max.x),
        (ray_o.y, ray_d.y, min.y, max.y),
        (ray_o.z, ray_d.z, min.z, max.z),
    ] {
        if direction.abs() <= 1e-7 {
            if origin < slab_min || origin > slab_max {
                return None;
            }
            continue;
        }
        let inv = direction.recip();
        let mut t1 = (slab_min - origin) * inv;
        let mut t2 = (slab_max - origin) * inv;
        if t1 > t2 {
            core::mem::swap(&mut t1, &mut t2);
        }
        t_min = t_min.max(t1);
        t_max = t_max.min(t2);
        if t_max < t_min {
            return None;
        }
    }
    (t_max > 0.0).then_some(if t_min > 0.0 { t_min } else { t_max })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_hits_aabb_from_front() {
        let t = ray_aabb_intersection(
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::splat(-1.0),
            Vec3::splat(1.0),
        );
        assert_eq!(t, Some(4.0));
    }

    #[test]
    fn parallel_ray_outside_aabb_misses() {
        assert!(ray_aabb_intersection(
            Vec3::new(2.0, 0.0, 5.0),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::splat(-1.0),
            Vec3::splat(1.0),
        )
        .is_none());
    }
}
