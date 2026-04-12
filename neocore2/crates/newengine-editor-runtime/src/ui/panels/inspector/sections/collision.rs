#![forbid(unsafe_op_in_unsafe_fn)]

use crate::gameplay::{CollisionBody, CollisionShape};
use crate::ui::panels::inspector::components;
use crate::ui::{property_grid, schema, EditorUiBuild};

pub(crate) fn draw(
    me: &mut EditorUiBuild,
    ui: &mut egui::Ui,
    selection_ctx: &schema::SelectionContext,
) {
    let entity = selection_ctx.entity;
    let fields = schema::property_fields(me, selection_ctx, schema::PropertySectionId::Collision);
    property_grid::section_card_descriptor(ui, "Collision", &fields, |ui, field| {
        let scene = me.scene_bridge.scene();
        let scene = scene.read();
        let world = scene.world();
        let existing = world.get::<CollisionBody>(entity).copied();
        let mut next = existing.unwrap_or_default();

        match field.id {
            schema::PropertyFieldId::CollisionEnabled => {
                let mut enabled = existing.is_some();
                if property_grid::checkbox_row(ui, field.label, &mut enabled) {
                    if enabled {
                        me.apply_collision_with_history(entity, existing, Some(CollisionBody::default()));
                    } else {
                        me.apply_collision_with_history(entity, existing, None);
                    }
                }
            }
            schema::PropertyFieldId::CollisionDynamic => {
                if property_grid::checkbox_row(ui, field.label, &mut next.dynamic) && existing.is_some() {
                    me.apply_collision_with_history(entity, existing, Some(next));
                }
            }
            schema::PropertyFieldId::CollisionTrigger => {
                if property_grid::checkbox_row(ui, field.label, &mut next.is_trigger) && existing.is_some() {
                    me.apply_collision_with_history(entity, existing, Some(next));
                }
            }
            schema::PropertyFieldId::CollisionShape => {
                property_grid::field_label(ui, field);
                let mut shape_kind = match next.shape {
                    CollisionShape::Box { .. } => 0,
                    CollisionShape::Sphere { .. } => 1,
                    CollisionShape::Capsule { .. } => 2,
                };
                egui::ComboBox::from_id_salt(("collision_shape", entity.stable_u64()))
                    .selected_text(match shape_kind {
                        0 => "Box",
                        1 => "Sphere",
                        _ => "Capsule",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut shape_kind, 0, "Box");
                        ui.selectable_value(&mut shape_kind, 1, "Sphere");
                        ui.selectable_value(&mut shape_kind, 2, "Capsule");
                    });
                next.shape = match (shape_kind, next.shape) {
                    (0, CollisionShape::Box { half_extents }) => CollisionShape::Box { half_extents },
                    (0, _) => CollisionShape::Box {
                        half_extents: [0.5, 0.5, 0.5],
                    },
                    (1, CollisionShape::Sphere { radius }) => CollisionShape::Sphere { radius },
                    (1, _) => CollisionShape::Sphere { radius: 0.5 },
                    (2, CollisionShape::Capsule { radius, half_height }) => {
                        CollisionShape::Capsule { radius, half_height }
                    }
                    (2, _) => CollisionShape::Capsule {
                        radius: 0.45,
                        half_height: 0.5,
                    },
                    _ => next.shape,
                };
                if existing.is_some() && next != existing.unwrap_or_default() {
                    me.apply_collision_with_history(entity, existing, Some(next));
                }
                property_grid::end_row(ui);
            }
            schema::PropertyFieldId::CollisionBoxExtents => {
                property_grid::field_label(ui, field);
                if let CollisionShape::Box { mut half_extents } = next.shape {
                    components::drag_triplet(ui, &mut half_extents, 0.05, 0.01..=4096.0);
                    next.shape = CollisionShape::Box { half_extents };
                    if existing.is_some() {
                        me.apply_collision_with_history(entity, existing, Some(next));
                    }
                } else {
                    ui.label("-");
                }
                property_grid::end_row(ui);
            }
            schema::PropertyFieldId::CollisionSphereRadius => {
                property_grid::field_label(ui, field);
                if let CollisionShape::Sphere { mut radius } = next.shape {
                    ui.add(egui::DragValue::new(&mut radius).speed(0.05).range(0.01..=4096.0));
                    next.shape = CollisionShape::Sphere { radius };
                    if existing.is_some() {
                        me.apply_collision_with_history(entity, existing, Some(next));
                    }
                } else {
                    ui.label("-");
                }
                property_grid::end_row(ui);
            }
            schema::PropertyFieldId::CollisionCapsuleRadius => {
                property_grid::field_label(ui, field);
                if let CollisionShape::Capsule {
                    mut radius,
                    half_height,
                } = next.shape
                {
                    ui.add(egui::DragValue::new(&mut radius).speed(0.05).range(0.01..=4096.0));
                    next.shape = CollisionShape::Capsule { radius, half_height };
                    if existing.is_some() {
                        me.apply_collision_with_history(entity, existing, Some(next));
                    }
                } else {
                    ui.label("-");
                }
                property_grid::end_row(ui);
            }
            schema::PropertyFieldId::CollisionCapsuleHalfHeight => {
                property_grid::field_label(ui, field);
                if let CollisionShape::Capsule {
                    radius,
                    mut half_height,
                } = next.shape
                {
                    ui.add(
                        egui::DragValue::new(&mut half_height)
                            .speed(0.05)
                            .range(0.0..=4096.0),
                    );
                    next.shape = CollisionShape::Capsule { radius, half_height };
                    if existing.is_some() {
                        me.apply_collision_with_history(entity, existing, Some(next));
                    }
                } else {
                    ui.label("-");
                }
                property_grid::end_row(ui);
            }
            _ => {}
        }
    });
}
