use super::types::GizmoStyle;
use crate::GizmoAxis;
use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Stroke, StrokeKind};

pub(crate) fn axis_color(axis: GizmoAxis, hovered: Option<GizmoAxis>, active: Option<GizmoAxis>, highlight_mul: f32) -> Color32 {
    let base = match axis {
        GizmoAxis::X => Color32::from_rgb(220, 70, 70),
        GizmoAxis::Y => Color32::from_rgb(80, 210, 110),
        GizmoAxis::Z => Color32::from_rgb(80, 140, 255),
    };

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
    p.line_segment([a, b], stroke);

    let dir = b - a;
    let len = dir.length().max(1.0);
    let n = dir / len;
    let perp = egui::vec2(-n.y, n.x);

    let tip = b;
    let s = style.arrow_size_pt;
    let left = tip - n * s + perp * (s * 0.45);
    let right = tip - n * s - perp * (s * 0.45);

    p.line_segment([left, tip], stroke);
    p.line_segment([right, tip], stroke);
}

pub(crate) fn draw_axis_scale(p: &Painter, a: Pos2, b: Pos2, color: Color32, style: GizmoStyle) {
    let stroke = Stroke::new(style.line_width_pt, color);
    p.line_segment([a, b], stroke);

    let s = style.arrow_size_pt.max(6.0);
    let half = s * 0.5;
    let r = Rect::from_min_max(Pos2::new(b.x - half, b.y - half), Pos2::new(b.x + half, b.y + half));

    // Solid cube-like handle in screen space.
    let cr = CornerRadius::same(1.0 as u8);
    p.rect_filled(r, cr, color);
    p.rect_stroke(
        r,
        cr,
        Stroke::new(1.0, Color32::from_rgb(20, 20, 20)),
        StrokeKind::Inside,
    );
}
