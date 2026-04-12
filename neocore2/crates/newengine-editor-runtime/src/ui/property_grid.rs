#![forbid(unsafe_op_in_unsafe_fn)]

use egui;

use crate::ui::schema::{FieldEditorKind, PropertyFieldSchema};

pub(crate) fn grid(ui: &mut egui::Ui, id: impl std::hash::Hash, add: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing(egui::vec2(10.0, 6.0))
        .striped(false)
        .show(ui, |ui| add(ui));
}

pub(crate) fn section(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.collapsing(title, |ui| {
        grid(ui, ("property_grid", title), add);
    });
}

pub(crate) fn section_card(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    crate::ui::theme::section_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(egui::RichText::new(title).strong());
        ui.add_space(6.0);
        add(ui);
    });
}

pub(crate) fn section_card_descriptor(
    ui: &mut egui::Ui,
    title: &str,
    fields: &[PropertyFieldSchema],
    mut render: impl FnMut(&mut egui::Ui, &PropertyFieldSchema),
) {
    section_card(ui, title, |ui| {
        grid(ui, ("property_grid_descriptor", title), |ui| {
            for field in fields.iter().filter(|it| it.visible) {
                render(ui, field);
            }
        });
    });
}

pub(crate) fn label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).weak());
}

pub(crate) fn field_label(ui: &mut egui::Ui, field: &PropertyFieldSchema) {
    let suffix = match field.editor {
        FieldEditorKind::ReadOnlyText => "",
        FieldEditorKind::Vec3 => "",
        FieldEditorKind::Toggle => "",
        FieldEditorKind::EnumChoice => "",
        FieldEditorKind::Color3 => "",
        FieldEditorKind::Color4 => "",
        FieldEditorKind::Scalar => "",
        FieldEditorKind::MaterialChoice => "",
    };
    label(ui, &format!("{}{}", field.label, suffix));
}

pub(crate) fn end_row(ui: &mut egui::Ui) {
    ui.end_row();
}

pub(crate) fn vec3_row(
    ui: &mut egui::Ui,
    label_text: &str,
    value: &mut [f32; 3],
    speed: f32,
) -> bool {
    label(ui, label_text);
    let mut changed = false;
    ui.horizontal(|ui| {
        changed |= ui.add(egui::DragValue::new(&mut value[0]).speed(speed)).changed();
        changed |= ui.add(egui::DragValue::new(&mut value[1]).speed(speed)).changed();
        changed |= ui.add(egui::DragValue::new(&mut value[2]).speed(speed)).changed();
    });
    end_row(ui);
    changed
}

pub(crate) fn checkbox_row(ui: &mut egui::Ui, label_text: &str, value: &mut bool) -> bool {
    label(ui, label_text);
    let changed = ui.checkbox(value, "").changed();
    end_row(ui);
    changed
}
