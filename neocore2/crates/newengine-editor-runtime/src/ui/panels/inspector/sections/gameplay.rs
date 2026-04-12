#![forbid(unsafe_op_in_unsafe_fn)]

use crate::gameplay::PlayerActor;
use crate::ui::{property_grid, schema, EditorUiBuild};

pub(crate) fn draw(
    me: &mut EditorUiBuild,
    ui: &mut egui::Ui,
    selection_ctx: &schema::SelectionContext,
) {
    let entity = selection_ctx.entity;
    let fields = schema::property_fields(me, selection_ctx, schema::PropertySectionId::Gameplay);
    property_grid::section_card_descriptor(ui, "Gameplay", &fields, |ui, field| {
        property_grid::field_label(ui, field);
        let scene = me.scene_bridge.scene();
        let scene = scene.read();
        let world = scene.world();
        if world.get::<PlayerActor>(entity).is_some() {
            ui.label("Player");
        } else {
            ui.label("Scene actor");
        }
        property_grid::end_row(ui);
    });
}
