#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_platform_winit::egui;

use super::super::EditorUiBuild;

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    egui::TopBottomPanel::top("topbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("NewEngine Editor (Foundation)");
            ui.separator();

            let entities = me.scene_bridge.scene().read().world().entity_count();
            ui.label(format!("entities: {entities}"));

            ui.separator();

            if ui.button("New Scene").clicked() {
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
                                    let st = egui::load::SizedTexture::new(tid, egui::vec2(24.0, 24.0));
                                    ui.image(st);
                                } else {
                                    ui.add_space(24.0);
                                }
                                ui.selectable_value(&mut me.selected_primitive, Some(*id), name);
                            });
                        }
                    });

                if ui.button("Add").clicked() {
                    if let Some(id) = me.selected_primitive {
                        let name = prims
                            .iter()
                            .find(|x| x.1 == id)
                            .map(|x| x.0.clone())
                            .unwrap_or_else(|| "Primitive".to_string());

                        me.scene_bridge
                            .cmd_spawn_primitive(id, name, newengine_math::Vec3::new(0.0, 0.5, 0.0));
                    }
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Ok(mut pm) = me.plugin_manager.lock() {
                    pm.topbar_button(ui);
                }
                if ui.button("Console").clicked() {
                    me.console_open = !me.console_open;
                }
            });
        });
    });
}
