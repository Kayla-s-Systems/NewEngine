#![forbid(unsafe_op_in_unsafe_fn)]

use eframe::egui;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub accent: egui::Color32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            accent: egui::Color32::from_rgb(86, 156, 214),
        }
    }
}

pub fn apply_once(ctx: &egui::Context, theme: Theme) {
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = egui::Color32::from_rgb(18, 18, 18);
    visuals.window_fill = egui::Color32::from_rgb(22, 22, 22);
    visuals.extreme_bg_color = egui::Color32::from_rgb(14, 14, 14);
    visuals.faint_bg_color = egui::Color32::from_rgb(28, 28, 28);

    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(30, 30, 30);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(40, 40, 40);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(46, 46, 46);

    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, theme.accent);
    visuals.selection.bg_fill = theme.accent.linear_multiply(0.35);
    visuals.selection.stroke = egui::Stroke::new(1.0, theme.accent);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.visuals.window_shadow = egui::Shadow::NONE;

    let cr = egui::CornerRadius::same(10);
    style.visuals.widgets.inactive.corner_radius = cr;
    style.visuals.widgets.hovered.corner_radius = cr;
    style.visuals.widgets.active.corner_radius = cr;
    style.visuals.widgets.open.corner_radius = cr;
    style.visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(8);

    ctx.set_style(style);
}

pub fn card_frame(theme: Theme) -> egui::Frame {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(24, 24, 24))
        .stroke(egui::Stroke::new(1.0, theme.accent.linear_multiply(0.25)))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin::same(12))
}
