#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_lighting::{AmbientLight, DirectionalLight, PointLight};
use newengine_materials::api::MaterialRegistryApi;
use newengine_materials::MaterialId;
use newengine_materials::MaterialRef;
use newengine_platform_winit::egui;
use newengine_primitives::Primitive;
use newengine_scene::components::Name;
use newengine_transform::GlobalTransform;
use newengine_ui::BuiltinUiIcon;

use super::super::EditorUiBuild;

#[inline]
fn dir_to_yaw_pitch_deg(d: newengine_math::Vec3) -> (f32, f32) {
    let dn = d.normalize_or_zero();
    // yaw around Y, pitch around X (looking down negative Y is pitch -90).
    let yaw = dn.x.atan2(dn.z);
    let pitch = (-dn.y).asin();
    (yaw.to_degrees(), pitch.to_degrees())
}

#[inline]
fn yaw_pitch_deg_to_dir(yaw_deg: f32, pitch_deg: f32) -> newengine_math::Vec3 {
    let yaw = yaw_deg.to_radians();
    let pitch = pitch_deg.to_radians();
    let cy = yaw.cos();
    let sy = yaw.sin();
    let cp = pitch.cos();
    let sp = pitch.sin();
    // Inverse of dir_to_yaw_pitch_deg convention.
    newengine_math::Vec3::new(sy * cp, -sp, cy * cp).normalize_or_zero()
}

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    egui::SidePanel::right("inspector")
        .resizable(true)
        .default_width(300.0)
        .min_width(260.0)
        .show(ctx, |ui| {
            ui.heading("Inspector");
            ui.add_space(6.0);

            // Viewport controls intentionally removed: grid is an editor overlay with fixed defaults.

            let sel_count = me.editor.selection.len();
            let selected = me.editor.selection.primary();
            let Some(e) = selected else {
                ui.label("No selection.");
                return;
            };

            if sel_count > 1 {
                ui.label(format!("Selected: {sel_count} entities (editing primary)"));
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
                    changed |= ui
                        .add(egui::DragValue::new(&mut me.insp_pos[0]).speed(0.05))
                        .changed();
                    changed |= ui
                        .add(egui::DragValue::new(&mut me.insp_pos[1]).speed(0.05))
                        .changed();
                    changed |= ui
                        .add(egui::DragValue::new(&mut me.insp_pos[2]).speed(0.05))
                        .changed();
                });

                ui.horizontal(|ui| {
                    ui.label("Rotation (deg)");
                    changed |= ui
                        .add(egui::DragValue::new(&mut me.insp_rot_deg[0]).speed(0.25))
                        .changed();
                    changed |= ui
                        .add(egui::DragValue::new(&mut me.insp_rot_deg[1]).speed(0.25))
                        .changed();
                    changed |= ui
                        .add(egui::DragValue::new(&mut me.insp_rot_deg[2]).speed(0.25))
                        .changed();
                });

                ui.horizontal(|ui| {
                    ui.label("Scale");
                    changed |= ui
                        .add(egui::DragValue::new(&mut me.insp_scale[0]).speed(0.05))
                        .changed();
                    changed |= ui
                        .add(egui::DragValue::new(&mut me.insp_scale[1]).speed(0.05))
                        .changed();
                    changed |= ui
                        .add(egui::DragValue::new(&mut me.insp_scale[2]).speed(0.05))
                        .changed();
                });

                if changed {
                    let pos =
                        newengine_math::Vec3::new(me.insp_pos[0], me.insp_pos[1], me.insp_pos[2]);
                    let ypr = (
                        me.insp_rot_deg[0].to_radians(),
                        me.insp_rot_deg[1].to_radians(),
                        me.insp_rot_deg[2].to_radians(),
                    );
                    let scale = newengine_math::Vec3::new(
                        me.insp_scale[0],
                        me.insp_scale[1],
                        me.insp_scale[2],
                    );
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

            // Lighting (editor-first, deterministic, module-driven).
            ui.collapsing("Lighting", |ui| {
                // Global ambient (resource).
                {
                    let scene = me.scene_bridge.scene();
                    let s = scene.read();
                    let w = s.world();
                    let amb = w.resource::<AmbientLight>().copied().unwrap_or_default();

                    let mut amb_rgb = amb.color;
                    let mut amb_int = amb.intensity;

                    ui.horizontal(|ui| {
                        ui.label("Ambient");
                        ui.color_edit_button_rgb(&mut amb_rgb);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Ambient intensity");
                        ui.add(egui::Slider::new(&mut amb_int, 0.0..=2.0));
                    });

                    if amb_rgb != amb.color || (amb_int - amb.intensity).abs() > 1e-6 {
                        me.scene_bridge.cmd_set_ambient_light(amb_rgb, amb_int);
                    }

                    ui.separator();
                }

                let scene = me.scene_bridge.scene();
                let s = scene.read();
                let w = s.world();

                if let Some(dl) = w.get::<DirectionalLight>(e).copied() {
                    ui.horizontal(|ui| {
                        if let Some(tid) = me.icons.tex_id(BuiltinUiIcon::LightDirectional) {
                            let st = egui::load::SizedTexture::new(tid, egui::vec2(16.0, 16.0));
                            ui.image(st);
                        }
                        ui.label("Directional Light");
                    });

                    let mut rgb = dl.color;
                    let mut intensity = dl.intensity;

                    let d = newengine_math::Vec3::new(
                        dl.direction_ws[0],
                        dl.direction_ws[1],
                        dl.direction_ws[2],
                    );
                    let (mut yaw_deg, mut pitch_deg) = dir_to_yaw_pitch_deg(d);

                    ui.horizontal(|ui| {
                        ui.label("Color");
                        ui.color_edit_button_rgb(&mut rgb);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Intensity");
                        ui.add(egui::Slider::new(&mut intensity, 0.0..=50.0));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Yaw (deg)");
                        ui.add(egui::DragValue::new(&mut yaw_deg).speed(0.25));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Pitch (deg)");
                        ui.add(egui::DragValue::new(&mut pitch_deg).speed(0.25));
                    });

                    let new_dir = yaw_pitch_deg_to_dir(yaw_deg, pitch_deg);
                    if rgb != dl.color
                        || (intensity - dl.intensity).abs() > 1e-6
                        || (new_dir.x - dl.direction_ws[0]).abs() > 1e-4
                        || (new_dir.y - dl.direction_ws[1]).abs() > 1e-4
                        || (new_dir.z - dl.direction_ws[2]).abs() > 1e-4
                    {
                        me.scene_bridge
                            .cmd_set_directional_light(e, new_dir, rgb, intensity);
                    }
                } else if let Some(pl) = w.get::<PointLight>(e).copied() {
                    ui.horizontal(|ui| {
                        if let Some(tid) = me.icons.tex_id(BuiltinUiIcon::LightPoint) {
                            let st = egui::load::SizedTexture::new(tid, egui::vec2(16.0, 16.0));
                            ui.image(st);
                        }
                        ui.label("Point Light");
                    });

                    let mut rgb = pl.color;
                    let mut intensity = pl.intensity;
                    let mut range = pl.range;

                    ui.horizontal(|ui| {
                        ui.label("Color");
                        ui.color_edit_button_rgb(&mut rgb);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Intensity");
                        ui.add(egui::Slider::new(&mut intensity, 0.0..=200.0));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Range");
                        ui.add(egui::Slider::new(&mut range, 0.1..=100.0));
                    });

                    if rgb != pl.color
                        || (intensity - pl.intensity).abs() > 1e-6
                        || (range - pl.range).abs() > 1e-6
                    {
                        me.scene_bridge
                            .cmd_set_point_light(e, rgb, intensity, range);
                    }

                    if let Some(gt) = w.get::<GlobalTransform>(e) {
                        let p = gt.0.w_axis;
                        ui.label(format!("Position: [{:.2}, {:.2}, {:.2}]", p.x, p.y, p.z));
                    }
                } else {
                    ui.label("(no Light component on selection)");
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
                            changed |= ui
                                .add(egui::Slider::new(&mut desc.metallic, 0.0..=1.0))
                                .changed();
                        });

                        ui.horizontal(|ui| {
                            ui.label("Roughness");
                            changed |= ui
                                .add(egui::Slider::new(&mut desc.roughness, 0.02..=1.0))
                                .changed();
                        });

                        ui.horizontal(|ui| {
                            ui.label("Emissive");
                            changed |= ui.color_edit_button_rgb(&mut desc.emissive).changed();
                        });

                        ui.horizontal(|ui| {
                            ui.label("Normal scale");
                            changed |= ui
                                .add(egui::Slider::new(&mut desc.normal_scale, 0.0..=4.0))
                                .changed();
                        });

                        ui.horizontal(|ui| {
                            ui.label("AO strength");
                            changed |= ui
                                .add(egui::Slider::new(&mut desc.occlusion_strength, 0.0..=1.0))
                                .changed();
                        });

                        ui.horizontal(|ui| {
                            ui.label("Alpha cutoff");
                            changed |= ui
                                .add(egui::Slider::new(&mut desc.alpha_cutoff, 0.0..=1.0))
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
