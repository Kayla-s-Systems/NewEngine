use super::*;

pub(super) enum PluginEntryAction {
    None,
    OpenConfig,
    ToggleEnabled,
}

pub(super) fn plugin_module_entry(ui: &mut egui::Ui, tab: &PluginTab, selected: bool) -> PluginEntryAction {
    let fill = if selected && tab.enabled {
        egui::Color32::from_rgb(22, 34, 56)
    } else if selected {
        egui::Color32::from_rgb(42, 24, 34)
    } else if tab.enabled {
        egui::Color32::from_rgb(12, 17, 26)
    } else {
        egui::Color32::from_rgb(26, 13, 18)
    };

    let mut status_response: Option<egui::Response> = None;
    let response = egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(11))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(32, 40, 55)))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icon(ui, plugin_icon(tab), 20.0, egui::Color32::from_rgb(145, 161, 187));
                let status_width = 86.0;
                let title_width = (ui.available_width() - status_width - 10.0).clamp(70.0, 240.0);
                ui.add_sized(
                    [title_width, 24.0],
                    egui::Label::new(
                        egui::RichText::new(&tab.title)
                            .size(14.0)
                            .color(egui::Color32::from_rgb(219, 228, 245)),
                    ),
                );
                ui.add_space((ui.available_width() - status_width).max(0.0));
                status_response = Some(plugin_status_toggle(ui, tab.enabled));
            });
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);

    if response.hovered() {
        ui.painter().rect_filled(response.rect.expand(1.0), egui::CornerRadius::same(11), egui::Color32::from_rgba_unmultiplied(82, 158, 255, 22));
        ui.painter().rect_stroke(response.rect.expand(1.0), egui::CornerRadius::same(11), egui::Stroke::new(1.2, ACCENT_BLUE_BRIGHT), egui::StrokeKind::Inside);
    }

    if let Some(status) = status_response {
        if status.clicked() {
            return PluginEntryAction::ToggleEnabled;
        }
        if status.hovered() {
            return PluginEntryAction::None;
        }
    }

    if response.clicked() {
        PluginEntryAction::OpenConfig
    } else {
        PluginEntryAction::None
    }
}

pub(super) fn plugin_status_toggle(ui: &mut egui::Ui, enabled: bool) -> egui::Response {
    let (dot_color, status) = if enabled {
        (egui::Color32::from_rgb(107, 231, 113), "Enabled")
    } else {
        (egui::Color32::from_rgb(231, 79, 86), "Disabled")
    };
    let fill = if enabled {
        egui::Color32::from_rgb(14, 36, 24)
    } else {
        egui::Color32::from_rgb(42, 16, 22)
    };
    let stroke = if enabled {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(63, 138, 75))
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(150, 56, 68))
    };

    let response = egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(10))
        .stroke(stroke)
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 3.7, dot_color);
                ui.painter().circle_filled(rect.center(), 7.0, egui::Color32::from_rgba_unmultiplied(dot_color.r(), dot_color.g(), dot_color.b(), 25));
                ui.label(egui::RichText::new(status).size(10.8).strong().color(egui::Color32::from_rgb(222, 232, 246)));
            });
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);

    if response.hovered() {
        ui.painter().rect_stroke(response.rect.expand(1.0), egui::CornerRadius::same(11), egui::Stroke::new(1.2, egui::Color32::from_rgb(128, 190, 255)), egui::StrokeKind::Inside);
    }

    response
}

pub(super) fn plugin_icon(tab: &PluginTab) -> IconKind {
    let text = format!("{} {}", tab.title.to_lowercase(), tab.plugin_id.to_lowercase());
    if text.contains("render") || text.contains("vulkan") { IconKind::Renderer }
    else if text.contains("physics") { IconKind::Physics }
    else if text.contains("audio") { IconKind::Audio }
    else if text.contains("input") { IconKind::Input }
    else if text.contains("ui") { IconKind::Ui }
    else if text.contains("animation") { IconKind::Animation }
    else if text.contains("script") || text.contains("lua") { IconKind::Script }
    else if text.contains("asset") { IconKind::Folder }
    else { IconKind::Core }
}
