#![forbid(unsafe_op_in_unsafe_fn)]

use eframe::egui;

// SVG source assets live in `assets/icons/prestart/*.svg`.
// These painter routines are runtime fallbacks for egui builds that do not
// include a vector-image renderer.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IconKind {
    Logo,
    Project,
    Terminal,
    Chip,
    Monitor,
    ScreenMode,
    Check,
    Bookmark,
    Puzzle,
    Clock,
    Save,
    Launch,
    Cancel,
    Folder,
    Settings,
    Core,
    Renderer,
    Physics,
    Audio,
    Input,
    Ui,
    Animation,
    Script,
}

pub(crate) fn icon(ui: &mut egui::Ui, kind: IconKind, size: f32, color: egui::Color32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    paint_icon(ui.painter(), rect.shrink(size * 0.12), kind, color);
    response
}

pub(crate) fn paint_icon(painter: &egui::Painter, rect: egui::Rect, kind: IconKind, color: egui::Color32) {
    let stroke = egui::Stroke::new((rect.width() * 0.075).clamp(1.1, 2.2), color);
    match kind {
        IconKind::Logo => paint_logo(painter, rect, color),
        IconKind::Project | IconKind::Core => paint_cube(painter, rect, stroke),
        IconKind::Terminal | IconKind::Script => paint_terminal(painter, rect, stroke),
        IconKind::Chip | IconKind::Renderer => paint_chip(painter, rect, stroke),
        IconKind::Monitor | IconKind::Ui => paint_monitor(painter, rect, stroke),
        IconKind::ScreenMode => paint_screen_mode(painter, rect, stroke),
        IconKind::Check => paint_check(painter, rect, stroke),
        IconKind::Bookmark => paint_bookmark(painter, rect, stroke),
        IconKind::Puzzle => paint_puzzle(painter, rect, stroke),
        IconKind::Clock => paint_clock(painter, rect, stroke),
        IconKind::Save => paint_save(painter, rect, stroke),
        IconKind::Launch => paint_launch(painter, rect, color),
        IconKind::Cancel => paint_cancel(painter, rect, stroke),
        IconKind::Folder => paint_folder(painter, rect, stroke),
        IconKind::Settings => paint_settings(painter, rect, stroke),
        IconKind::Physics => paint_physics(painter, rect, stroke),
        IconKind::Audio => paint_audio(painter, rect, stroke),
        IconKind::Input => paint_input(painter, rect, stroke),
        IconKind::Animation => paint_animation(painter, rect, stroke),
    }
}

fn c(rect: egui::Rect, x: f32, y: f32) -> egui::Pos2 {
    egui::pos2(egui::lerp(rect.left()..=rect.right(), x), egui::lerp(rect.top()..=rect.bottom(), y))
}

fn paint_logo(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let stroke = egui::Stroke::new((rect.width() * 0.06).clamp(1.1, 2.4), color);
    let center = rect.center();
    let r = rect.width().min(rect.height()) * 0.43;
    let mut pts = Vec::new();
    for i in 0..6 {
        let angle = std::f32::consts::TAU * (i as f32) / 6.0 - std::f32::consts::FRAC_PI_2;
        pts.push(center + egui::vec2(angle.cos() * r, angle.sin() * r));
    }
    painter.add(egui::Shape::closed_line(pts, stroke));
    let cyan = egui::Color32::from_rgb(92, 181, 255);
    painter.line_segment([center, c(rect, 0.5, 0.12)], egui::Stroke::new(stroke.width * 1.3, cyan));
    painter.line_segment([center, c(rect, 0.18, 0.68)], egui::Stroke::new(stroke.width * 1.3, cyan));
    painter.line_segment([center, c(rect, 0.82, 0.68)], egui::Stroke::new(stroke.width * 1.3, cyan));
}

fn paint_cube(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    let a = c(r, 0.50, 0.10);
    let b = c(r, 0.82, 0.28);
    let d = c(r, 0.50, 0.46);
    let e = c(r, 0.18, 0.28);
    let f = c(r, 0.18, 0.62);
    let g = c(r, 0.50, 0.84);
    let h = c(r, 0.82, 0.62);
    for (x, y) in [(a,b),(b,d),(d,e),(e,a),(e,f),(f,g),(g,h),(h,b),(d,g)] { p.line_segment([x,y], s); }
}

fn paint_terminal(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    p.rect_stroke(r, egui::CornerRadius::same(3), s, egui::StrokeKind::Inside);
    p.line_segment([c(r,0.18,0.32), c(r,0.36,0.50)], s);
    p.line_segment([c(r,0.36,0.50), c(r,0.18,0.68)], s);
    p.line_segment([c(r,0.48,0.68), c(r,0.76,0.68)], s);
}

fn paint_chip(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    let body = egui::Rect::from_min_max(c(r,0.25,0.25), c(r,0.75,0.75));
    p.rect_stroke(body, egui::CornerRadius::same(3), s, egui::StrokeKind::Inside);
    for t in [0.18, 0.34, 0.50, 0.66, 0.82] {
        p.line_segment([c(r,t,0.12), c(r,t,0.25)], s);
        p.line_segment([c(r,t,0.75), c(r,t,0.88)], s);
        p.line_segment([c(r,0.12,t), c(r,0.25,t)], s);
        p.line_segment([c(r,0.75,t), c(r,0.88,t)], s);
    }
    p.rect_stroke(egui::Rect::from_min_max(c(r,0.38,0.38), c(r,0.62,0.62)), egui::CornerRadius::same(2), s, egui::StrokeKind::Inside);
}

fn paint_monitor(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    p.rect_stroke(egui::Rect::from_min_max(c(r,0.12,0.18), c(r,0.88,0.68)), egui::CornerRadius::same(3), s, egui::StrokeKind::Inside);
    p.line_segment([c(r,0.50,0.68), c(r,0.50,0.82)], s);
    p.line_segment([c(r,0.32,0.84), c(r,0.68,0.84)], s);
}

fn paint_screen_mode(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    p.rect_stroke(egui::Rect::from_min_max(c(r,0.10,0.24), c(r,0.90,0.76)), egui::CornerRadius::same(3), s, egui::StrokeKind::Inside);
    p.line_segment([c(r,0.38,0.24), c(r,0.38,0.76)], s);
    p.line_segment([c(r,0.64,0.24), c(r,0.64,0.76)], s);
}

fn paint_check(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    p.line_segment([c(r,0.18,0.54), c(r,0.42,0.76)], s);
    p.line_segment([c(r,0.42,0.76), c(r,0.84,0.24)], s);
}

fn paint_bookmark(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    let pts = vec![c(r,0.28,0.12), c(r,0.72,0.12), c(r,0.72,0.86), c(r,0.50,0.68), c(r,0.28,0.86), c(r,0.28,0.12)];
    p.add(egui::Shape::line(pts, s));
}

fn paint_puzzle(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    p.rect_stroke(egui::Rect::from_min_max(c(r,0.18,0.25), c(r,0.82,0.78)), egui::CornerRadius::same(4), s, egui::StrokeKind::Inside);
    p.circle_stroke(c(r,0.50,0.25), r.width()*0.10, s);
    p.circle_stroke(c(r,0.82,0.50), r.width()*0.09, s);
}

fn paint_clock(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    p.circle_stroke(r.center(), r.width().min(r.height())*0.38, s);
    p.line_segment([r.center(), c(r,0.50,0.26)], s);
    p.line_segment([r.center(), c(r,0.68,0.58)], s);
}

fn paint_save(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    p.rect_stroke(egui::Rect::from_min_max(c(r,0.18,0.14), c(r,0.82,0.86)), egui::CornerRadius::same(3), s, egui::StrokeKind::Inside);
    p.rect_stroke(egui::Rect::from_min_max(c(r,0.30,0.16), c(r,0.68,0.38)), egui::CornerRadius::same(2), s, egui::StrokeKind::Inside);
    p.line_segment([c(r,0.32,0.72), c(r,0.68,0.72)], s);
}

fn paint_launch(p: &egui::Painter, r: egui::Rect, color: egui::Color32) {
    let pts = vec![c(r,0.30,0.18), c(r,0.82,0.50), c(r,0.30,0.82)];
    p.add(egui::Shape::convex_polygon(pts, color, egui::Stroke::NONE));
}

fn paint_cancel(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    p.line_segment([c(r,0.22,0.22), c(r,0.78,0.78)], s);
    p.line_segment([c(r,0.78,0.22), c(r,0.22,0.78)], s);
}

fn paint_folder(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    let pts = vec![c(r,0.10,0.32), c(r,0.36,0.32), c(r,0.44,0.22), c(r,0.74,0.22), c(r,0.84,0.34), c(r,0.90,0.34), c(r,0.90,0.78), c(r,0.10,0.78), c(r,0.10,0.32)];
    p.add(egui::Shape::line(pts, s));
}

fn paint_settings(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    p.circle_stroke(r.center(), r.width()*0.18, s);
    for i in 0..8 {
        let a = std::f32::consts::TAU * (i as f32) / 8.0;
        let v = egui::vec2(a.cos(), a.sin());
        p.line_segment([r.center()+v*r.width()*0.28, r.center()+v*r.width()*0.42], s);
    }
}

fn paint_physics(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    p.circle_stroke(c(r,0.30,0.34), r.width()*0.13, s);
    p.circle_stroke(c(r,0.68,0.34), r.width()*0.13, s);
    p.circle_stroke(c(r,0.50,0.70), r.width()*0.13, s);
    p.line_segment([c(r,0.40,0.40), c(r,0.58,0.40)], s);
    p.line_segment([c(r,0.35,0.48), c(r,0.45,0.62)], s);
    p.line_segment([c(r,0.64,0.48), c(r,0.55,0.62)], s);
}

fn paint_audio(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    p.line_segment([c(r,0.18,0.42), c(r,0.36,0.42)], s);
    p.line_segment([c(r,0.36,0.42), c(r,0.58,0.24)], s);
    p.line_segment([c(r,0.58,0.24), c(r,0.58,0.76)], s);
    p.line_segment([c(r,0.58,0.76), c(r,0.36,0.58)], s);
    p.line_segment([c(r,0.36,0.58), c(r,0.18,0.58)], s);
    let center = c(r,0.64,0.50);
    let radius = r.width()*0.20;
    let points = (0..=8)
        .map(|i| {
            let t = -0.7 + 1.4 * (i as f32 / 8.0);
            center + egui::vec2(t.cos() * radius, t.sin() * radius)
        })
        .collect::<Vec<_>>();
    p.add(egui::Shape::line(points, s));
}

fn paint_input(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    p.rect_stroke(egui::Rect::from_min_max(c(r,0.16,0.28), c(r,0.84,0.72)), egui::CornerRadius::same(9), s, egui::StrokeKind::Inside);
    p.circle_stroke(c(r,0.34,0.50), r.width()*0.08, s);
    p.line_segment([c(r,0.62,0.43), c(r,0.62,0.57)], s);
    p.line_segment([c(r,0.55,0.50), c(r,0.69,0.50)], s);
}

fn paint_animation(p: &egui::Painter, r: egui::Rect, s: egui::Stroke) {
    p.circle_stroke(c(r,0.50,0.18), r.width()*0.08, s);
    p.line_segment([c(r,0.50,0.28), c(r,0.50,0.58)], s);
    p.line_segment([c(r,0.50,0.40), c(r,0.26,0.50)], s);
    p.line_segment([c(r,0.50,0.40), c(r,0.76,0.36)], s);
    p.line_segment([c(r,0.50,0.58), c(r,0.30,0.84)], s);
    p.line_segment([c(r,0.50,0.58), c(r,0.74,0.84)], s);
}
