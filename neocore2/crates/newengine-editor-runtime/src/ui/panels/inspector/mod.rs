#![forbid(unsafe_op_in_unsafe_fn)]

mod components;
mod sections;

use egui;

use super::super::schema;
use super::super::theme;
use super::super::util;
use super::super::EditorUiBuild;

#[inline]
fn has_section(
    sections: &[schema::PropertySectionSchema],
    id: schema::PropertySectionId,
) -> bool {
    sections.iter().any(|section| section.id == id)
}

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    let max_width = util::details_max_width(ctx, me.layout.show_left_toolbar, me.layout.show_outliner);

    egui::SidePanel::right("inspector")
        .resizable(true)
        .default_width(300.0)
        .min_width(260.0)
        .max_width(max_width)
        .show(ctx, |ui| {
            draw_content(me, ui);
        });
}

pub(crate) fn draw_content(me: &mut EditorUiBuild, ui: &mut egui::Ui) {
    sections::summary::draw_header(me, ui);

    let selected = me.editor.selection.primary();
    let Some(entity) = selected else {
        theme::section_frame(ui).show(ui, |ui| {
            ui.label(egui::RichText::new("Nothing selected").strong());
            ui.label("Select an actor in the viewport or outliner to inspect and edit it.");
        });
        return;
    };

    if me.selected_entity_cached != Some(entity) {
        me.refresh_inspector_cache(entity);
    }

    let selection_ctx = schema::build_selection_context(me, entity);
    let sections = schema::property_sections(me, &selection_ctx);

    sections::summary::draw_summary(me, ui, &selection_ctx);
    sections::summary::draw_context_actions(me, ui, &selection_ctx);

    if has_section(&sections, schema::PropertySectionId::Transform) {
        sections::transform::draw(me, ui, &selection_ctx);
    }
    if has_section(&sections, schema::PropertySectionId::Display) {
        sections::display::draw(me, ui, &selection_ctx);
    }
    if has_section(&sections, schema::PropertySectionId::Gameplay) {
        sections::gameplay::draw(me, ui, &selection_ctx);
    }
    if has_section(&sections, schema::PropertySectionId::Collision) {
        sections::collision::draw(me, ui, &selection_ctx);
    }
    if has_section(&sections, schema::PropertySectionId::Primitive) {
        sections::primitive::draw(me, ui, &selection_ctx);
    }
    if has_section(&sections, schema::PropertySectionId::Lighting) {
        sections::lighting::draw(me, ui, &selection_ctx);
    }
    if has_section(&sections, schema::PropertySectionId::Material) {
        sections::material::draw(me, ui, &selection_ctx);
    }
}
