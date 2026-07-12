#![forbid(unsafe_op_in_unsafe_fn)]

use eframe::egui;
use newengine_loading_api::bootstrap_ui::north_star_bootstrap_ui_style;

use crate::startup_window::GraphicsPreset;

use super::super::model::SettingsPage;
use super::super::style::color32;

pub(crate) fn nav_button(ui: &mut egui::Ui, page: SettingsPage, selected: bool) -> egui::Response {
    let style = north_star_bootstrap_ui_style();
    let fill = if selected {
        color32(style.palette.panel_active)
    } else {
        color32(style.palette.bg_deep)
    };
    let stroke = if selected {
        egui::Stroke::new(1.0, color32(style.palette.blue))
    } else {
        egui::Stroke::new(1.0, color32(style.palette.edge_soft))
    };
    ui.add_sized(
        [ui.available_width(), 48.0],
        egui::Button::new(
            egui::RichText::new(format!("{}   {}", page.number(), page.label()))
                .size(13.0)
                .strong()
                .color(if selected {
                    color32(style.palette.text)
                } else {
                    color32(style.palette.text_dim)
                }),
        )
        .fill(fill)
        .stroke(stroke)
        .rounding(egui::Rounding::same(8.0)),
    )
}
pub(in crate::startup_window::egui_presenter) fn engine_toggle(
    ui: &mut egui::Ui,
    value: &mut bool,
    label: &str,
) -> bool {
    ui.checkbox(value, label).changed()
}
pub(in crate::startup_window::egui_presenter) fn compact_choice_button(
    ui: &mut egui::Ui,
    label: &str,
    selected: bool,
) -> egui::Response {
    let palette = north_star_bootstrap_ui_style().palette;
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .size(11.5)
                .strong()
                .color(if selected {
                    color32(palette.text)
                } else {
                    color32(palette.text_dim)
                }),
        )
        .fill(if selected {
            color32(palette.panel_active)
        } else {
            color32(palette.bg_deep)
        })
        .stroke(egui::Stroke::new(
            1.0,
            if selected {
                color32(palette.blue)
            } else {
                color32(palette.edge_soft)
            },
        ))
        .rounding(egui::Rounding::same(7.0)),
    )
}
pub(in crate::startup_window::egui_presenter) fn preset_choice_button(
    ui: &mut egui::Ui,
    preset: GraphicsPreset,
    selected: bool,
) -> egui::Response {
    let palette = north_star_bootstrap_ui_style().palette;
    ui.add_sized(
        [118.0, 56.0],
        egui::Button::new(
            egui::RichText::new(preset.label())
                .size(13.0)
                .strong()
                .color(if selected {
                    color32(palette.text)
                } else {
                    color32(palette.text_dim)
                }),
        )
        .fill(if selected {
            color32(palette.panel_active)
        } else {
            color32(palette.bg_deep)
        })
        .stroke(egui::Stroke::new(
            1.0,
            if selected {
                color32(palette.blue_bright)
            } else {
                color32(palette.edge_soft)
            },
        ))
        .rounding(egui::Rounding::same(9.0)),
    )
}
pub(in crate::startup_window::egui_presenter) fn primary_button(
    ui: &mut egui::Ui,
    label: &str,
) -> egui::Response {
    let palette = north_star_bootstrap_ui_style().palette;
    ui.add_sized(
        [166.0, 38.0],
        egui::Button::new(
            egui::RichText::new(label)
                .size(12.5)
                .strong()
                .color(color32(palette.bg_deep)),
        )
        .fill(color32(palette.blue_bright))
        .stroke(egui::Stroke::new(1.0, color32(palette.blue_bright)))
        .rounding(egui::Rounding::same(8.0)),
    )
}
pub(in crate::startup_window::egui_presenter) fn secondary_button(
    ui: &mut egui::Ui,
    label: &str,
) -> egui::Response {
    let palette = north_star_bootstrap_ui_style().palette;
    ui.add_sized(
        [112.0, 36.0],
        egui::Button::new(
            egui::RichText::new(label)
                .size(11.5)
                .strong()
                .color(color32(palette.text_dim)),
        )
        .fill(color32(palette.bg_deep))
        .stroke(egui::Stroke::new(1.0, color32(palette.edge)))
        .rounding(egui::Rounding::same(8.0)),
    )
}
