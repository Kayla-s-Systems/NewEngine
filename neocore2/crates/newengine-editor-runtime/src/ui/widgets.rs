#![forbid(unsafe_op_in_unsafe_fn)]

use egui;

use super::theme;

#[inline]
pub(crate) fn panel_title(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.heading(title);
        if !subtitle.is_empty() {
            ui.separator();
            ui.label(egui::RichText::new(subtitle).small().weak());
        }
    });
}

#[inline]
pub(crate) fn section_card(ui: &mut egui::Ui, title: &str, add_body: impl FnOnce(&mut egui::Ui)) {
    theme::section_frame(ui).show(ui, |ui| {
        ui.label(egui::RichText::new(title).strong());
        ui.add_space(6.0);
        add_body(ui);
    });
}

#[inline]
pub(crate) fn filter_chip(ui: &mut egui::Ui, value: &mut bool, label: &str) -> egui::Response {
    let fill = if *value {
        ui.visuals().selection.bg_fill.gamma_multiply(0.28)
    } else {
        ui.visuals().widgets.inactive.bg_fill
    };
    let stroke = if *value {
        ui.visuals().selection.stroke
    } else {
        ui.visuals().widgets.inactive.bg_stroke
    };
    let response = ui.add(
        egui::Button::new(label)
            .fill(fill)
            .stroke(stroke)
            .corner_radius(egui::CornerRadius::same(12))
            .min_size(egui::vec2(0.0, 22.0)),
    );
    if response.clicked() {
        *value = !*value;
    }
    response
}

#[inline]
pub(crate) fn search_field(ui: &mut egui::Ui, value: &mut String, hint: &str) {
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .desired_width(f32::INFINITY),
    );
}

#[inline]
pub(crate) fn stat_row(ui: &mut egui::Ui, label: &str, value: impl Into<egui::WidgetText>) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).small().weak());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(value);
        });
    });
}
