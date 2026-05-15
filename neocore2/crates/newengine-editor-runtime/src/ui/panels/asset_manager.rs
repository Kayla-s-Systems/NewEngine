#![forbid(unsafe_op_in_unsafe_fn)]

use egui;
use newengine_assets::{AssetAccess, AssetService};

use super::super::EditorUiBuild;

pub(crate) fn draw(me: &mut EditorUiBuild, ctx: &egui::Context) {
    if !me.asset_ui.open {
        return;
    }

    let mut open = me.asset_ui.open;
    let assets = me.assets.clone();

    egui::Window::new("Asset Manager")
        .open(&mut open)
        .resizable(true)
        .default_size(egui::vec2(680.0, 520.0))
        .show(ctx, |ui| {
            let Some(assets) = assets.as_ref() else {
                ui.label("AssetManager service is not available.\n\nRun the editor with an AssetManager runtime plugin.");
                return;
            };

            ui.horizontal(|ui| {
                if ui.button("Refresh Sources").clicked() {
                    match assets.sources_json_v1() {
                        Ok(v) => {
                            me.asset_ui.sources_json = format!("{v:#?}");
                            me.asset_ui.last_error.clear();
                        }
                        Err(e) => {
                            me.asset_ui.last_error = e;
                        }
                    }
                }

                if ui.button("Refresh Formats").clicked() {
                    match assets.formats_json_v1() {
                        Ok(v) => {
                            me.asset_ui.formats_json = format!("{v:#?}");
                            me.asset_ui.last_error.clear();
                        }
                        Err(e) => {
                            me.asset_ui.last_error = e;
                        }
                    }
                }

                ui.separator();

                ui.label("Logical path:");
                ui.add(
                    egui::TextEdit::singleline(&mut me.asset_ui.path)
                        .hint_text("e.g. ui/icons/builtin_icons.neytd@app_logo")
                        .desired_width(320.0),
                );

                if ui.button("Import v1").clicked() {
                    match assets.import_v1(me.asset_ui.path.trim()) {
                        Ok(id) => {
                            me.asset_ui.last_id = Some(id);
                            me.asset_ui.last_error.clear();
                        }
                        Err(e) => {
                            me.asset_ui.last_error = e;
                        }
                    }
                }

                if ui.button("Reload v1").clicked() {
                    match assets.reload(me.asset_ui.path.trim()) {
                        Ok(id) => {
                            me.asset_ui.last_id = Some(id);
                            me.asset_ui.last_error.clear();
                        }
                        Err(e) => {
                            me.asset_ui.last_error = e;
                        }
                    }
                }

                if ui.button("Resolve Trace").clicked() {
                    match assets.resolve_trace_json_v1(me.asset_ui.path.trim()) {
                        Ok(v) => {
                            me.asset_ui.last_trace_json = format!("{v:#?}");
                            me.asset_ui.last_error.clear();
                        }
                        Err(e) => {
                            me.asset_ui.last_error = e;
                        }
                    }
                }

                if ui.button("Status Graph v1").clicked() {
                    match assets.status_graph_json_v1(me.asset_ui.path.trim()) {
                        Ok(v) => {
                            me.asset_ui.last_status_graph_json = serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string());
                            me.asset_ui.last_error.clear();
                        }
                        Err(e) => {
                            me.asset_ui.last_error = e;
                        }
                    }
                }


                ui.separator();

                if ui.button("Stage Spawn").clicked() {
                    let path = me.asset_ui.path.trim().to_string();
                    if !path.is_empty() {
                        me.queue_asset_spawn_from_path(path, "asset_manager");
                    }
                }

                if ui.button("Spawn Near Camera").clicked() {
                    let path = me.asset_ui.path.trim().to_string();
                    if !path.is_empty() {
                        me.queue_asset_spawn_from_path(path, "asset_manager");
                        me.spawn_pending_asset_near_camera();
                    }
                }
            });

            if let Some(request) = me.asset_spawn_request.as_ref() {
                ui.add_space(6.0);
                ui.label(format!("Pending asset spawn: {} [{}]", request.contract.logical_path, request.contract.import.class.label()));
            }

            if !me.asset_ui.last_error.trim().is_empty() {
                ui.add_space(6.0);
                ui.colored_label(ui.visuals().error_fg_color, &me.asset_ui.last_error);
            }

            ui.add_space(8.0);

            // Status.
            if let Some(id) = me.asset_ui.last_id.clone() {
                let st = assets.state(&id);
                me.asset_ui.last_state = match st {
                    Ok(s) => format!("{s:?}"),
                    Err(e) => format!("error: {e}"),
                };

                ui.horizontal(|ui| {
                    ui.label("Last id:");
                    ui.monospace(&id);
                    ui.separator();
                    ui.label("State:");
                    ui.monospace(&me.asset_ui.last_state);

                    if ui.button("Read blob_wire_v1 meta").clicked() {
                        match assets.blob_wire_v1(&id) {
                            Ok((meta, payload)) => {
                                me.asset_ui.last_meta_json = meta;
                                me.asset_ui.last_error.clear();
                                log::info!(
                                    "asset meta loaded id='{}' payload_bytes={}",
                                    id,
                                    payload.len()
                                );
                            }
                            Err(e) => {
                                me.asset_ui.last_error = e;
                            }
                        }
                    }
                });
            }

            ui.separator();

            egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                ui.collapsing("Resolve Trace", |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut me.asset_ui.last_trace_json)
                            .desired_width(f32::INFINITY)
                            .desired_rows(10),
                    );
                });

                ui.collapsing("Asset Status Graph", |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut me.asset_ui.last_status_graph_json)
                            .desired_width(f32::INFINITY)
                            .desired_rows(12),
                    );
                });

                ui.collapsing("Sources", |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut me.asset_ui.sources_json)
                            .desired_width(f32::INFINITY)
                            .desired_rows(14),
                    );
                });

                ui.collapsing("Formats", |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut me.asset_ui.formats_json)
                            .desired_width(f32::INFINITY)
                            .desired_rows(10),
                    );
                });

                ui.collapsing("Last meta.json", |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut me.asset_ui.last_meta_json)
                            .desired_width(f32::INFINITY)
                            .desired_rows(10),
                    );
                });
            });
        });

    me.asset_ui.open = open;
}
