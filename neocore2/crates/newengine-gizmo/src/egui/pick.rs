use super::camera::GizmoCamera;
use super::math::{axis_end, dist_to_segment, plane_basis, screen_ray, world_radius_for_screen, world_to_screen};
use super::types::{GizmoStyle, GizmoTransform};
use crate::GizmoAxis;
use egui::{Pos2, Rect};

pub(crate) fn pick_axis_lines(center: Pos2, x_end: Pos2, y_end: Pos2, z_end: Pos2, m: Pos2, r: f32) -> Option<GizmoAxis> {
    let mut best: Option<GizmoAxis> = None;
    let mut best_d = r;
    for (axis, end) in [(GizmoAxis::X, x_end), (GizmoAxis::Y, y_end), (GizmoAxis::Z, z_end)] {
        let d = dist_to_segment(m, center, end);
        if d <= best_d {
            best_d = d;
            best = Some(axis);
        }
    }
    best
}

pub(crate) fn pick_axis_scale(center: Pos2, x_end: Pos2, y_end: Pos2, z_end: Pos2, m: Pos2, style: GizmoStyle) -> Option<GizmoAxis> {
    let cube = style.arrow_size_pt.max(6.0);
    let half = cube * 0.5;
    let r = style.pick_radius_pt.max(6.0);

    let mut best: Option<GizmoAxis> = None;
    let mut best_d = r;
    for (axis, end) in [(GizmoAxis::X, x_end), (GizmoAxis::Y, y_end), (GizmoAxis::Z, z_end)] {
        let rect = Rect::from_min_max(Pos2::new(end.x - half, end.y - half), Pos2::new(end.x + half, end.y + half));
        if rect.contains(m) {
            return Some(axis);
        }

        let d = dist_to_segment(m, center, end);
        if d <= best_d {
            best_d = d;
            best = Some(axis);
        }
    }
    best
}

pub(crate) fn pick_rotate_axis(camera: &impl GizmoCamera, rect: Rect, tr: GizmoTransform, mouse: Pos2, style: GizmoStyle) -> Option<GizmoAxis> {
    let Some((center, _)) = world_to_screen(camera, rect, tr.pos) else {
        return None;
    };

    let view_dir = {
        let (_ro, rd) = screen_ray(camera, rect, center);
        rd
    };

    let mut best: Option<GizmoAxis> = None;
    let mut best_d = style.pick_radius_pt;

    for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
        let axis_world = (tr.rot * axis.vec3()).normalize_or_zero();
        let (u, v) = plane_basis(axis_world);
        let radius_w = world_radius_for_screen(camera, rect, tr.pos, u, style.rotate_radius_pt);
        let d_plane = (view_dir - axis_world * view_dir.dot(axis_world)).normalize_or_zero();

        let segs = style.rotate_segments.max(16) as usize;
        let mut pts: Vec<Pos2> = Vec::with_capacity(segs + 1);
        for i in 0..=segs {
            let t = (i as f32 / segs as f32) * core::f32::consts::TAU;
            let r = u * t.cos() + v * t.sin();
            if r.dot(d_plane) <= 0.0 {
                continue;
            }
            let wp = tr.pos + r * radius_w;
            if let Some((sp, _)) = world_to_screen(camera, rect, wp) {
                pts.push(sp);
            }
        }

        let d = dist_to_polyline(mouse, &pts);
        if d <= best_d {
            best_d = d;
            best = Some(axis);
        }
    }

    best
}

fn dist_to_polyline(p: Pos2, pts: &[Pos2]) -> f32 {
    if pts.len() < 2 {
        return f32::INFINITY;
    }
    let mut best = f32::INFINITY;
    for w in pts.windows(2) {
        best = best.min(dist_to_segment(p, w[0], w[1]));
    }
    best
}

pub(crate) fn pick_non_rotate_axis(
    camera: &impl GizmoCamera,
    rect: Rect,
    tr: GizmoTransform,
    mouse: Pos2,
    style: GizmoStyle,
    scale_mode: bool,
) -> Option<GizmoAxis> {
    let Some((center, _)) = world_to_screen(camera, rect, tr.pos) else {
        return None;
    };

    let x_end = axis_end(camera, rect, tr.pos, tr.rot, GizmoAxis::X, center, style.axis_len_pt);
    let y_end = axis_end(camera, rect, tr.pos, tr.rot, GizmoAxis::Y, center, style.axis_len_pt);
    let z_end = axis_end(camera, rect, tr.pos, tr.rot, GizmoAxis::Z, center, style.axis_len_pt);

    if scale_mode {
        pick_axis_scale(center, x_end, y_end, z_end, mouse, style)
    } else {
        pick_axis_lines(center, x_end, y_end, z_end, mouse, style.pick_radius_pt)
    }
}
