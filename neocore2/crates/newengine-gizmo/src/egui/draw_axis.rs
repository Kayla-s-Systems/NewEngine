use super::types::GizmoStyle;
use crate::GizmoAxis;
use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Stroke, StrokeKind};

pub(crate) fn axis_color(
    axis: GizmoAxis,
    hovered: Option<GizmoAxis>,
    active: Option<GizmoAxis>,
    highlight_mul: f32,
) -> Color32 {
    let base = match axis {
        GizmoAxis::X => Color32::from_rgb(220, 70, 70),
        GizmoAxis::Y => Color32::from_rgb(80, 210, 110),
        GizmoAxis::Z => Color32::from_rgb(80, 140, 255),
        GizmoAxis::Screen => Color32::from_rgb(235, 235, 235),
    };

    if active == Some(axis) {
        // UE-like: screen ring becomes warm when active.
        if axis == GizmoAxis::Screen {
            return Color32::from_rgb(255, 210, 90);
        }
    }

    if active == Some(axis) || hovered == Some(axis) {
        let r = (base.r() as f32 * highlight_mul).min(255.0) as u8;
        let g = (base.g() as f32 * highlight_mul).min(255.0) as u8;
        let b = (base.b() as f32 * highlight_mul).min(255.0) as u8;
        Color32::from_rgb(r, g, b)
    } else {
        base
    }
}

pub(crate) fn draw_axis(p: &Painter, a: Pos2, b: Pos2, color: Color32, style: GizmoStyle) {
    let stroke = Stroke::new(style.line_width_pt, color);

    // Runtime/AAA style (translate): axis line + solid arrow head.
    // Cube caps are kept for scale handles; translation needs clear directional arrows.
    let dir = b - a;
    let len = dir.length().max(1.0);
    let n = dir / len;

    let cap = style.axis_cap_pt.max(8.0);
    let head_len = cap;
    let head_w = (cap * 0.75).max(6.0);

    let stem_end = b - n * head_len;
    p.line_segment([a, stem_end], stroke);

    let perp = egui::vec2(-n.y, n.x);
    let left = stem_end + perp * (head_w * 0.5);
    let right = stem_end - perp * (head_w * 0.5);

    let tri = vec![b, left, right];
    p.add(egui::Shape::convex_polygon(
        tri.clone(),
        color,
        Stroke::NONE,
    ));
    p.add(egui::Shape::closed_line(
        tri,
        Stroke::new(1.0, Color32::from_rgb(20, 20, 20)),
    ));
}

pub(crate) fn draw_axis_scale(p: &Painter, a: Pos2, b: Pos2, color: Color32, style: GizmoStyle) {
    let stroke = Stroke::new(style.line_width_pt, color);

    let cap = (style.axis_cap_pt * 1.05).max(7.0);
    let dir = b - a;
    let len = dir.length().max(1.0);
    let n = dir / len;
    let stem_end = b - n * (cap * 0.65);
    p.line_segment([a, stem_end], stroke);

    let half = cap * 0.5;
    let r = Rect::from_min_max(
        Pos2::new(b.x - half, b.y - half),
        Pos2::new(b.x + half, b.y + half),
    );

    // Solid cube-like handle in screen space.
    let cr = CornerRadius::same(2);
    p.rect_filled(r, cr, color);
    p.rect_stroke(
        r,
        cr,
        Stroke::new(1.0, Color32::from_rgb(20, 20, 20)),
        StrokeKind::Inside,
    );
}
