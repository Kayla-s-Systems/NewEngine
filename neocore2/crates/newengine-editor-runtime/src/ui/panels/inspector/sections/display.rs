#![forbid(unsafe_op_in_unsafe_fn)]

use crate::gameplay::{DisplayMode, DisplayVisibility};
use crate::ui::{property_grid, schema, EditorUiBuild};

pub(crate) fn draw(
    me: &mut EditorUiBuild,
    ui: &mut egui::Ui,
    selection_ctx: &schema::SelectionContext,
) {
    let entity = selection_ctx.entity;
    let fields = schema::property_fields(me, selection_ctx, schema::PropertySectionId::Display);
    property_grid::section_card_descriptor(ui, "Display", &fields, |ui, field| {
        property_grid::field_label(ui, field);
        let scene = me.scene_bridge.scene();
        let scene = scene.read();
        let world = scene.world();
        let current = world
            .get::<DisplayVisibility>(entity)
            .copied()
            .unwrap_or_default()
            .mode;
        let mut next = current;
        egui::ComboBox::from_id_salt(("display_visibility", entity.stable_u64()))
            .selected_text(match next {
                DisplayMode::Both => "Editor + Game",
                DisplayMode::EditorOnly => "Editor only",
                DisplayMode::GameOnly => "Game only",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut next, DisplayMode::Both, "Editor + Game");
                ui.selectable_value(&mut next, DisplayMode::EditorOnly, "Editor only");
                ui.selectable_value(&mut next, DisplayMode::GameOnly, "Game only");
            });
        if next != current {
            me.apply_display_mode_with_history(entity, current, next);
        }
        property_grid::end_row(ui);
    });
}
