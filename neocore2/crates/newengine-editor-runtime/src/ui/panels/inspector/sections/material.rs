#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_materials::api::MaterialRegistryApi;
use newengine_materials::{MaterialId, MaterialRef};
use newengine_primitives::Primitive;

use crate::scene_bridge::PrimitiveMaterialBase;
use crate::ui::{property_grid, schema, EditorUiBuild};

pub(crate) fn draw(
    me: &mut EditorUiBuild,
    ui: &mut egui::Ui,
    selection_ctx: &schema::SelectionContext,
) {
    let entity = selection_ctx.entity;
    let fields = schema::property_fields(me, selection_ctx, schema::PropertySectionId::Material);
    property_grid::section_card_descriptor(ui, "Material", &fields, |ui, field| {
        let materials = me.scene_bridge.materials_snapshot();
        match field.id {
            schema::PropertyFieldId::MaterialAsset => {
                property_grid::field_label(ui, field);
                let current_label = materials
                    .iter()
                    .find(|entry| entry.1 == me.insp_material)
                    .map(|entry| entry.0.as_str())
                    .unwrap_or("<none>");
                egui::ComboBox::from_id_salt("material_combo")
                    .width(180.0)
                    .selected_text(current_label)
                    .show_ui(ui, |ui| {
                        for (name, id) in &materials {
                            ui.selectable_value(&mut me.insp_material, *id, name);
                        }
                    });

                if me.insp_material != MaterialId::invalid() {
                    let scene = me.scene_bridge.scene();
                    let scene = scene.read();
                    let world = scene.world();
                    let current = if world.get::<Primitive>(entity).is_some() {
                        world
                            .get::<PrimitiveMaterialBase>(entity)
                            .map(|base| base.id)
                            .unwrap_or(MaterialId::invalid())
                    } else {
                        world
                            .get::<MaterialRef>(entity)
                            .map(|material_ref| material_ref.id)
                            .unwrap_or(MaterialId::invalid())
                    };
                    if current != me.insp_material {
                        me.apply_material_with_history(entity, current, me.insp_material);
                    }
                }
                property_grid::end_row(ui);
            }
            _ => {
                let registry = me.scene_bridge.materials();
                let registry = registry.read();
                if let Some(mut descriptor) = registry.get(me.insp_material) {
                    property_grid::field_label(ui, field);
                    let mut changed = false;
                    match field.id {
                        schema::PropertyFieldId::MaterialBaseColor => {
                            changed = ui
                                .color_edit_button_rgba_unmultiplied(&mut descriptor.base_color)
                                .changed();
                        }
                        schema::PropertyFieldId::MaterialMetallic => {
                            changed = ui
                                .add(egui::Slider::new(&mut descriptor.metallic, 0.0..=1.0))
                                .changed();
                        }
                        schema::PropertyFieldId::MaterialRoughness => {
                            changed = ui
                                .add(egui::Slider::new(&mut descriptor.roughness, 0.02..=1.0))
                                .changed();
                        }
                        schema::PropertyFieldId::MaterialEmissiveColor => {
                            changed = ui.color_edit_button_rgb(&mut descriptor.emissive).changed();
                        }
                        schema::PropertyFieldId::MaterialEmissiveStrength => {
                            changed = ui
                                .add(egui::Slider::new(
                                    &mut descriptor.emissive_strength,
                                    0.0..=50.0,
                                ))
                                .changed();
                        }
                        schema::PropertyFieldId::MaterialNormalScale => {
                            changed = ui
                                .add(egui::Slider::new(&mut descriptor.normal_scale, 0.0..=4.0))
                                .changed();
                        }
                        schema::PropertyFieldId::MaterialAoStrength => {
                            changed = ui
                                .add(egui::Slider::new(
                                    &mut descriptor.occlusion_strength,
                                    0.0..=1.0,
                                ))
                                .changed();
                        }
                        schema::PropertyFieldId::MaterialAlphaCutoff => {
                            changed = ui
                                .add(egui::Slider::new(&mut descriptor.alpha_cutoff, 0.0..=1.0))
                                .changed();
                        }
                        _ => {
                            ui.label("-");
                        }
                    }
                    property_grid::end_row(ui);
                    if changed {
                        me.scene_bridge.cmd_update_material(me.insp_material, descriptor);
                    }
                } else {
                    property_grid::field_label(ui, field);
                    ui.label("(material not found)");
                    property_grid::end_row(ui);
                }
            }
        }
    });
}
