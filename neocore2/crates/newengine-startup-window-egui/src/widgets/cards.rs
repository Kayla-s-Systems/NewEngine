#![forbid(unsafe_op_in_unsafe_fn)]

use eframe::egui;
use newengine_loading_api::bootstrap_ui::north_star_bootstrap_ui_style;

use super::super::style::color32;

pub(crate) fn sidebar_card(
    ui: &mut egui::Ui,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let style = north_star_bootstrap_ui_style();
    egui::Frame::none()
        .fill(color32(style.palette.panel))
        .stroke(egui::Stroke::new(1.0, color32(style.palette.edge_soft)))
        .rounding(egui::Rounding::same(9.0))
        .inner_margin(egui::Margin::same(12.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new(title)
                    .size(9.5)
                    .strong()
                    .color(color32(style.palette.muted)),
            );
            ui.add_space(7.0);
            add_contents(ui);
        });
}
pub(crate) fn section_card(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let style = north_star_bootstrap_ui_style();
    egui::Frame::none()
        .fill(color32(style.palette.panel))
        .stroke(egui::Stroke::new(1.0, color32(style.palette.edge_soft)))
        .rounding(egui::Rounding::same(11.0))
        .inner_margin(egui::Margin::same(18.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new(title)
                    .size(16.5)
                    .strong()
                    .color(color32(style.palette.text)),
            );
            ui.label(
                egui::RichText::new(subtitle)
                    .size(11.5)
                    .color(color32(style.palette.text_dim)),
            );
            ui.add_space(12.0);
            add_contents(ui);
        });
}
pub(crate) fn status_pill(ui: &mut egui::Ui, label: &str, accent: egui::Color32) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(
            accent.r(),
            accent.g(),
            accent.b(),
            24,
        ))
        .stroke(egui::Stroke::new(1.0, accent))
        .rounding(egui::Rounding::same(999.0))
        .inner_margin(egui::Margin::symmetric(9.0, 4.0))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(label)
                    .size(9.5)
                    .strong()
                    .monospace()
                    .color(accent),
            );
        });
}
pub(crate) fn warning_banner(ui: &mut egui::Ui, text: &str) {
    let palette = north_star_bootstrap_ui_style().palette;
    egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(
            palette.warn.r,
            palette.warn.g,
            palette.warn.b,
            18,
        ))
        .stroke(egui::Stroke::new(1.0_f32, color32(palette.warn)))
        .rounding(egui::Rounding::same(8.0))
        .inner_margin(egui::Margin::symmetric(14.0, 10.0))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!("PERFORMANCE NOTICE  /  {text}"))
                    .size(11.5)
                    .color(color32(palette.warn)),
            );
        });
}
pub(crate) fn summary_metric(ui: &mut egui::Ui, label: &str, value: &str) {
    let palette = north_star_bootstrap_ui_style().palette;
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(label)
                .size(9.0)
                .strong()
                .monospace()
                .color(color32(palette.muted)),
        );
        ui.label(
            egui::RichText::new(value)
                .size(12.0)
                .strong()
                .color(color32(palette.text)),
        );
    });
}
