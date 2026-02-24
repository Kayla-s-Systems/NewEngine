use super::camera::GizmoCamera;
use super::math::{
    axis_end, dist_to_segment, plane_basis, world_radius_for_screen_plane, world_to_screen,
};
use super::types::{GizmoStyle, GizmoTransform};
use crate::GizmoAxis;
use egui::{Pos2, Rect};
use newengine_math::Quat;

pub(crate) fn pick_axis_lines(
    center: Pos2,
    x_end: Pos2,
    y_end: Pos2,
    z_end: Pos2,
    m: Pos2,
    r: f32,
) -> Option<GizmoAxis> {
    let mut best: Option<GizmoAxis> = None;
    let mut best_d = r;
    for (axis, end) in [
        (GizmoAxis::X, x_end),
        (GizmoAxis::Y, y_end),
        (GizmoAxis::Z, z_end),
    ] {
        let d = dist_to_segment(m, center, end);
        if d <= best_d {
            best_d = d;
            best = Some(axis);
        }
    }
    best
}

pub(crate) fn pick_axis_scale(
    center: Pos2,
    x_end: Pos2,
    y_end: Pos2,
    z_end: Pos2,
    m: Pos2,
    style: GizmoStyle,
) -> Option<GizmoAxis> {
    let cube = style.axis_cap_pt.max(6.0);
    let half = cube * 0.5;
    let r = style.pick_radius_pt.max(6.0);

    let mut best: Option<GizmoAxis> = None;
    let mut best_d = r;
    for (axis, end) in [
        (GizmoAxis::X, x_end),
        (GizmoAxis::Y, y_end),
        (GizmoAxis::Z, z_end),
    ] {
        let rect = Rect::from_min_max(
            Pos2::new(end.x - half, end.y - half),
            Pos2::new(end.x + half, end.y + half),
        );
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

pub(crate) fn pick_rotate_axis(
    camera: &impl GizmoCamera,
    rect: Rect,
    axes_rot: Quat,
    tr: GizmoTransform,
    mouse: Pos2,
    style: GizmoStyle,
) -> Option<GizmoAxis> {
    let Some((center, _pivot_ndc_z)) = world_to_screen(camera, rect, tr.pos) else {
        return None;
    };

    let mut best: Option<GizmoAxis> = None;
    let mut best_d = style.pick_radius_pt;

    // Match render: UE5-style full rings in world space with constant on-screen size.
    let r_pt = style.rotate_radius_pt.max(18.0);
    let segs = style.rotate_segments.max(48);
    for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
        let axis_world = (axes_rot * axis.vec3()).normalize_or_zero();
        if axis_world.dot(axis_world) < 1e-6 {
            continue;
        }
        let pts = build_axis_ring_world(camera, rect, tr.pos, axis_world, r_pt, segs);
        let d = dist_to_polyline(mouse, &pts);
        if d <= best_d {
            best_d = d;
            best = Some(axis);
        }
    }

    // UE-like outer screen-space ring (free rotate).
    // Only allow it when the cursor isn't already on one of the axis rings.
    if best.is_none() {
        let d = mouse.distance(center);
        let ring = style
            .screen_ring_radius_pt
            .max(style.rotate_radius_pt + 8.0);
        let ring_w = style.screen_ring_width_pt.max(2.0);
        let ring_band = (ring_w * 0.75 + style.pick_radius_pt).max(6.0);
        let ring_d = (d - ring).abs();
        if ring_d <= ring_band {
            best = Some(GizmoAxis::Screen);
        }
    }

    best
}

fn build_axis_ring_world(
    camera: &impl GizmoCamera,
    rect: Rect,
    pivot: newengine_math::Vec3,
    axis_n: newengine_math::Vec3,
    radius_pt: f32,
    segments: u32,
) -> Vec<Pos2> {
    let segs = segments.max(24) as usize;
    let mut out: Vec<Pos2> = Vec::with_capacity(segs + 1);

    // Must match render: world-space ring in the axis plane.
    let (u, v) = plane_basis(axis_n);
    let radius_w = world_radius_for_screen_plane(camera, rect, pivot, u, v, radius_pt);

    for i in 0..=segs {
        let t = (i as f32 / segs as f32) * core::f32::consts::TAU;
        let wp = pivot + (u * t.cos() + v * t.sin()) * radius_w;
        if let Some((sp, _)) = world_to_screen(camera, rect, wp) {
            out.push(sp);
        }
    }

    out
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
    axes_rot: Quat,
    tr: GizmoTransform,
    mouse: Pos2,
    style: GizmoStyle,
    scale_mode: bool,
) -> Option<GizmoAxis> {
    let Some((center, _)) = world_to_screen(camera, rect, tr.pos) else {
        return None;
    };

    let x_end = axis_end(
        camera,
        rect,
        tr.pos,
        axes_rot,
        GizmoAxis::X,
        center,
        style.axis_len_pt,
    );
    let y_end = axis_end(
        camera,
        rect,
        tr.pos,
        axes_rot,
        GizmoAxis::Y,
        center,
        style.axis_len_pt,
    );
    let z_end = axis_end(
        camera,
        rect,
        tr.pos,
        axes_rot,
        GizmoAxis::Z,
        center,
        style.axis_len_pt,
    );

    if scale_mode {
        pick_axis_scale(center, x_end, y_end, z_end, mouse, style)
    } else {
        pick_axis_lines(center, x_end, y_end, z_end, mouse, style.pick_radius_pt)
    }
}
