#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Mat4, Vec3};
use newengine_primitives::Primitive;
use newengine_transform::GlobalTransform;

use crate::gameplay::display_visible_in_mode;

use super::EditorRenderController;

pub(super) fn handle_picking(
    this: &mut EditorRenderController,
    scene: &newengine_scene::Scene,
    viewproj: Mat4,
    vp_w: u32,
    vp_h: u32,
) {
    let (pick_seq, pick_x, pick_y) = this.viewport_bridge.read_pick_request();
    if pick_seq == this.last_pick_seq {
        return;
    }
    this.last_pick_seq = pick_seq;

    let world = scene.world();
    let picked = pick_entity(viewproj, vp_w, vp_h, pick_x, pick_y, world);
    this.scene_bridge.set_selection(picked);
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

    let ray_o: Vec3 = near3;
    let mut ray_d: Vec3 = far3 - near3;
    let len2 = ray_d.length_squared();
    if len2 <= 1e-12 {
        return None;
    }
    ray_d *= len2.sqrt().recip();

    let mut best_t = f32::INFINITY;
    let mut best_e: Option<newengine_ecs::EntityId> = None;

    for (e, _prim, gt) in world.query2::<Primitive, GlobalTransform>() {
        if !display_visible_in_mode(world, e, false) {
            continue;
        }
        let m = gt.0;
        let center = Vec3::new(m.w_axis.x, m.w_axis.y, m.w_axis.z);

        let sx = Vec3::new(m.x_axis.x, m.x_axis.y, m.x_axis.z).length();
        let sy = Vec3::new(m.y_axis.x, m.y_axis.y, m.y_axis.z).length();
        let sz = Vec3::new(m.z_axis.x, m.z_axis.y, m.z_axis.z).length();
        let r = 0.8660254 * sx.max(sy).max(sz).max(1e-3);

        let oc = ray_o - center;
        let b = oc.dot(ray_d);
        let c = oc.length_squared() - r * r;
        let disc = b * b - c;
        if disc < 0.0 {
            continue;
        }
        let t = -b - disc.sqrt();
        if t > 0.0 && t < best_t {
            best_t = t;
            best_e = Some(e);
        }
    }

    best_e
}
