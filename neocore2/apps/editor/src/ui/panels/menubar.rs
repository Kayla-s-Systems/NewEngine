#![forbid(unsafe_op_in_unsafe_fn)]

use egui;

use super::super::EditorUiBuild;

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    egui::TopBottomPanel::top("ne_menubar")
        .resizable(false)
        .exact_height(24.0)
        .show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Scene\tCtrl+N").clicked() {
                        me.scene_bridge.cmd_new_scene();
                        me.editor.commands.clear();
                        me.editor.selection.clear();
                        ui.close_menu();
                    }

                    let has_scene_io = me.scene_io.is_some();

                    if ui
                        .add_enabled(has_scene_io, egui::Button::new("Load Scene...\tCtrl+O"))
                        .clicked()
                    {
                        me.scene_io_ui.open = true;
                        me.scene_io_ui.mode = super::super::SceneIoMode::Load;
                        ui.close_menu();
                    }

                    if ui
                        .add_enabled(has_scene_io, egui::Button::new("Save Scene\tCtrl+S"))
                        .clicked()
                    {
                        me.scene_io_ui.open = true;
                        me.scene_io_ui.mode = super::super::SceneIoMode::Save;
                        ui.close_menu();
                    }

                    if !has_scene_io {
                        ui.label("Scene IO service not found");
                    }

                    ui.separator();

                    if ui.button("Quit").clicked() {
                        log::warn!("Quit: not implemented yet (need shutdown token in UI)");
                        ui.close_menu();
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo\tCtrl+Z").clicked() {
                        if let Some(cmd) = me.editor.commands.pop_undo() {
                            me.apply_editor_command_undo(cmd);
                        }
                        ui.close_menu();
                    }

                    if ui.button("Redo\tCtrl+Y").clicked() {
                        if let Some(cmd) = me.editor.commands.pop_redo() {
                            me.apply_editor_command_redo(cmd);
                        }
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.button("Deselect\tEsc").clicked() {
                        me.editor.selection.clear();
                        me.scene_bridge.set_selection(None);
                        ui.close_menu();
                    }
                });

                ui.menu_button("Asset", |ui| {
                    let has_assets = me.assets.is_some();

                    if ui
                        .add_enabled(has_assets, egui::Button::new("Asset Manager"))
                        .clicked()
                    {
                        me.asset_ui.open = true;
                        ui.close_menu();
                    }

                    if !has_assets {
                        ui.label("AssetManager service not found");
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui.button("Console\tF1").clicked() {
                        me.console_open = !me.console_open;
                        ui.close_menu();
                    }

                    if ui.button("Plugins\tF2").clicked() {
                        if let Ok(mut pm) = me.plugin_manager.lock() {
                            pm.toggle();
                        }
                        ui.close_menu();
                    }
                });

                ui.menu_button("Window", |ui| {
                    if ui
                        .checkbox(&mut me.layout.show_outliner, "World Outliner")
                        .clicked()
                    {
                        ui.close_menu();
                    }

                    if ui
                        .checkbox(&mut me.layout.show_details, "Details")
                        .clicked()
                    {
                        ui.close_menu();
                    }

                    if ui
                        .checkbox(&mut me.layout.show_left_toolbar, "Left Tools")
                        .clicked()
                    {
                        ui.close_menu();
                    }

                    let has_assets = me.assets.is_some();
                    if ui
                        .add_enabled(has_assets, egui::Button::new("Asset Manager"))
                        .clicked()
                    {
                        me.asset_ui.open = true;
                        ui.close_menu();
                    }
                });

                ui.menu_button("Tools", |ui| {
                    if ui.button("Frame Selection\tF").clicked() {
                        me.viewport_bridge.publish_frame_request(false);
                        ui.close_menu();
                    }

                    if ui.button("Frame All\tShift+F").clicked() {
                        me.viewport_bridge.publish_frame_request(true);
                        ui.close_menu();
                    }
                });

                ui.menu_button("Help", |ui| {
                    ui.label("NewEngine Editor");
                    ui.label("UI shell: UE-style layout (foundation)");
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let entities = me.scene_bridge.scene().read().world().entity_count();
                    ui.label(format!("Entities: {entities}"));
                });
            });
        });
}
