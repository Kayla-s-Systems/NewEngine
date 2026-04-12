#![forbid(unsafe_op_in_unsafe_fn)]

use egui;

#[inline]
pub(crate) fn apply_editor_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = egui::Color32::from_rgb(24, 26, 30);
    visuals.window_fill = egui::Color32::from_rgb(27, 29, 34);
    visuals.faint_bg_color = egui::Color32::from_rgb(34, 37, 42);
    visuals.extreme_bg_color = egui::Color32::from_rgb(18, 20, 24);
    visuals.code_bg_color = egui::Color32::from_rgb(20, 22, 27);
    visuals.selection.bg_fill = egui::Color32::from_rgb(196, 138, 58);
    visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(250, 232, 198));
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(31, 33, 38);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(52, 56, 64));
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(39, 42, 48);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 64, 72));
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(52, 56, 64);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 106, 118));
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(70, 74, 82);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(214, 170, 92));
    visuals.widgets.open.bg_fill = egui::Color32::from_rgb(46, 50, 57);
    visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(98, 104, 115));
    visuals.window_shadow.color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 90);
    visuals.popup_shadow.color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 90);

    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.menu_margin = egui::Margin::same(6);
    style.spacing.window_margin = egui::Margin::same(8);
    style.spacing.combo_width = 120.0;
    style.spacing.indent = 14.0;
    style.spacing.interact_size = egui::vec2(24.0, 24.0);

    ctx.set_style(style);
}

#[inline]
pub(crate) fn section_frame(ui: &egui::Ui) -> egui::Frame {
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::same(6))
}

#[inline]
pub(crate) fn toolbar_button(ui: &mut egui::Ui, selected: bool, text: &str) -> egui::Response {
    let fill = if selected {
        ui.visuals().selection.bg_fill
    } else {
        ui.visuals().widgets.inactive.bg_fill
    };
    let stroke = if selected {
        ui.visuals().selection.stroke
    } else {
        ui.visuals().widgets.inactive.bg_stroke
    };
    ui.add(
        egui::Button::new(text)
            .fill(fill)
            .stroke(stroke)
            .corner_radius(egui::CornerRadius::same(5))
            .min_size(egui::vec2(0.0, 24.0)),
    )
}


#[inline]
pub(crate) fn selectable_row(
    ui: &mut egui::Ui,
    selected: bool,
    title: &str,
    subtitle: &str,
) -> egui::Response {
    let fill = if selected {
        ui.visuals().selection.bg_fill.gamma_multiply(0.22)
    } else {
        ui.visuals().faint_bg_color
    };
    let stroke = if selected {
        ui.visuals().selection.stroke
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke
    };
    section_frame(ui)
        .fill(fill)
        .stroke(stroke)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(title).strong());
                if !subtitle.is_empty() {
                    ui.label(egui::RichText::new(subtitle).small().weak());
                }
            });
        })
        .response
}

#[inline]
pub(crate) fn tune_dock_style(style: &mut egui_dock::Style, visuals: &egui::Visuals) {
    style.dock_area_padding = Some(egui::Margin::same(4));
    style.separator.width = 1.0;
    style.separator.color_idle = visuals.widgets.noninteractive.bg_stroke.color;
    style.separator.color_hovered = visuals.selection.bg_fill;
    style.separator.color_dragged = visuals.selection.stroke.color;
    style.tab_bar.bg_fill = visuals.panel_fill;
    style.tab_bar.height = 28.0;
    style.tab_bar.hline_color = visuals.widgets.noninteractive.bg_stroke.color;
    style.tab_bar.fill_tab_bar = false;
    style.tab.active.bg_fill = visuals.faint_bg_color;
    style.tab.active.text_color = visuals.text_color();
    style.tab.active.outline_color = visuals.selection.stroke.color;
    style.tab.inactive.bg_fill = visuals.widgets.inactive.bg_fill;
    style.tab.inactive.text_color = visuals.weak_text_color();
    style.tab.inactive.outline_color = visuals.widgets.inactive.bg_stroke.color;
    style.tab.hovered.bg_fill = visuals.widgets.hovered.bg_fill;
    style.tab.hovered.text_color = visuals.text_color();
    style.tab.hovered.outline_color = visuals.widgets.hovered.bg_stroke.color;
    style.tab.focused = style.tab.active.clone();
    style.tab.inactive_with_kb_focus = style.tab.inactive.clone();
    style.tab.active_with_kb_focus = style.tab.active.clone();
    style.tab.focused_with_kb_focus = style.tab.active.clone();
    style.tab.tab_body.bg_fill = visuals.panel_fill;
    style.tab.tab_body.stroke = visuals.widgets.noninteractive.bg_stroke;
    style.tab.tab_body.inner_margin = egui::Margin::same(6);
    style.tab.spacing = 4.0;
    style.tab.minimum_width = Some(72.0);
    style.buttons.add_tab_bg_fill = visuals.widgets.inactive.bg_fill;
    style.buttons.add_tab_color = visuals.text_color();
    style.buttons.add_tab_active_color = visuals.text_color();
    style.buttons.close_tab_bg_fill = visuals.widgets.inactive.bg_fill;
    style.buttons.close_tab_color = visuals.text_color();
    style.buttons.close_tab_active_color = visuals.text_color();
    style.overlay.selection_color = visuals.selection.bg_fill.gamma_multiply(0.32);
}
