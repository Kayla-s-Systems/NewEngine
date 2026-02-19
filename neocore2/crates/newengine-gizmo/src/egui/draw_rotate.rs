use super::camera::GizmoCamera;
use super::draw_axis::axis_color;
use super::math::{plane_basis, rotation_angle_on_plane, screen_ray, world_radius_for_screen, world_radius_for_screen_plane, world_to_screen};
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
    axes_rot: newengine_math::Quat,
) {
    let view_dir = {
        let (_ro, rd) = screen_ray(camera, rect, center);
        rd
    };

    // UE-like outer screen-space ring (free rotate around view axis).
    draw_screen_ring(p, ctx, rect, camera, tr, hovered, active, drag, style, center);

    // UE5-style rotation widget: full rings in world space.
    // Constant on-screen size is achieved by computing a world-space radius per ring.
    // Front half is solid, back half is dashed & faded (depth cue).
    draw_rotate_axis_rings(p, rect, camera, tr, hovered, active, style, axes_rot);

    // Plane-based active feedback (drives actual transform).
    for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
        if active != Some(axis) {
            continue;
        }
        let Some(d) = drag else {
            continue;
        };
        if d.axis != axis || d.mode != GizmoMode::Rotate {
            continue;
        }

        let axis_world = (axes_rot * axis.vec3()).normalize_or_zero();
        let color = axis_color(axis, hovered, active, style.highlight_mul);
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

fn draw_rotate_axis_rings(
    p: &Painter,
    rect: Rect,
    camera: &impl GizmoCamera,
    tr: GizmoTransform,
    hovered: Option<GizmoAxis>,
    active: Option<GizmoAxis>,
    style: GizmoStyle,
    axes_rot: newengine_math::Quat,
) {
    let Some((c2, c_ndc_z)) = world_to_screen(camera, rect, tr.pos) else {
        return;
    };

    let r_pt = style.rotate_radius_pt.max(18.0);
    let segs = style.rotate_segments.max(48);

    // UE-like dark outline for readability.
    let outline = Color32::from_rgba_unmultiplied(0, 0, 0, 190);

    for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
        let axis_world = (axes_rot * axis.vec3()).normalize_or_zero();
        if axis_world.dot(axis_world) < 1e-6 {
            continue;
        }

        let hot = hovered == Some(axis) || active == Some(axis);

        let col = axis_color(axis, hovered, active, style.highlight_mul);
        let w_base = style.rotate_width_pt.max(2.0) * if hot { 1.25 } else { 1.0 };
        let w_front = (w_base + style.rotate_front_width_add_pt).max(2.0);
        let w_back = (w_base * style.rotate_back_width_mul).max(1.5);

        let back = Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), style.rotate_back_alpha);

        let ring = build_axis_ring_world(camera, rect, tr.pos, c_ndc_z, axis_world, r_pt, segs);
        if ring.len() < 3 {
            continue;
        }

        // Back (dashed, faint) then front (solid) to match UE depth cue.
        draw_ring_with_depth_cue(p, &ring, outline, col, back, w_front, w_back, hot, style);
    }
}

#[derive(Clone, Copy)]
struct RingPt {
    p: Pos2,
    front: bool,
}

fn build_axis_ring_world(
    camera: &impl GizmoCamera,
    rect: Rect,
    pivot: newengine_math::Vec3,
    pivot_ndc_z: f32,
    axis_n: newengine_math::Vec3,
    radius_pt: f32,
    segments: u32,
) -> Vec<RingPt> {
    let segs = segments.max(24) as usize;
    let mut out: Vec<RingPt> = Vec::with_capacity(segs + 1);

    // UE5 behavior: rings are actual world-space circles in the rotation plane.
    // We compute a world-space radius that yields ~constant on-screen size.
    let (u, v) = plane_basis(axis_n);
    let radius_w = world_radius_for_screen_plane(camera, rect, pivot, u, v, radius_pt);

    for i in 0..=segs {
        let t = (i as f32 / segs as f32) * core::f32::consts::TAU;
        let wp = pivot + (u * t.cos() + v * t.sin()) * radius_w;
        if let Some((sp, ndc_z)) = world_to_screen(camera, rect, wp) {
            let front = ndc_z < pivot_ndc_z;
            out.push(RingPt { p: sp, front });
        }
    }

    out
}

fn draw_ring_with_depth_cue(
    p: &Painter,
    pts: &[RingPt],
    outline: Color32,
    front: Color32,
    back: Color32,
    w_front: f32,
    w_back: f32,
    hot: bool,
    style: GizmoStyle,
) {
    if pts.len() < 2 {
        return;
    }

    // Split into contiguous segments by front/back classification.
    let mut cur: Vec<Pos2> = Vec::with_capacity(pts.len());
    let mut cur_front = pts[0].front;
    cur.push(pts[0].p);

    let mut flush = |p: &Painter, cur: &mut Vec<Pos2>, is_front: bool| {
        if cur.len() < 2 {
            cur.clear();
            return;
        }

        let w = if is_front { w_front } else { w_back };
        let ow = (w + 2.0).max(w + 1.0);

        // UE5-ish glow: wide translucent pass under the colored stroke.
        let glow_w = (w + style.rotate_glow_width_add_pt + if hot { style.rotate_hot_glow_width_add_pt } else { 0.0 }).max(w + 2.0);
        let mut glow_a = style.rotate_glow_alpha as i32;
        if hot {
            glow_a += style.rotate_hot_glow_alpha_add as i32;
        }
        let glow_a = glow_a.clamp(0, 255) as u8;
        let glow_col = if is_front {
            Color32::from_rgba_unmultiplied(front.r(), front.g(), front.b(), glow_a)
        } else {
            Color32::from_rgba_unmultiplied(back.r(), back.g(), back.b(), (glow_a as u16 * 2 / 3) as u8)
        };

        if is_front {
            p.add(egui::Shape::line(cur.clone(), Stroke::new(glow_w, glow_col)));
            p.add(egui::Shape::line(cur.clone(), Stroke::new(ow, outline)));
            p.add(egui::Shape::line(cur.clone(), Stroke::new(w, front)));
        } else {
            // Dashed back half.
            draw_dashed_polyline(p, cur, Stroke::new(glow_w, glow_col), style.rotate_back_dash_pt, style.rotate_back_gap_pt);
            draw_dashed_polyline(p, cur, Stroke::new(ow, outline), style.rotate_back_dash_pt, style.rotate_back_gap_pt);
            draw_dashed_polyline(p, cur, Stroke::new(w, back), style.rotate_back_dash_pt, style.rotate_back_gap_pt);
        }
        cur.clear();
    };

    for w2 in pts.windows(2) {
        let a = w2[0];
        let b = w2[1];
        if b.front != cur_front {
            cur.push(b.p);
            flush(p, &mut cur, cur_front);
            cur_front = b.front;
            cur.push(b.p);
        } else {
            cur.push(b.p);
        }
    }

    flush(p, &mut cur, cur_front);
}

fn draw_dashed_polyline(p: &Painter, pts: &[Pos2], stroke: Stroke, dash: f32, gap: f32) {
    if pts.len() < 2 {
        return;
    }
    let mut carry = 0.0_f32;
    let period = (dash + gap).max(1.0);

    for seg in pts.windows(2) {
        let a = seg[0];
        let b = seg[1];
        let ab = b - a;
        let len = ab.length();
        if len <= 1e-3 {
            continue;
        }
        let dir = ab / len;
        let mut t = 0.0;
        if carry > 0.0 {
            t += carry.min(len);
            carry = 0.0;
        }

        while t < len {
            let phase = (t % period) / period;
            let in_dash = phase < (dash / period);
            let next = ((t / period).floor() + 1.0) * period;
            let t2 = next.min(len);

            if in_dash {
                let p0 = a + dir * t;
                let p1 = a + dir * t2;
                p.line_segment([p0, p1], stroke);
            }
            t = t2;
        }

        // Preserve phase between segments.
        let rem = len % period;
        carry = if rem > 0.0 { rem } else { 0.0 };
    }
}

fn draw_screen_ring(
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
    let ring_r = style.screen_ring_radius_pt.max(style.rotate_radius_pt + 8.0);
    let base = axis_color(GizmoAxis::Screen, hovered, active, style.highlight_mul);
    let w = if hovered == Some(GizmoAxis::Screen) || active == Some(GizmoAxis::Screen) {
        style.screen_ring_width_pt * 1.35
    } else {
        style.screen_ring_width_pt
    };

    // UE5: screen ring should not be always visible.
    // Show it only on hover/active/drag to avoid visual clutter and pick conflicts.
    let show = hovered == Some(GizmoAxis::Screen) || active == Some(GizmoAxis::Screen) || drag.is_some();
    if !show {
        let _ = (camera, rect, tr);
        return;
    }

    // Outer ring.
    p.circle_stroke(center, ring_r, Stroke::new(w, base));

    // Active drag feedback: screen-space arc on the ring.
    if active == Some(GizmoAxis::Screen) {
        if let Some(d) = drag {
            if d.axis == GizmoAxis::Screen && d.mode == GizmoMode::Rotate {
                let mouse = ctx.input(|i| i.pointer.interact_pos()).unwrap_or(center);
                let a1 = (mouse.y - center.y).atan2(mouse.x - center.x);
                let mut da = a1 - d.start_angle;
                while da > core::f32::consts::PI {
                    da -= core::f32::consts::TAU;
                }
                while da < -core::f32::consts::PI {
                    da += core::f32::consts::TAU;
                }

                let steps = ((style.rotate_segments as f32) * (da.abs() / core::f32::consts::TAU)).ceil().max(24.0) as usize;
                let mut pts: Vec<Pos2> = Vec::with_capacity(steps + 1);
                for i in 0..=steps {
                    let t = d.start_angle + da * (i as f32 / steps as f32);
                    pts.push(center + egui::vec2(t.cos() * ring_r, t.sin() * ring_r));
                }
                if pts.len() >= 2 {
                    let stroke = Stroke::new(w * 1.6, base);
                    p.add(egui::Shape::line(pts, stroke));
                }

                // Optional subtle inner hint: a thin dashed-ish arc inside the ring.
                // Kept deterministic and cheap (no real dashing).
                let inner_r = ring_r - (w * 1.25).max(4.0);
                let hint_steps = (steps / 2).max(18);
                let mut hint: Vec<Pos2> = Vec::with_capacity(hint_steps + 1);
                for i in 0..=hint_steps {
                    let t = d.start_angle + da * (i as f32 / hint_steps as f32);
                    hint.push(center + egui::vec2(t.cos() * inner_r, t.sin() * inner_r));
                }
                if hint.len() >= 2 {
                    let hint_col = Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), 90);
                    p.add(egui::Shape::line(hint, Stroke::new((w * 0.45).max(1.5), hint_col)));
                }
            }
        }
    }

    // Ensure pivot is valid on screen (avoid unused warnings if camera param is dropped later).
    let _ = (camera, rect, tr);
}

#[allow(dead_code)]
fn draw_rotate_ring_half(
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
    front: bool,
    radius_add_pt: f32,
    width_mul: f32,
) {
    let radius_w = world_radius_for_screen(camera, rect, pivot, u, style.rotate_radius_pt + radius_add_pt);
    // Project view direction onto the rotation plane. If the camera looks almost exactly along the
    // rotation axis, the projection degenerates and the arc orientation becomes unstable.
    // In that case, fall back to a stable in-plane direction (u).
    let mut d_plane = view_dir - n * view_dir.dot(n);
    if d_plane.dot(d_plane) < 1e-6 {
        d_plane = u;
    } else {
        d_plane = d_plane.normalize_or_zero();
    }

    // Runtime/AAA style: draw short thick arcs (like common in-game gizmos), not full rings.
    // The arc is centered on the camera-facing direction on the ring.
    let segs = style.rotate_segments.max(24) as usize;
    let half_span = (style.rotate_arc_deg.clamp(15.0, 175.0)).to_radians() * 0.5;
    let phi = d_plane.dot(v).atan2(d_plane.dot(u));
    let center_t = if front { phi } else { phi + core::f32::consts::PI };
    let t0 = center_t - half_span;
    let t1 = center_t + half_span;

    let mut pts: Vec<Pos2> = Vec::with_capacity(segs + 1);
    for i in 0..=segs {
        let t = t0 + (t1 - t0) * (i as f32 / segs as f32);
        let r = u * t.cos() + v * t.sin();
        let wp = pivot + r * radius_w;
        if let Some((sp, _)) = world_to_screen(camera, rect, wp) {
            pts.push(sp);
        }
    }

    if pts.len() >= 2 {
        let stroke = Stroke::new(style.rotate_width_pt * width_mul, color);
        p.add(egui::Shape::line(pts, stroke));
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

    // Same degeneracy handling as for ring halves.
    let mut d_plane = view_dir - n * view_dir.dot(n);
    if d_plane.dot(d_plane) < 1e-6 {
        d_plane = u;
    } else {
        d_plane = d_plane.normalize_or_zero();
    }

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

    // UE/runtime style: active rotation feedback is a thick arc, not a filled wedge.
    // Drawing a wedge with front-half culling causes visible breaks when the arc crosses the back.
    let mut pts: Vec<Pos2> = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = a0 + da * (i as f32 / steps as f32);
        let r = u * t.cos() + v * t.sin();
        let wp = pivot + r * radius_w;
        if let Some((sp, _)) = world_to_screen(camera, rect, wp) {
            pts.push(sp);
        }
    }

    if pts.len() >= 2 {
        // Slightly boost thickness for active feedback.
        let active = Stroke::new(outline.width * 1.25, outline.color);
        p.add(egui::Shape::line(pts.clone(), active));

        // UE-style active sector: filled wedge + internal grid mesh.
        // We draw it in projected world-space to keep perspective consistent.
        if style.rotate_plane_fill_alpha != 0 {
            let fill2 = Color32::from_rgba_unmultiplied(fill.r(), fill.g(), fill.b(), style.rotate_plane_fill_alpha);
            let grid = Color32::from_rgba_unmultiplied(outline.color.r(), outline.color.g(), outline.color.b(), style.rotate_plane_grid_alpha);
            draw_world_wedge_mesh(p, camera, rect, pivot, u, v, radius_w, a0, a0 + da, fill2, grid, style);
        } else {
            // Minimal fallback: subtle fill without mesh.
            let mut poly: Vec<Pos2> = Vec::with_capacity(pts.len() + 1);
            poly.push(c2);
            poly.extend_from_slice(&pts);
            p.add(egui::Shape::convex_polygon(poly, fill, Stroke::NONE));
        }
    }
}

fn draw_world_wedge_mesh(
    p: &Painter,
    camera: &impl GizmoCamera,
    rect: Rect,
    pivot: newengine_math::Vec3,
    u: newengine_math::Vec3,
    v: newengine_math::Vec3,
    radius_w: f32,
    t0: f32,
    t1: f32,
    fill: Color32,
    grid: Color32,
    style: GizmoStyle,
) {
    let mut da = t1 - t0;
    while da > core::f32::consts::PI {
        da -= core::f32::consts::TAU;
    }
    while da < -core::f32::consts::PI {
        da += core::f32::consts::TAU;
    }

    let steps = ((style.rotate_segments as f32) * (da.abs() / core::f32::consts::TAU)).ceil().max(24.0) as usize;
    let Some((c2, _)) = world_to_screen(camera, rect, pivot) else { return; };

    // Outer arc in screen space from projected world points.
    let mut arc: Vec<Pos2> = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let s = i as f32 / steps as f32;
        let t = t0 + da * s;
        let r = u * t.cos() + v * t.sin();
        let wp = pivot + r * radius_w;
        if let Some((sp, _)) = world_to_screen(camera, rect, wp) {
            arc.push(sp);
        }
    }

    if arc.len() >= 2 {
        let mut poly: Vec<Pos2> = Vec::with_capacity(arc.len() + 1);
        poly.push(c2);
        poly.extend_from_slice(&arc);
        p.add(egui::Shape::convex_polygon(poly, fill, Stroke::NONE));
    }

    // Radial grid lines.
    let ang_div = style.rotate_plane_grid_angular.max(2) as usize;
    for i in 1..ang_div {
        let s = i as f32 / ang_div as f32;
        let t = t0 + da * s;
        let r = u * t.cos() + v * t.sin();
        let wp = pivot + r * radius_w;
        if let Some((sp, _)) = world_to_screen(camera, rect, wp) {
            p.line_segment([c2, sp], Stroke::new(1.0, grid));
        }
    }

    // Concentric arcs.
    let rad_div = style.rotate_plane_grid_radial.max(2) as usize;
    for j in 1..rad_div {
        let rr = radius_w * (j as f32 / rad_div as f32);
        let mut pts: Vec<Pos2> = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let s = i as f32 / steps as f32;
            let t = t0 + da * s;
            let r = u * t.cos() + v * t.sin();
            let wp = pivot + r * rr;
            if let Some((sp, _)) = world_to_screen(camera, rect, wp) {
                pts.push(sp);
            }
        }
        if pts.len() >= 2 {
            p.add(egui::Shape::line(pts, Stroke::new(1.0, grid)));
        }
    }
}
