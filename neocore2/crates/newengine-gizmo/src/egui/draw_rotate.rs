use super::camera::GizmoCamera;
use super::draw_axis::axis_color;
use super::math::{plane_basis, rotation_angle_on_plane, screen_ray, world_radius_for_screen, world_to_screen};
use super::types::{DragState, GizmoStyle, GizmoTransform};
use crate::{GizmoAxis, GizmoMode};
use egui::{Color32, Painter, Pos2, Rect, Stroke};

pub(crate) fn draw_rotate_gizmo(
    p: &Painter,
    ctx: &egui::Context,
    rect: Rect,
    camera: &impl GizmoCamera,
    tr: GizmoTransform,
    hovered: Option<GizmoAxis>,
    active: Option<GizmoAxis>,
    drag: Option<DragState>,
    style: GizmoStyle,
    center: Pos2,
) {
    let view_dir = {
        let (_ro, rd) = screen_ray(camera, rect, center);
        rd
    };

    for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
        let axis_world = (tr.rot * axis.vec3()).normalize_or_zero();
        let (u, v) = plane_basis(axis_world);

        let color = axis_color(axis, hovered, active, style.highlight_mul);
        draw_rotate_ring_front_half(p, camera, rect, tr.pos, axis_world, u, v, view_dir, style, color);

        if active == Some(axis) {
            if let Some(d) = drag {
                if d.axis == axis && d.mode == GizmoMode::Rotate {
                    let mouse = ctx.input(|i| i.pointer.interact_pos()).unwrap_or(center);
                    let a1 = rotation_angle_on_plane(camera, rect, tr.pos, axis_world, d.plane_u, d.plane_v, mouse);
                    let fill = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), style.rotate_fill_alpha);
                    let outline = Stroke::new(style.rotate_width_pt, color);
                    draw_rotate_wedge(
                        p,
                        camera,
                        rect,
                        tr.pos,
                        axis_world,
                        d.plane_u,
                        d.plane_v,
                        d.start_angle,
                        a1,
                        view_dir,
                        style,
                        fill,
                        outline,
                    );
                }
            }
        }
    }
}

fn draw_rotate_ring_front_half(
    p: &Painter,
    camera: &impl GizmoCamera,
    rect: Rect,
    pivot: newengine_math::Vec3,
    n: newengine_math::Vec3,
    u: newengine_math::Vec3,
    v: newengine_math::Vec3,
    view_dir: newengine_math::Vec3,
    style: GizmoStyle,
    color: Color32,
) {
    let radius_w = world_radius_for_screen(camera, rect, pivot, u, style.rotate_radius_pt);
    let d_plane = (view_dir - n * view_dir.dot(n)).normalize_or_zero();

    let segs = style.rotate_segments.max(16) as usize;
    let mut points: Vec<Pos2> = Vec::with_capacity(segs + 1);

    for i in 0..=segs {
        let t = (i as f32 / segs as f32) * core::f32::consts::TAU;
        let r = u * t.cos() + v * t.sin();

        if r.dot(d_plane) > 0.0 {
            let wp = pivot + r * radius_w;
            if let Some((sp, _)) = world_to_screen(camera, rect, wp) {
                points.push(sp);
            }
        } else if points.len() >= 2 {
            let stroke = Stroke::new(style.rotate_width_pt, color);
            p.add(egui::Shape::line(points.clone(), stroke));
            points.clear();
        }
    }

    if points.len() >= 2 {
        let stroke = Stroke::new(style.rotate_width_pt, color);
        p.add(egui::Shape::line(points, stroke));
    }
}

fn draw_rotate_wedge(
    p: &Painter,
    camera: &impl GizmoCamera,
    rect: Rect,
    pivot: newengine_math::Vec3,
    n: newengine_math::Vec3,
    u: newengine_math::Vec3,
    v: newengine_math::Vec3,
    a0: f32,
    a1: f32,
    view_dir: newengine_math::Vec3,
    style: GizmoStyle,
    fill: Color32,
    outline: Stroke,
) {
    let radius_w = world_radius_for_screen(camera, rect, pivot, u, style.rotate_radius_pt);
    let d_plane = (view_dir - n * view_dir.dot(n)).normalize_or_zero();

    let mut da = a1 - a0;
    while da > core::f32::consts::PI {
        da -= core::f32::consts::TAU;
    }
    while da < -core::f32::consts::PI {
        da += core::f32::consts::TAU;
    }

    let steps = ((style.rotate_segments as f32) * (da.abs() / core::f32::consts::TAU)).ceil().max(12.0) as usize;
    let Some((c2, _)) = world_to_screen(camera, rect, pivot) else {
        return;
    };

    let mut poly: Vec<Pos2> = Vec::with_capacity(steps + 2);
    poly.push(c2);

    for i in 0..=steps {
        let t = a0 + da * (i as f32 / steps as f32);
        let r = u * t.cos() + v * t.sin();
        if r.dot(d_plane) <= 0.0 {
            continue;
        }
        let wp = pivot + r * radius_w;
        if let Some((sp, _)) = world_to_screen(camera, rect, wp) {
            poly.push(sp);
        }
    }

    if poly.len() >= 3 {
        p.add(egui::Shape::convex_polygon(poly.clone(), fill, Stroke::NONE));
        p.add(egui::Shape::line(poly[1..].to_vec(), outline));
        p.line_segment([poly[0], poly[1]], outline);
        p.line_segment([poly[0], *poly.last().unwrap()], outline);
    }
}
