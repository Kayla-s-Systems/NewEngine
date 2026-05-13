#![forbid(unsafe_op_in_unsafe_fn)]

use egui;
use newengine_assets::{AssetAccess, AssetService};

use super::super::widgets;
use super::super::EditorUiBuild;

pub(crate) fn draw_content(me: &mut EditorUiBuild, ui: &mut egui::Ui) {
    widgets::panel_title(
        ui,
        "Asset Browser",
        if me.assets.is_some() {
            "VFS-backed asset service"
        } else {
            "Asset service unavailable in this session"
        },
    );

    widgets::search_field(
        ui,
        &mut me.asset_browser_filter,
        "Filter visible sections (sources, formats, trace, meta)",
    );

    ui.horizontal_wrapped(|ui| {
        ui.label("Path");
        ui.add(
            egui::TextEdit::singleline(&mut me.asset_ui.path)
                .hint_text("content/models/crate.glb")
                .desired_width(320.0),
        );
    });

    let assets = me.assets.clone();
    let Some(assets) = assets.as_ref() else {
        ui.add_space(8.0);
        ui.label("Run the editor with the AssetManager runtime plugin to use the docked asset browser.");
        return;
    };

    ui.horizontal_wrapped(|ui| {
        if ui.button("Info").clicked() {
            match assets.info_json(me.asset_ui.path.trim()) {
                Ok(v) => {
                    me.asset_ui.last_meta_json = serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string());
                    me.asset_ui.last_error.clear();
                }
                Err(e) => me.asset_ui.last_error = e,
            }
        }
        if ui.button("Import v1").clicked() {
            match assets.import_v1(me.asset_ui.path.trim()) {
                Ok(id) => {
                    me.asset_ui.last_id = Some(id);
                    me.asset_ui.last_error.clear();
                }
                Err(e) => me.asset_ui.last_error = e,
            }
        }
        if ui.button("Reload v1").clicked() {
            match assets.reload(me.asset_ui.path.trim()) {
                Ok(id) => {
                    me.asset_ui.last_id = Some(id);
                    me.asset_ui.last_error.clear();
                }
                Err(e) => me.asset_ui.last_error = e,
            }
        }
        if ui.button("Sources").clicked() {
            match assets.sources_json() {
                Ok(v) => {
                    me.asset_ui.sources_json = serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string());
                    me.asset_ui.last_error.clear();
                }
                Err(e) => me.asset_ui.last_error = e,
            }
        }
        if ui.button("Formats").clicked() {
            match assets.formats_json() {
                Ok(v) => {
                    me.asset_ui.formats_json = serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string());
                    me.asset_ui.last_error.clear();
                }
                Err(e) => me.asset_ui.last_error = e,
            }
        }
        if ui.button("Resolve Trace").clicked() {
            match assets.resolve_trace_json(me.asset_ui.path.trim()) {
                Ok(v) => {
                    me.asset_ui.last_trace_json = serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string());
                    me.asset_ui.last_error.clear();
                }
                Err(e) => me.asset_ui.last_error = e,
            }
        }
        if ui.button("Stage Spawn").clicked() {
            let path = me.asset_ui.path.trim().to_string();
            if !path.is_empty() {
                me.queue_asset_spawn_from_path(path, "asset_browser");
            }
        }
        if ui.button("Spawn Near Camera").clicked() {
            let path = me.asset_ui.path.trim().to_string();
            if !path.is_empty() {
                me.queue_asset_spawn_from_path(path, "asset_browser");
                me.spawn_pending_asset_near_camera();
            }
        }
    });

    if !me.asset_ui.last_error.trim().is_empty() {
        ui.colored_label(ui.visuals().error_fg_color, &me.asset_ui.last_error);
    }

    if let Some(id) = me.asset_ui.last_id.clone() {
        me.asset_ui.last_state = match assets.state(&id) {
            Ok(state) => format!("{state:?}"),
            Err(err) => format!("error: {err}"),
        };
        widgets::section_card(ui, "Status", |ui| {
            widgets::stat_row(ui, "Asset Id", id);
            widgets::stat_row(ui, "State", me.asset_ui.last_state.clone());
            if let Some(request) = me.asset_spawn_request.as_ref() {
                widgets::stat_row(ui, "Pending Spawn", request.contract.logical_path.clone());
            }
        });
    }

    let filter = me.asset_browser_filter.trim().to_ascii_lowercase();
    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        let show = |label: &str, filter: &str| filter.is_empty() || label.to_ascii_lowercase().contains(filter);

        if show("resolve trace", &filter) {
            widgets::section_card(ui, "Resolve Trace", |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut me.asset_ui.last_trace_json)
                        .desired_width(f32::INFINITY)
                        .desired_rows(10),
                );
            });
        }
        if show("sources", &filter) {
            widgets::section_card(ui, "Sources", |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut me.asset_ui.sources_json)
                        .desired_width(f32::INFINITY)
                        .desired_rows(8),
                );
            });
        }
        if show("formats", &filter) {
            widgets::section_card(ui, "Formats", |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut me.asset_ui.formats_json)
                        .desired_width(f32::INFINITY)
                        .desired_rows(8),
                );
            });
        }
        if show("meta", &filter) || show("info", &filter) {
            widgets::section_card(ui, "Info / Meta", |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut me.asset_ui.last_meta_json)
                        .desired_width(f32::INFINITY)
                        .desired_rows(10),
                );
            });
        }
    });
}
