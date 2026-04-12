#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_primitives::Primitive;

use crate::ui::{property_grid, schema, EditorUiBuild};

pub(crate) fn draw(
    me: &mut EditorUiBuild,
    ui: &mut egui::Ui,
    selection_ctx: &schema::SelectionContext,
) {
    let entity = selection_ctx.entity;
    let fields = schema::property_fields(me, selection_ctx, schema::PropertySectionId::Primitive);
    property_grid::section_card_descriptor(ui, "Primitive", &fields, |ui, field| {
        let scene = me.scene_bridge.scene();
        let scene = scene.read();
        let world = scene.world();
        let primitive = world.get::<Primitive>(entity);
        match field.id {
            schema::PropertyFieldId::PrimitiveKind => {
                property_grid::field_label(ui, field);
                if let Some(primitive) = primitive {
                    let registry = me.scene_bridge.primitives();
                    let registry = registry.read();
                    let primitive_name = registry.name(primitive.id).unwrap_or("<unknown>");
                    ui.label(primitive_name);
                } else {
                    ui.label("-");
                }
                property_grid::end_row(ui);
            }
            schema::PropertyFieldId::PrimitiveColor => {
                property_grid::field_label(ui, field);
                let mut rgba = me.insp_color;
                if ui.color_edit_button_rgba_unmultiplied(&mut rgba).changed() {
                    me.insp_color = rgba;
                    let before = primitive.map(|value| value.color).unwrap_or(rgba);
                    me.apply_primitive_color_with_history(entity, before, rgba);
                }
                property_grid::end_row(ui);
            }
            _ => {}
        }
    });
}
