#![forbid(unsafe_op_in_unsafe_fn)]

use crate::ui::{property_grid, schema, EditorUiBuild};

#[inline]
fn commit_transform(me: &mut EditorUiBuild, entity: newengine_ecs::EntityId) {
    let position = newengine_math::Vec3::new(me.insp_pos[0], me.insp_pos[1], me.insp_pos[2]);
    let rotation_ypr = (
        me.insp_rot_deg[0].to_radians(),
        me.insp_rot_deg[1].to_radians(),
        me.insp_rot_deg[2].to_radians(),
    );
    let scale = newengine_math::Vec3::new(me.insp_scale[0], me.insp_scale[1], me.insp_scale[2]);
    me.scene_bridge.cmd_set_transform(entity, position, rotation_ypr, scale);
}

pub(crate) fn draw(
    me: &mut EditorUiBuild,
    ui: &mut egui::Ui,
    selection_ctx: &schema::SelectionContext,
) {
    let entity = selection_ctx.entity;
    let fields = schema::property_fields(me, selection_ctx, schema::PropertySectionId::Transform);
    property_grid::section_card_descriptor(ui, "Transform", &fields, |ui, field| {
        match field.id {
            schema::PropertyFieldId::Position => {
                if property_grid::vec3_row(ui, field.label, &mut me.insp_pos, 0.05) {
                    me.apply_transform_snap_to_inspector();
                    commit_transform(me, entity);
                }
            }
            schema::PropertyFieldId::RotationDeg => {
                if property_grid::vec3_row(ui, field.label, &mut me.insp_rot_deg, 0.25) {
                    me.apply_transform_snap_to_inspector();
                    commit_transform(me, entity);
                }
            }
            schema::PropertyFieldId::Scale => {
                if property_grid::vec3_row(ui, field.label, &mut me.insp_scale, 0.05) {
                    me.apply_transform_snap_to_inspector();
                    commit_transform(me, entity);
                }
            }
            schema::PropertyFieldId::SnapTranslate => {
                let _ = property_grid::checkbox_row(ui, field.label, &mut me.transform_snap.translate_enabled);
            }
            schema::PropertyFieldId::SnapRotate => {
                let _ = property_grid::checkbox_row(ui, field.label, &mut me.transform_snap.rotate_enabled);
            }
            schema::PropertyFieldId::SnapScale => {
                let _ = property_grid::checkbox_row(ui, field.label, &mut me.transform_snap.scale_enabled);
            }
            _ => {}
        }
    });
}
