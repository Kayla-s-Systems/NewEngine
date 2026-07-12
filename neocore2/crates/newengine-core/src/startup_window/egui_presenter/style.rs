#![forbid(unsafe_op_in_unsafe_fn)]

use eframe::egui;
use newengine_loading_api::bootstrap_ui::{north_star_bootstrap_ui_style, BootstrapUiRgb};

use super::model::{RenderPressure, StatusKind};

pub(super) fn configure_style(ctx: &egui::Context) {
    let style = north_star_bootstrap_ui_style();
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = color32(style.palette.bg);
    visuals.window_fill = color32(style.palette.panel);
    visuals.extreme_bg_color = color32(style.palette.bg_deep);
    visuals.faint_bg_color = color32(style.palette.panel);
    visuals.selection.bg_fill = color32(style.palette.blue);
    visuals.selection.stroke = egui::Stroke::new(1.0, color32(style.palette.blue_bright));
    visuals.widgets.inactive.bg_fill = color32(style.palette.panel);
    visuals.widgets.inactive.weak_bg_fill = color32(style.palette.panel);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, color32(style.palette.edge_soft));
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, color32(style.palette.text_dim));
    visuals.widgets.hovered.bg_fill = color32(style.palette.panel_active);
    visuals.widgets.hovered.weak_bg_fill = color32(style.palette.panel_active);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, color32(style.palette.blue));
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, color32(style.palette.text));
    visuals.widgets.active.bg_fill = color32(style.palette.panel_active);
    visuals.widgets.active.weak_bg_fill = color32(style.palette.panel_active);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, color32(style.palette.blue_bright));
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, color32(style.palette.text));
    ctx.set_visuals(visuals);

    let mut style_mut = (*ctx.style()).clone();
    style_mut.spacing.item_spacing = egui::vec2(9.0, 8.0);
    style_mut.spacing.button_padding = egui::vec2(12.0, 8.0);
    style_mut.spacing.indent = 16.0;
    style_mut
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(13.5));
    style_mut
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
    style_mut
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::monospace(11.5));
    ctx.set_style(style_mut);
}

pub(super) fn color32(rgb: BootstrapUiRgb) -> egui::Color32 {
    egui::Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

pub(super) fn pressure_color(pressure: RenderPressure) -> egui::Color32 {
    let palette = north_star_bootstrap_ui_style().palette;
    match pressure {
        RenderPressure::Low => color32(palette.ok),
        RenderPressure::Balanced => color32(palette.blue_bright),
        RenderPressure::High => color32(palette.warn),
        RenderPressure::Extreme => color32(palette.fail),
    }
}

pub(super) fn status_color(kind: StatusKind) -> egui::Color32 {
    let palette = north_star_bootstrap_ui_style().palette;
    match kind {
        StatusKind::Info => color32(palette.text_dim),
        StatusKind::Warning => color32(palette.warn),
        StatusKind::Error => color32(palette.fail),
    }
}
