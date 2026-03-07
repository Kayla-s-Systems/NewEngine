#![forbid(unsafe_op_in_unsafe_fn)]

use egui;

const OUTLINER_MIN_W: f32 = 200.0;
const DETAILS_MIN_W: f32 = 260.0;
const LEFT_TOOLBAR_W: f32 = 56.0;

// Hard guardrail: never allow side panels to eat the viewport completely.
// This is the single most important UX invariant for an editor.
const VIEWPORT_MIN_W: f32 = 700.0;

#[inline]
pub(super) fn outliner_max_width(
    ctx: &egui::Context,
    show_left_toolbar: bool,
    show_details: bool,
) -> f32 {
    let screen_w = ctx.screen_rect().width().max(1.0);
    let fixed = if show_left_toolbar { LEFT_TOOLBAR_W } else { 0.0 };

    let budget = (screen_w - fixed - VIEWPORT_MIN_W).max(0.0);
    let max_by_layout = if show_details {
        (budget * 0.5).max(OUTLINER_MIN_W)
    } else {
        budget.max(OUTLINER_MIN_W)
    };

    let max_cap = (screen_w * 0.55).clamp(OUTLINER_MIN_W, 720.0);
    max_by_layout.min(max_cap)
}

#[inline]
pub(super) fn details_max_width(
    ctx: &egui::Context,
    show_left_toolbar: bool,
    show_outliner: bool,
) -> f32 {
    let screen_w = ctx.screen_rect().width().max(1.0);
    let fixed = if show_left_toolbar { LEFT_TOOLBAR_W } else { 0.0 };

    let budget = (screen_w - fixed - VIEWPORT_MIN_W).max(0.0);
    let max_by_layout = if show_outliner {
        (budget * 0.5).max(DETAILS_MIN_W)
    } else {
        budget.max(DETAILS_MIN_W)
    };

    let max_cap = (screen_w * 0.55).clamp(DETAILS_MIN_W, 800.0);
    max_by_layout.min(max_cap)
}

pub(super) fn infer_model_exts(snap: &newengine_core::plugins::PluginsSnapshot) -> Vec<String> {
    use std::collections::BTreeSet;

    let mut out: BTreeSet<String> = BTreeSet::new();
    let tokens: [(&str, &[&str]); 8] = [
        ("obj", &["obj"]),
        ("gltf", &["gltf"]),
        ("glb", &["glb"]),
        ("fbx", &["fbx"]),
        ("dae", &["dae", "collada"]),
        ("stl", &["stl"]),
        ("ply", &["ply"]),
        ("blend", &["blend"]),
    ];

    for p in &snap.plugins {
        for c in &p.capabilities {
            let id = c.id.to_ascii_lowercase();
            for (ext, keys) in tokens {
                if keys.iter().any(|k| id.contains(k)) {
                    out.insert(format!(".{ext}"));
                }
            }
        }
    }

    out.into_iter().collect()
}

#[inline]
pub(super) fn world_to_screen(
    frame: &crate::viewport_bridge::ViewportCameraFrame,
    rect: egui::Rect,
    world: newengine_math::Vec3,
) -> Option<(egui::Pos2, f32)> {
    let v = frame.viewproj * newengine_math::Vec4::new(world.x, world.y, world.z, 1.0);
    if !v.w.is_finite() || v.w.abs() < 1e-6 {
        return None;
    }
    let ndc = v / v.w;
    if !ndc.x.is_finite() || !ndc.y.is_finite() || !ndc.z.is_finite() {
        return None;
    }
    let sx_px = (ndc.x * 0.5 + 0.5) * frame.vp_w as f32;
    let sy_px = (ndc.y * 0.5 + 0.5) * frame.vp_h as f32;

    let ppp = (rect.width() / frame.vp_w as f32).max(1e-6);
    let x_pt = rect.min.x + sx_px * ppp;
    let y_pt = rect.min.y + sy_px * ppp;
    Some((egui::pos2(x_pt, y_pt), ndc.z))
}

#[allow(dead_code)]
#[inline]
pub(super) fn screen_to_world_at_ndc_z(
    frame: &crate::viewport_bridge::ViewportCameraFrame,
    rect: egui::Rect,
    screen: egui::Pos2,
    ndc_z: f32,
) -> newengine_math::Vec3 {
    let ppp = (rect.width() / frame.vp_w as f32).max(1e-6);
    let px = ((screen.x - rect.min.x) / ppp).clamp(0.0, frame.vp_w as f32);
    let py = ((screen.y - rect.min.y) / ppp).clamp(0.0, frame.vp_h as f32);

    let x = (px / frame.vp_w as f32) * 2.0 - 1.0;
    let y = (py / frame.vp_h as f32) * 2.0 - 1.0;

    let h = frame.inv_viewproj * newengine_math::Vec4::new(x, y, ndc_z, 1.0);
    if h.w.abs() < 1e-6 {
        return newengine_math::Vec3::ZERO;
    }
    (h / h.w).truncate()
}

pub(super) fn draw_selection_outline(
    painter: &egui::Painter,
    frame: &crate::viewport_bridge::ViewportCameraFrame,
    rect: egui::Rect,
    pos: newengine_math::Vec3,
    rot: newengine_math::Quat,
    scale: newengine_math::Vec3,
) {
    // Modern DCC-style highlight:
    // - projects OBB corners
    // - computes screen-space AABB
    // - draws corner brackets + center dot + axis hints

    let hx = 0.5 * scale.x.abs().max(0.001);
    let hy = 0.5 * scale.y.abs().max(0.001);
    let hz = 0.5 * scale.z.abs().max(0.001);
    let corners_local = [
        newengine_math::Vec3::new(-hx, -hy, -hz),
        newengine_math::Vec3::new(hx, -hy, -hz),
        newengine_math::Vec3::new(hx, -hy, hz),
        newengine_math::Vec3::new(-hx, -hy, hz),
        newengine_math::Vec3::new(-hx, hy, -hz),
        newengine_math::Vec3::new(hx, hy, -hz),
        newengine_math::Vec3::new(hx, hy, hz),
        newengine_math::Vec3::new(-hx, hy, hz),
    ];

    let mut min = egui::pos2(f32::INFINITY, f32::INFINITY);
    let mut max = egui::pos2(f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut any = false;

    for c in corners_local {
        let wpos = pos + (rot * c);
        if let Some((sp, _z)) = world_to_screen(frame, rect, wpos) {
            any = true;
            min.x = min.x.min(sp.x);
            min.y = min.y.min(sp.y);
            max.x = max.x.max(sp.x);
            max.y = max.y.max(sp.y);
        }
    }

    if !any {
        return;
    }

    let bb = egui::Rect::from_min_max(min, max);

    // Corner bracket size scales with bbox.
    let w = bb.width().max(1.0);
    let h = bb.height().max(1.0);
    let k = (w.min(h) * 0.18).clamp(8.0, 28.0);

    let glow = egui::Stroke::new(4.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 110));
    let stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(235, 210, 90));

    let c0 = bb.min;
    let c1 = egui::pos2(bb.max.x, bb.min.y);
    let c2 = bb.max;
    let c3 = egui::pos2(bb.min.x, bb.max.y);

    let corners = [c0, c1, c2, c3];
    let dirs = [
        (egui::vec2(1.0, 0.0), egui::vec2(0.0, 1.0)),
        (egui::vec2(-1.0, 0.0), egui::vec2(0.0, 1.0)),
        (egui::vec2(-1.0, 0.0), egui::vec2(0.0, -1.0)),
        (egui::vec2(1.0, 0.0), egui::vec2(0.0, -1.0)),
    ];

    for (c, (dx, dy)) in corners.into_iter().zip(dirs) {
        let a = c;
        let b = c + dx * k;
        let d = c + dy * k;
        painter.line_segment([a, b], glow);
        painter.line_segment([a, d], glow);
        painter.line_segment([a, b], stroke);
        painter.line_segment([a, d], stroke);
    }

    // Center dot.
    let center = bb.center();
    painter.circle_filled(center, 2.5, egui::Color32::from_rgb(235, 210, 90));

    // Axis hints (small triad), screen-space.
    let axis_len = k * 0.75;
    let x_dir = rot * newengine_math::Vec3::X;
    let y_dir = rot * newengine_math::Vec3::Y;
    let z_dir = rot * newengine_math::Vec3::Z;

    let p0 = world_to_screen(frame, rect, pos)
        .map(|v| v.0)
        .unwrap_or(center);
    if let Some((px, _)) = world_to_screen(frame, rect, pos + x_dir * 0.35) {
        let v = px - p0;
        let l = v.length().max(1e-4);
        painter.line_segment(
            [p0, p0 + v / l * axis_len],
            egui::Stroke::new(1.25, egui::Color32::from_rgb(240, 80, 80)),
        );
    }
    if let Some((py, _)) = world_to_screen(frame, rect, pos + y_dir * 0.35) {
        let v = py - p0;
        let l = v.length().max(1e-4);
        painter.line_segment(
            [p0, p0 + v / l * axis_len],
            egui::Stroke::new(1.25, egui::Color32::from_rgb(90, 240, 120)),
        );
    }
    if let Some((pz, _)) = world_to_screen(frame, rect, pos + z_dir * 0.35) {
        let v = pz - p0;
        let l = v.length().max(1e-4);
        painter.line_segment(
            [p0, p0 + v / l * axis_len],
            egui::Stroke::new(1.25, egui::Color32::from_rgb(90, 160, 255)),
        );
    }
}
