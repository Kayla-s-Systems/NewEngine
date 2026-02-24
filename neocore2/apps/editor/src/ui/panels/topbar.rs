#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_platform_winit::egui;
use newengine_ui::BuiltinUiIcon;

use super::super::EditorUiBuild;

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    egui::TopBottomPanel::top("topbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("NewEngine Editor (Foundation)");
            ui.separator();

            let entities = me.scene_bridge.scene().read().world().entity_count();
            ui.label(format!("entities: {entities}"));

            ui.separator();

            if me
                .icons
                .icon_button(ui, BuiltinUiIcon::Reset, "New Scene")
                .clicked()
            {
                me.scene_bridge.cmd_new_scene();
            }

            // Dynamic primitives dropdown (registry-driven).
            {
                let prims = me.scene_bridge.primitives_snapshot();

                if me.selected_primitive.is_none() {
                    me.selected_primitive = prims.first().map(|p| p.1);
                }

                if let Some(sel) = me.selected_primitive {
                    if !prims.iter().any(|p| p.1 == sel) {
                        me.selected_primitive = prims.first().map(|p| p.1);
                    }
                }

                let current_label = me
                    .selected_primitive
                    .and_then(|id| prims.iter().find(|x| x.1 == id).map(|x| x.0.as_str()))
                    .unwrap_or("<none>");

                egui::ComboBox::from_id_salt("add_primitive_combo")
                    .width(160.0)
                    .selected_text(current_label)
                    .show_ui(ui, |ui| {
                        for (name, id) in &prims {
                            let tex = me
                                .previews
                                .lock()
                                .request(*id, newengine_previews::PrimitivePreviewSize::S48);

                            ui.horizontal(|ui| {
                                if tex.0 != 0 {
                                    let tid = egui::TextureId::User(tex.0 as u64);
                                    let st =
                                        egui::load::SizedTexture::new(tid, egui::vec2(24.0, 24.0));
                                    ui.image(st);
                                } else {
                                    ui.add_space(24.0);
                                }
                                ui.selectable_value(&mut me.selected_primitive, Some(*id), name);
                            });
                        }
                    });

                if me
                    .icons
                    .icon_button(ui, BuiltinUiIcon::Load, "Add")
                    .clicked()
                {
                    if let Some(id) = me.selected_primitive {
                        let name = prims
                            .iter()
                            .find(|x| x.1 == id)
                            .map(|x| x.0.clone())
                            .unwrap_or_else(|| "Primitive".to_string());

                        me.scene_bridge.cmd_spawn_primitive(id, name, {
                            // Spawn near camera, not at a hardcoded far-away coordinate.
                            let (cam_pos, cam_fwd) = me.viewport_bridge.read_camera_spawn();
                            let mut p = cam_pos + cam_fwd * 3.0;
                            p.y = p.y.max(0.5);
                            p
                        });
                    }
                }
            }

            ui.separator();

            // Lights: foundation-level editor spawns (no hardcoded renderer dependencies).
            {
                use crate::ui::LightSpawnKind;

                let current_label = match me.selected_light_kind {
                    LightSpawnKind::Directional => "Directional",
                    LightSpawnKind::Point => "Point",
                };

                egui::ComboBox::from_id_salt("add_light_combo")
                    .width(130.0)
                    .selected_text(current_label)
                    .show_ui(ui, |ui| {
                        ui.horizontal(|ui| {
                            if let Some(tid) = me.icons.tex_id(BuiltinUiIcon::LightDirectional) {
                                let st = egui::load::SizedTexture::new(tid, egui::vec2(16.0, 16.0));
                                ui.image(st);
                            } else {
                                ui.add_space(16.0);
                            }
                            ui.selectable_value(
                                &mut me.selected_light_kind,
                                LightSpawnKind::Directional,
                                "Directional",
                            );
                        });

                        ui.horizontal(|ui| {
                            if let Some(tid) = me.icons.tex_id(BuiltinUiIcon::LightPoint) {
                                let st = egui::load::SizedTexture::new(tid, egui::vec2(16.0, 16.0));
                                ui.image(st);
                            } else {
                                ui.add_space(16.0);
                            }
                            ui.selectable_value(
                                &mut me.selected_light_kind,
                                LightSpawnKind::Point,
                                "Point",
                            );
                        });
                    });

                if me
                    .icons
                    .icon_button(
                        ui,
                        match me.selected_light_kind {
                            LightSpawnKind::Directional => BuiltinUiIcon::LightDirectional,
                            LightSpawnKind::Point => BuiltinUiIcon::LightPoint,
                        },
                        "Add Light",
                    )
                    .clicked()
                {
                    match me.selected_light_kind {
                        LightSpawnKind::Directional => {
                            // Spawn a sun-like light above the origin looking down.
                            me.scene_bridge.cmd_spawn_directional_light(
                                "Sun".to_string(),
                                newengine_math::Vec3::new(0.0, 6.0, 0.0),
                                newengine_math::Vec3::new(-0.35, -1.0, -0.25),
                            );
                        }
                        LightSpawnKind::Point => {
                            me.scene_bridge.cmd_spawn_point_light(
                                "PointLight".to_string(),
                                newengine_math::Vec3::new(0.0, 2.0, 0.0),
                            );
                        }
                    }
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Ok(mut pm) = me.plugin_manager.lock() {
                    pm.topbar_button(ui);
                }
                if me
                    .icons
                    .icon_button(ui, BuiltinUiIcon::Console, "Console")
                    .clicked()
                {
                    me.console_open = !me.console_open;
                }
            });
        });
    });
}
