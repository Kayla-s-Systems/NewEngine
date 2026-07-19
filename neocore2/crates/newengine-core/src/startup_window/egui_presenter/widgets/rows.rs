#![forbid(unsafe_op_in_unsafe_fn)]

use eframe::egui;
use newengine_loading_api::bootstrap_ui::north_star_bootstrap_ui_style;

use super::super::style::color32;

const GRID_SETTING_LABEL_WIDTH: f32 = 220.0;

pub(in crate::startup_window::egui_presenter) fn setting_label(
    ui: &mut egui::Ui,
    title: &str,
    detail: &str,
) {
    let style = north_star_bootstrap_ui_style();
    ui.vertical(|ui| {
        // Egui Grid otherwise sizes this cell from the short title and then wraps
        // the explanatory text into a one-word-wide column. Keep every settings
        // page readable even when it still uses the legacy two-column grid.
        ui.set_min_width(GRID_SETTING_LABEL_WIDTH.min(ui.available_width().max(1.0)));
        ui.add(
            egui::Label::new(
                egui::RichText::new(title)
                    .size(12.5)
                    .strong()
                    .color(color32(style.palette.text)),
            )
            .wrap(),
        );
        ui.add(
            egui::Label::new(
                egui::RichText::new(detail)
                    .size(10.5)
                    .color(color32(style.palette.muted)),
            )
            .wrap(),
        );
    });
}

pub(in crate::startup_window::egui_presenter) fn setting_block(
    ui: &mut egui::Ui,
    title: &str,
    detail: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let style = north_star_bootstrap_ui_style();
    let outer_width = ui.available_width();
    egui::Frame::none()
        .fill(color32(style.palette.bg_deep))
        .stroke(egui::Stroke::new(1.0, color32(style.palette.edge_soft)))
        .rounding(egui::Rounding::same(8.0))
        .inner_margin(egui::Margin::same(12.0))
        .show(ui, |ui| {
            ui.set_min_width((outer_width - 24.0).max(180.0));
            ui.add(
                egui::Label::new(
                    egui::RichText::new(title)
                        .size(13.0)
                        .strong()
                        .color(color32(style.palette.text)),
                )
                .wrap(),
            );
            ui.add(
                egui::Label::new(
                    egui::RichText::new(detail)
                        .size(10.5)
                        .color(color32(style.palette.muted)),
                )
                .wrap(),
            );
            ui.add_space(9.0);
            add_contents(ui);
        });
}

pub(in crate::startup_window::egui_presenter) fn value_caption(ui: &mut egui::Ui, label: &str) {
    let palette = north_star_bootstrap_ui_style().palette;
    ui.label(
        egui::RichText::new(label)
            .size(9.0)
            .strong()
            .monospace()
            .color(color32(palette.muted)),
    );
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
