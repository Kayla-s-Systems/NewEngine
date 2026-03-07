#![forbid(unsafe_op_in_unsafe_fn)]

use egui;

use super::super::{EditorUiBuild, SceneIoMode};

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    if !me.scene_io_ui.open {
        return;
    }

    let title = match me.scene_io_ui.mode {
        SceneIoMode::Load => "Load Scene",
        SceneIoMode::Save => "Save Scene",
    };

    egui::Window::new(title)
        .open(&mut me.scene_io_ui.open)
        .collapsible(false)
        .resizable(true)
        .default_width(520.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Path");
                ui.add(
                    egui::TextEdit::singleline(&mut me.scene_io_ui.path)
                        .hint_text("scenes/example.scene.json")
                        .desired_width(f32::INFINITY),
                );
            });

            ui.add_space(6.0);

            match me.scene_io_ui.mode {
                SceneIoMode::Load => {
                    ui.label("Mode: Replace current scene");

                    if ui.button("Load (Replace)").clicked() {
                        me.scene_io_ui.last_error.clear();
                        me.scene_io_ui.last_status.clear();

                        let Some(io) = me.scene_io.as_ref() else {
                            me.scene_io_ui.last_error = "Scene IO service not found".to_string();
                            return;
                        };

                        match io.load_json_v1(&me.scene_io_ui.path, true) {
                            Ok(v) => {
                                me.editor.commands.clear();
                                me.editor.selection.clear();
                                me.scene_bridge.set_selection(None);

                                me.scene_io_ui.last_status = v.to_string();
                            }
                            Err(e) => {
                                me.scene_io_ui.last_error = e;
                            }
                        }
                    }
                }

                SceneIoMode::Save => {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut me.scene_io_ui.pretty, "Pretty JSON");
                        ui.checkbox(
                            &mut me.scene_io_ui.include_empty_entities,
                            "Include empty entities",
                        );
                    });

                    if ui.button("Save").clicked() {
                        me.scene_io_ui.last_error.clear();
                        me.scene_io_ui.last_status.clear();

                        let Some(io) = me.scene_io.as_ref() else {
                            me.scene_io_ui.last_error = "Scene IO service not found".to_string();
                            return;
                        };

                        match io.save_json_v1(
                            &me.scene_io_ui.path,
                            me.scene_io_ui.pretty,
                            me.scene_io_ui.include_empty_entities,
                        ) {
                            Ok(v) => {
                                me.scene_io_ui.last_status = v.to_string();
                            }
                            Err(e) => {
                                me.scene_io_ui.last_error = e;
                            }
                        }
                    }
                }
            }

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("Refresh Formats").clicked() {
                    me.scene_io_ui.last_error.clear();

                    let Some(io) = me.scene_io.as_ref() else {
                        me.scene_io_ui.last_error = "Scene IO service not found".to_string();
                        return;
                    };

                    match io.formats_json() {
                        Ok(v) => me.scene_io_ui.formats_json = v.to_string(),
                        Err(e) => me.scene_io_ui.last_error = e,
                    }
                }

                if ui.button("Copy Status").clicked() {
                    ui.output_mut(|o| {
                        o.commands.push(egui::OutputCommand::CopyText(
                            me.scene_io_ui.last_status.clone(),
                        ));
                    });
                }
            });

            if !me.scene_io_ui.formats_json.trim().is_empty() {
                ui.collapsing("Formats (json)", |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut me.scene_io_ui.formats_json)
                            .desired_width(f32::INFINITY)
                            .desired_rows(6)
                            .code_editor(),
                    );
                });
            }

            if !me.scene_io_ui.last_error.trim().is_empty() {
                ui.colored_label(egui::Color32::LIGHT_RED, &me.scene_io_ui.last_error);
            }

            if !me.scene_io_ui.last_status.trim().is_empty() {
                ui.collapsing("Last result (json)", |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut me.scene_io_ui.last_status)
                            .desired_width(f32::INFINITY)
                            .desired_rows(10)
                            .code_editor(),
                    );
                });
            }
        });
}
