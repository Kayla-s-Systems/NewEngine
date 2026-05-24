use super::*;

pub(super) fn status_dot(ui: &mut egui::Ui, enabled: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
    let color = if enabled { ACCENT_GREEN } else { egui::Color32::from_rgb(231, 79, 86) };
    ui.painter().circle_filled(rect.center(), 4.5, color);
    ui.painter().circle_filled(rect.center(), 8.0, egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 28));
}

pub(super) fn ready_status_pill(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(13, 28, 22))
        .corner_radius(egui::CornerRadius::same(10))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(42, 88, 55)))
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                status_dot(ui, true);
                ui.label(egui::RichText::new("Ready to Launch").size(14.0).strong().color(ACCENT_GREEN));
            });
        });
}


pub(super) fn action_button(ui: &mut egui::Ui, kind: IconKind, text: &str, width: f32, primary: bool) -> egui::Response {
    let fill = if primary {
        egui::Color32::from_rgb(26, 88, 185)
    } else {
        egui::Color32::from_rgb(18, 23, 32)
    };
    let stroke = if primary {
        egui::Stroke::new(1.1, egui::Color32::from_rgb(85, 170, 255))
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 53, 72))
    };
    let response = egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(stroke)
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_min_width(width - 32.0);
            ui.horizontal(|ui| {
                icon(ui, kind, 20.0, if primary { egui::Color32::WHITE } else { egui::Color32::from_rgb(174, 187, 210) });
                ui.add_space(8.0);
                ui.label(egui::RichText::new(text).size(if primary { 16.0 } else { 13.5 }).strong().color(if primary { egui::Color32::WHITE } else { TEXT_PRIMARY }));
            });
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.hovered() {
        ui.painter().rect_stroke(response.rect.expand(1.0), egui::CornerRadius::same(9), egui::Stroke::new(1.2, ACCENT_BLUE_BRIGHT), egui::StrokeKind::Inside);
    }
    response
}

pub(super) fn nav_button(ui: &mut egui::Ui, selected: &mut usize, index: usize, title: &str, subtitle: &str) {
    let is_selected = *selected == index;
    let fill = if is_selected {
        egui::Color32::from_rgb(35, 48, 74)
    } else {
        egui::Color32::from_rgb(17, 20, 28)
    };
    let response = egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_width(200.0);
            ui.label(egui::RichText::new(title).size(15.0).strong().color(egui::Color32::from_rgb(224, 232, 248)));
            ui.label(egui::RichText::new(subtitle).size(11.0).color(egui::Color32::from_rgb(128, 139, 160)));
        })
        .response;
    if response.interact(egui::Sense::click()).clicked() {
        *selected = index;
    }
    ui.add_space(6.0);
}

pub(super) fn section_header(ui: &mut egui::Ui, title: &str, detail: &str) {
    ui.label(egui::RichText::new(title).size(24.0).strong().color(egui::Color32::from_rgb(235, 240, 252)));
    ui.label(egui::RichText::new(detail).size(13.0).color(egui::Color32::from_rgb(146, 158, 184)));
    ui.add_space(14.0);
}

pub(super) fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(20, 24, 33))
        .corner_radius(egui::CornerRadius::same(16))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(38, 45, 60)))
        .inner_margin(egui::Margin::same(16))
        .show(ui, add);
}

pub(super) fn label(text: &str) -> egui::RichText {
    egui::RichText::new(text).size(13.0).color(egui::Color32::from_rgb(176, 187, 208))
}

pub(super) fn text_row(ui: &mut egui::Ui, title: &str, value: &mut String) {
    ui.horizontal(|ui| {
        let label_width = if ui.available_width() < 330.0 { 94.0 } else { 128.0 };
        ui.add_sized([label_width, 22.0], egui::Label::new(label(title)));
        let width = (ui.available_width() - 4.0).clamp(120.0, 360.0);
        styled_text_edit(ui, value, width, 28.0);
    });
}

pub(super) fn select_row(ui: &mut egui::Ui, title: &str, selected: &mut String, options: &[(&str, &str)]) {
    ui.horizontal(|ui| {
        if !title.is_empty() {
            let label_width = if ui.available_width() < 330.0 { 94.0 } else { 128.0 };
            ui.add_sized([label_width, 22.0], egui::Label::new(label(title)));
        }
        let width = (ui.available_width() - 4.0).clamp(130.0, 300.0);
        styled_select_static(ui, title, selected, options, width);
    });
}

pub(super) fn styled_text_edit(ui: &mut egui::Ui, value: &mut String, width: f32, height: f32) -> egui::Response {
    let frame_response = egui::Frame::new()
        .fill(egui::Color32::from_rgb(9, 12, 18))
        .corner_radius(egui::CornerRadius::same(7))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 51, 70)))
        .inner_margin(egui::Margin::symmetric(10, 3))
        .show(ui, |ui| {
            ui.add_sized(
                [width, height],
                egui::TextEdit::singleline(value)
                    .frame(false)
                    .desired_width(width),
            )
        });
    let response = frame_response.inner;
    let stroke = if response.has_focus() {
        egui::Stroke::new(1.35, ACCENT_BLUE_BRIGHT)
    } else if response.hovered() || frame_response.response.hovered() {
        egui::Stroke::new(1.1, ACCENT_BLUE)
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 51, 70))
    };
    ui.painter().rect_stroke(frame_response.response.rect, egui::CornerRadius::same(7), stroke, egui::StrokeKind::Inside);
    response
}

pub(super) fn styled_multiline_edit(ui: &mut egui::Ui, value: &mut String, width: f32, height: f32) -> egui::Response {
    let frame_response = egui::Frame::new()
        .fill(egui::Color32::from_rgb(9, 12, 18))
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 51, 70)))
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.add_sized(
                [width, height],
                egui::TextEdit::multiline(value)
                    .frame(false)
                    .desired_width(width),
            )
        });
    let response = frame_response.inner;
    let stroke = if response.has_focus() {
        egui::Stroke::new(1.35, ACCENT_BLUE_BRIGHT)
    } else if response.hovered() || frame_response.response.hovered() {
        egui::Stroke::new(1.1, ACCENT_BLUE)
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 51, 70))
    };
    ui.painter().rect_stroke(frame_response.response.rect, egui::CornerRadius::same(8), stroke, egui::StrokeKind::Inside);
    response
}

pub(super) fn styled_select_static(ui: &mut egui::Ui, id_salt: &str, selected: &mut String, options: &[(&str, &str)], width: f32) {
    let current_label = options
        .iter()
        .find(|(value, _)| *value == selected.as_str())
        .map(|(_, label)| (*label).to_owned())
        .unwrap_or_else(|| selected.clone());
    let owned_options: Vec<(String, String)> = options
        .iter()
        .map(|(value, label)| ((*value).to_owned(), (*label).to_owned()))
        .collect();

    styled_select_box(ui, id_salt, selected, &current_label, &owned_options, width);
}

pub(super) fn styled_select_dynamic(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    selected: &mut String,
    options: &[SelectOption],
    width: f32,
) {
    let current_label = option_label(options, selected.as_str());
    let owned_options: Vec<(String, String)> = options
        .iter()
        .map(|option| (option.value.clone(), option.label.clone()))
        .collect();

    styled_select_box(ui, id_salt, selected, &current_label, &owned_options, width);
}

pub(super) fn styled_select_box(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    selected: &mut String,
    current_label: &str,
    options: &[(String, String)],
    width: f32,
) {
    let button_id = ui.make_persistent_id(("prestart_select", id_salt));
    let popup_id = button_id.with("popup");
    let area_id = popup_id.with("area");
    let mut open = ui.ctx().data(|data| data.get_temp::<bool>(popup_id).unwrap_or(false));

    let button_height = 34.0;
    let desired_size = egui::vec2(width, button_height);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

    if response.clicked() {
        open = !open;
        ui.ctx().data_mut(|data| data.insert_temp(popup_id, open));
    }

    let fill = if response.hovered() || open {
        egui::Color32::from_rgb(14, 20, 31)
    } else {
        egui::Color32::from_rgb(9, 12, 18)
    };
    let stroke = if open {
        egui::Stroke::new(1.25, ACCENT_BLUE_BRIGHT)
    } else if response.hovered() {
        egui::Stroke::new(1.1, ACCENT_BLUE)
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 51, 70))
    };

    ui.painter().rect_filled(rect, egui::CornerRadius::same(8), fill);
    ui.painter().rect_stroke(rect, egui::CornerRadius::same(8), stroke, egui::StrokeKind::Inside);
    ui.painter().rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(rect.right() - 34.0, rect.top() + 1.0),
            egui::pos2(rect.right() - 1.0, rect.bottom() - 1.0),
        ),
        egui::CornerRadius { nw: 0, ne: 8, sw: 0, se: 8 },
        egui::Color32::from_rgb(14, 18, 27),
    );
    ui.painter().vline(
        rect.right() - 34.0,
        (rect.top() + 4.0)..=(rect.bottom() - 4.0),
        egui::Stroke::new(1.0, egui::Color32::from_rgb(32, 42, 58)),
    );
    let label_clip = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 8.0, rect.top()),
        egui::pos2(rect.right() - 38.0, rect.bottom()),
    );
    ui.painter().with_clip_rect(label_clip).text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        current_label,
        egui::FontId::proportional(13.5),
        TEXT_PRIMARY,
    );
    paint_chevron(ui, egui::Rect::from_center_size(egui::pos2(rect.right() - 17.0, rect.center().y), egui::vec2(14.0, 14.0)), open);

    let mut popup_rect: Option<egui::Rect> = None;
    if open {
        let popup_pos = egui::pos2(rect.left(), rect.bottom() + 6.0);
        let popup = egui::Area::new(area_id)
            .order(egui::Order::Foreground)
            .fixed_pos(popup_pos)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(10, 14, 21))
                    .corner_radius(egui::CornerRadius::same(10))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(53, 78, 118)))
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.set_min_width(width);
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .max_height(240.0)
                            .show(ui, |ui| {
                                for (value, label) in options {
                                    if popup_option_row(ui, label, selected.as_str() == value.as_str()).clicked() {
                                        *selected = value.clone();
                                        open = false;
                                    }
                                    ui.add_space(2.0);
                                }
                            });
                    });
            });
        popup_rect = Some(popup.response.rect);
    }

    if open && ui.ctx().input(|i| i.pointer.any_pressed()) {
        if let Some(pointer_pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
            let clicked_button = rect.contains(pointer_pos);
            let clicked_popup = popup_rect.map(|r| r.contains(pointer_pos)).unwrap_or(false);
            if !clicked_button && !clicked_popup {
                open = false;
            }
        }
    }

    ui.ctx().data_mut(|data| {
        data.insert_temp(popup_id, open);
    });
}

pub(super) fn popup_option_row(ui: &mut egui::Ui, label: &str, is_selected: bool) -> egui::Response {
    let desired_size = egui::vec2(ui.available_width(), 30.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let fill = if is_selected {
        egui::Color32::from_rgb(24, 45, 79)
    } else if response.hovered() {
        egui::Color32::from_rgb(18, 27, 41)
    } else {
        egui::Color32::TRANSPARENT
    };
    let stroke = if is_selected {
        egui::Stroke::new(1.0, ACCENT_BLUE)
    } else if response.hovered() {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(55, 81, 120))
    } else {
        egui::Stroke::NONE
    };
    ui.painter().rect_filled(rect, egui::CornerRadius::same(7), fill);
    if stroke != egui::Stroke::NONE {
        ui.painter().rect_stroke(rect, egui::CornerRadius::same(7), stroke, egui::StrokeKind::Inside);
    }
    if is_selected {
        ui.painter().circle_filled(egui::pos2(rect.left() + 12.0, rect.center().y), 3.5, ACCENT_BLUE_BRIGHT);
    }
    ui.painter().text(
        egui::pos2(rect.left() + if is_selected { 22.0 } else { 12.0 }, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.8),
        if is_selected { egui::Color32::WHITE } else { TEXT_PRIMARY },
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub(super) fn paint_chevron(ui: &mut egui::Ui, rect: egui::Rect, open: bool) {
    let stroke = egui::Stroke::new(1.7, egui::Color32::from_rgb(198, 210, 232));
    let (a, b, c) = if open {
        (
            egui::pos2(rect.left() + 3.0, rect.bottom() - 4.5),
            egui::pos2(rect.center().x, rect.top() + 4.5),
            egui::pos2(rect.right() - 3.0, rect.bottom() - 4.5),
        )
    } else {
        (
            egui::pos2(rect.left() + 3.0, rect.top() + 4.5),
            egui::pos2(rect.center().x, rect.bottom() - 4.5),
            egui::pos2(rect.right() - 3.0, rect.top() + 4.5),
        )
    };
    ui.painter().line_segment([a, b], stroke);
    ui.painter().line_segment([b, c], stroke);
}

pub(super) fn launcher_card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(15, 19, 27))
        .corner_radius(egui::CornerRadius::same(14))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(33, 42, 58)))
        .inner_margin(egui::Margin::same(16))
        .show(ui, add);
}

pub(super) fn card_title(ui: &mut egui::Ui, icon_kind: IconKind, title: &str, pill: Option<&str>) {
    ui.horizontal(|ui| {
        icon(ui, icon_kind, 22.0, egui::Color32::from_rgb(91, 167, 255));
        ui.label(egui::RichText::new(title).size(13.0).strong().color(egui::Color32::from_rgb(91, 167, 255)));
        if let Some(pill) = pill {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(23, 29, 40))
                    .corner_radius(egui::CornerRadius::same(12))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(42, 52, 70)))
                    .inner_margin(egui::Margin::symmetric(12, 4))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(pill).size(12.0).color(egui::Color32::from_rgb(210, 220, 238)));
                    });
            });
        }
    });
}

pub(super) fn icon_box(ui: &mut egui::Ui, kind: IconKind) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(18, 23, 33))
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 53, 72)))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            icon(ui, kind, 24.0, egui::Color32::from_rgb(154, 170, 196));
        });
}

pub(super) fn switch_only(ui: &mut egui::Ui, value: &mut bool) -> egui::Response {
    let desired_size = egui::vec2(52.0, 28.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }

    let t = ui.ctx().animate_bool(response.id, *value);
    let track_fill = if *value {
        egui::Color32::from_rgb(32, 86, 170)
    } else {
        egui::Color32::from_rgb(25, 31, 43)
    };
    let track_fill = egui::Color32::from_rgba_unmultiplied(track_fill.r(), track_fill.g(), track_fill.b(), 255);
    let stroke_color = if response.hovered() { ACCENT_BLUE_BRIGHT } else if *value { ACCENT_BLUE } else { egui::Color32::from_rgb(58, 69, 89) };
    let glow_color = if *value { egui::Color32::from_rgba_unmultiplied(74, 150, 255, 28) } else { egui::Color32::from_rgba_unmultiplied(104, 116, 138, 14) };

    ui.painter().rect_filled(rect.expand(1.5), egui::CornerRadius::same(14), glow_color);
    ui.painter().rect_filled(rect, egui::CornerRadius::same(14), track_fill);
    ui.painter().rect_stroke(rect, egui::CornerRadius::same(14), egui::Stroke::new(1.2, stroke_color), egui::StrokeKind::Inside);

    let knob_radius = 10.0;
    let knob_x = egui::lerp((rect.left() + 14.0)..=(rect.right() - 14.0), t);
    let knob_center = egui::pos2(knob_x, rect.center().y);
    if *value {
        ui.painter().circle_filled(knob_center, 13.0, egui::Color32::from_rgba_unmultiplied(84, 163, 255, 38));
    }
    ui.painter().circle_filled(knob_center, knob_radius, egui::Color32::from_rgb(244, 248, 255));
    ui.painter().circle_stroke(knob_center, knob_radius, egui::Stroke::new(1.0, egui::Color32::from_rgb(188, 205, 236)));

    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub(super) fn switch_chip(ui: &mut egui::Ui, label_text: &str, value: &mut bool) -> egui::Response {
    egui::Frame::new()
        .fill(if *value { egui::Color32::from_rgb(17, 32, 58) } else { egui::Color32::from_rgb(14, 18, 26) })
        .corner_radius(egui::CornerRadius::same(10))
        .stroke(egui::Stroke::new(1.0, if *value { ACCENT_BLUE } else { egui::Color32::from_rgb(40, 50, 67) }))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(label_text).size(12.5).color(TEXT_PRIMARY));
                switch_only(ui, value);
            });
        })
        .response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub(super) fn switch_chip_value(ui: &mut egui::Ui, label_text: &str, value: &mut bool) -> bool {
    let before = *value;
    switch_chip(ui, label_text, value);
    before != *value
}

pub(super) fn switch_row(ui: &mut egui::Ui, title: &str, subtitle: &str, value: &mut bool) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(title).size(13.0).strong().color(TEXT_PRIMARY));
            if !subtitle.is_empty() {
                ui.label(egui::RichText::new(subtitle).size(11.0).color(TEXT_MUTED));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            switch_only(ui, value);
        });
    });
}

pub(super) fn switch_row_change(ui: &mut egui::Ui, title: &str, subtitle: &str, value: &mut bool) -> bool {
    let before = *value;
    switch_row(ui, title, subtitle, value);
    before != *value
}

pub(super) fn icon_button_box(ui: &mut egui::Ui, kind: IconKind) -> egui::Response {
    let response = egui::Frame::new()
        .fill(egui::Color32::from_rgb(16, 20, 29))
        .corner_radius(egui::CornerRadius::same(7))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 55, 72)))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            icon(ui, kind, 18.0, egui::Color32::from_rgb(170, 184, 207));
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.hovered() {
        ui.painter().rect_stroke(response.rect.expand(1.0), egui::CornerRadius::same(8), egui::Stroke::new(1.0, egui::Color32::from_rgb(75, 145, 235)), egui::StrokeKind::Inside);
    }
    response
}

pub(super) fn beveled_button(ui: &mut egui::Ui, kind: IconKind, text: &str) -> egui::Response {
    let response = egui::Frame::new()
        .fill(egui::Color32::from_rgb(20, 25, 34))
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(43, 53, 72)))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                icon(ui, kind, 17.0, egui::Color32::from_rgb(164, 179, 205));
                ui.label(egui::RichText::new(text).size(12.5).color(egui::Color32::from_rgb(210, 219, 236)));
            });
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.hovered() {
        ui.painter().rect_stroke(response.rect.expand(1.0), egui::CornerRadius::same(9), egui::Stroke::new(1.0, egui::Color32::from_rgb(74, 140, 225)), egui::StrokeKind::Inside);
    }
    response
}

pub(super) fn segmented_screen_mode(ui: &mut egui::Ui, selected: &mut String) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        for (value, label) in [
            ("windowed", "Windowed"),
            ("borderless", "Borderless"),
            ("exclusive_fullscreen", "Fullscreen"),
        ] {
            let is_selected = selected.as_str() == value;
            let fill = if is_selected {
                egui::Color32::from_rgb(28, 56, 98)
            } else {
                egui::Color32::from_rgb(17, 21, 30)
            };
            let stroke = if is_selected {
                egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 150, 255))
            } else {
                egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 49, 66))
            };

            let response = egui::Frame::new()
                .fill(fill)
                .corner_radius(egui::CornerRadius::same(7))
                .stroke(stroke)
                .inner_margin(egui::Margin::symmetric(16, 8))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(label)
                            .size(12.5)
                            .color(egui::Color32::from_rgb(218, 227, 246)),
                    );
                })
                .response
                .interact(egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);

            if response.clicked() {
                *selected = value.to_owned();
                changed = true;
            }

            if response.hovered() {
                ui.painter().rect_stroke(
                    response.rect.expand(1.0),
                    egui::CornerRadius::same(8),
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(82, 160, 255)),
                    egui::StrokeKind::Inside,
                );
            }
        }
    });

    changed
}