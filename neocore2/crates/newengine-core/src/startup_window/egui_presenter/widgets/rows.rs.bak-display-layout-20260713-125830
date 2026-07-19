#![forbid(unsafe_op_in_unsafe_fn)]

use eframe::egui;
use newengine_loading_api::bootstrap_ui::north_star_bootstrap_ui_style;

use super::super::style::color32;

pub(in crate::startup_window::egui_presenter) fn setting_label(
    ui: &mut egui::Ui,
    title: &str,
    detail: &str,
) {
    let style = north_star_bootstrap_ui_style();
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(title)
                .size(12.5)
                .strong()
                .color(color32(style.palette.text)),
        );
        ui.label(
            egui::RichText::new(detail)
                .size(10.5)
                .color(color32(style.palette.muted)),
        );
    });
}
pub(in crate::startup_window::egui_presenter) fn setting_group_label(
    ui: &mut egui::Ui,
    label: &str,
) {
    let palette = north_star_bootstrap_ui_style().palette;
    ui.label(
        egui::RichText::new(label)
            .size(9.5)
            .strong()
            .monospace()
            .color(color32(palette.muted)),
    );
    ui.add_space(5.0);
}
pub(in crate::startup_window::egui_presenter) fn diagnostic_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &str,
) {
    let palette = north_star_bootstrap_ui_style().palette;
    ui.label(
        egui::RichText::new(label)
            .size(11.5)
            .color(color32(palette.text_dim)),
    );
    ui.label(
        egui::RichText::new(value)
            .size(11.5)
            .monospace()
            .color(color32(palette.text)),
    );
    ui.end_row();
}
pub(in crate::startup_window::egui_presenter) fn float_parameter_row(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    speed: f64,
) -> bool {
    setting_label(ui, label, "Renderer parameter");
    let changed = ui
        .add_enabled(
            enabled,
            egui::DragValue::new(value).range(range).speed(speed),
        )
        .changed();
    ui.end_row();
    changed
}
pub(in crate::startup_window::egui_presenter) fn integer_parameter_row(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
) -> bool {
    setting_label(ui, label, "Renderer parameter");
    let changed = ui
        .add_enabled(enabled, egui::DragValue::new(value).range(range))
        .changed();
    ui.end_row();
    changed
}
pub(in crate::startup_window::egui_presenter) fn variable_row(
    ui: &mut egui::Ui,
    name: &str,
    value: &str,
) {
    let palette = north_star_bootstrap_ui_style().palette;
    ui.label(
        egui::RichText::new(name)
            .size(10.5)
            .monospace()
            .color(color32(palette.text_dim)),
    );
    ui.label(
        egui::RichText::new(value)
            .size(10.5)
            .monospace()
            .color(color32(palette.blue_bright)),
    );
    ui.end_row();
}
