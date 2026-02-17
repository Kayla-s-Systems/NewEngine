#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_materials::api::MaterialRegistryApi;
use newengine_materials::MaterialId;
use newengine_materials::MaterialRef;
use newengine_platform_winit::egui;
use newengine_primitives::Primitive;
use newengine_scene::components::Name;

use crate::scene_bridge::GridSettings;

use super::super::EditorUiBuild;

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    egui::SidePanel::right("inspector")
        .resizable(true)
        .default_width(300.0)
        .min_width(260.0)
        .show(ctx, |ui| {
            ui.heading("Inspector");
            ui.add_space(6.0);

            // Viewport / Grid (editor-only).
            {
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Viewport").strong());
                    ui.add_space(4.0);

                    let mut gs: GridSettings = me.scene_bridge.grid_settings();
                    let mut changed = false;

                    changed |= ui.checkbox(&mut gs.auto_spacing, "Auto grid spacing").changed();

                    ui.add_enabled_ui(!gs.auto_spacing, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Spacing");
                            changed |= ui
                                .add(egui::DragValue::new(&mut gs.spacing).speed(0.05).range(0.001..=10_000.0))
                                .changed();
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Extent");
                        changed |= ui
                            .add(egui::DragValue::new(&mut gs.half_lines).speed(1).range(8..=4096))
                            .changed();
                        ui.label("half-lines");
                    });

                    ui.horizontal(|ui| {
                        ui.label("Major every");
                        changed |= ui
                            .add(egui::DragValue::new(&mut gs.major_every).speed(1).range(1..=256))
                            .changed();
                    });

                    ui.horizontal(|ui| {
                        ui.label("Minor");
                        changed |= ui.color_edit_button_rgba_unmultiplied(&mut gs.minor_color).changed();
                    });

                    ui.horizontal(|ui| {
                        ui.label("Major");
                        changed |= ui.color_edit_button_rgba_unmultiplied(&mut gs.major_color).changed();
                    });

                    ui.horizontal(|ui| {
                        ui.label("Background");
                        changed |= ui
                            .color_edit_button_rgba_unmultiplied(&mut gs.background_color)
                            .changed();
                    });

                    if changed {
                        me.scene_bridge.set_grid_settings(gs);
                    }
                });
                ui.add_space(8.0);
            }

            let sel_count = me.editor.selection.len();
            let selected = me.editor.selection.primary();
            let Some(e) = selected else {
                ui.label("No selection.");
                return;
            };

            if sel_count > 1 {
                ui.label(format!(
                    "Selected: {sel_count} entities (editing primary)"
                ));
            }

            if me.selected_entity_cached != Some(e) {
                me.refresh_inspector_cache(e);
            }

            // Name
            {
                let scene = me.scene_bridge.scene();
                let s = scene.read();
                let w = s.world();
                let name = w.get::<Name>(e).map(|n| n.as_str()).unwrap_or("<unnamed>");
                ui.label(format!("Entity: {name}"));
            }

            ui.separator();

            // Transform
            ui.collapsing("Transform", |ui| {
                let mut changed = false;

                ui.horizontal(|ui| {
                    ui.label("Position");
                    changed |= ui.add(egui::DragValue::new(&mut me.insp_pos[0]).speed(0.05)).changed();
                    changed |= ui.add(egui::DragValue::new(&mut me.insp_pos[1]).speed(0.05)).changed();
                    changed |= ui.add(egui::DragValue::new(&mut me.insp_pos[2]).speed(0.05)).changed();
                });

                ui.horizontal(|ui| {
                    ui.label("Rotation (deg)");
                    changed |= ui.add(egui::DragValue::new(&mut me.insp_rot_deg[0]).speed(0.25)).changed();
                    changed |= ui.add(egui::DragValue::new(&mut me.insp_rot_deg[1]).speed(0.25)).changed();
                    changed |= ui.add(egui::DragValue::new(&mut me.insp_rot_deg[2]).speed(0.25)).changed();
                });

                ui.horizontal(|ui| {
                    ui.label("Scale");
                    changed |= ui.add(egui::DragValue::new(&mut me.insp_scale[0]).speed(0.05)).changed();
                    changed |= ui.add(egui::DragValue::new(&mut me.insp_scale[1]).speed(0.05)).changed();
                    changed |= ui.add(egui::DragValue::new(&mut me.insp_scale[2]).speed(0.05)).changed();
                });

                if changed {
                    let pos = newengine_math::Vec3::new(me.insp_pos[0], me.insp_pos[1], me.insp_pos[2]);
                    let ypr = (
                        me.insp_rot_deg[0].to_radians(),
                        me.insp_rot_deg[1].to_radians(),
                        me.insp_rot_deg[2].to_radians(),
                    );
                    let scale = newengine_math::Vec3::new(me.insp_scale[0], me.insp_scale[1], me.insp_scale[2]);
                    me.scene_bridge.cmd_set_transform(e, pos, ypr, scale);
                }
            });

            // Primitive
            ui.collapsing("Primitive", |ui| {
                let scene = me.scene_bridge.scene();
                let s = scene.read();
                let w = s.world();
                let prim = w.get::<Primitive>(e);

                if let Some(p) = prim {
                    let reg = me.scene_bridge.primitives();
                    let reg = reg.read();
                    let prim_name = reg.name(p.id).unwrap_or("<unknown>");
                    ui.label(format!("Kind: {prim_name}"));

                    let mut rgba = me.insp_color;
                    let changed = ui.color_edit_button_rgba_unmultiplied(&mut rgba).changed();
                    if changed {
                        me.insp_color = rgba;
                        me.scene_bridge.cmd_set_primitive_color(e, rgba);
                    }
                } else {
                    ui.label("(no Primitive component)");
                }
            });

            // Materials (foundation step).
            ui.collapsing("Material", |ui| {
                let mats = me.scene_bridge.materials_snapshot();
                let current_label = mats
                    .iter()
                    .find(|x| x.1 == me.insp_material)
                    .map(|x| x.0.as_str())
                    .unwrap_or("<none>");

                egui::ComboBox::from_id_salt("material_combo")
                    .width(180.0)
                    .selected_text(current_label)
                    .show_ui(ui, |ui| {
                        for (name, id) in &mats {
                            ui.selectable_value(&mut me.insp_material, *id, name);
                        }
                    });

                if me.insp_material != MaterialId::invalid() {
                    let scene = me.scene_bridge.scene();
                    let s = scene.read();
                    let w = s.world();
                    let current = w
                        .get::<MaterialRef>(e)
                        .map(|mr| mr.id)
                        .unwrap_or(MaterialId::invalid());
                    if current != me.insp_material {
                        me.scene_bridge.cmd_set_material(e, me.insp_material);
                    }

                    let reg = me.scene_bridge.materials();
                    let reg = reg.read();
                    if let Some(mut desc) = reg.get(me.insp_material) {
                        let mut changed = false;

                        ui.horizontal(|ui| {
                            ui.label("Base color");
                            changed |= ui
                                .color_edit_button_rgba_unmultiplied(&mut desc.base_color)
                                .changed();
                        });

                        ui.horizontal(|ui| {
                            ui.label("Metallic");
                            changed |= ui.add(egui::Slider::new(&mut desc.metallic, 0.0..=1.0)).changed();
                        });

                        ui.horizontal(|ui| {
                            ui.label("Roughness");
                            changed |= ui
                                .add(egui::Slider::new(&mut desc.roughness, 0.02..=1.0))
                                .changed();
                        });

                        if changed {
                            me.scene_bridge.cmd_update_material(me.insp_material, desc);
                        }
                    } else {
                        ui.label("(material not found in registry)");
                    }
                } else {
                    ui.label("(no Material assigned)");
                }
            });
        });
}
