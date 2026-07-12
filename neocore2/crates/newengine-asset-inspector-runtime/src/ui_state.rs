use newengine_ui_api::{
    UiStatePatch, ENGINE_UI_SERVICE_ID, UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1,
};
use serde_json::json;

use crate::model::{AssetInspectorMode, AssetInspectorReport, InspectorEntry, InspectorField};
use crate::{
    ASSET_INSPECTOR_STATE_CONTRACT, ASSET_INSPECTOR_STATE_SOURCE, ASSET_INSPECTOR_SURFACE_ID,
};

pub(crate) const ENTRY_ROWS: usize = 12;
pub(crate) const FIELD_ROWS: usize = 10;
pub(crate) const DIAGNOSTIC_ROWS: usize = 4;

pub(crate) struct InspectorUiSnapshot<'a> {
    pub(crate) frame_index: u64,
    pub(crate) current_path: &'a str,
    pub(crate) mode: AssetInspectorMode,
    pub(crate) page: usize,
    pub(crate) entries: &'a [InspectorEntry],
    pub(crate) selected_index: Option<usize>,
    pub(crate) report: Option<&'a AssetInspectorReport>,
    pub(crate) status: &'a str,
}

pub(crate) fn publish_inspector_state(snapshot: InspectorUiSnapshot<'_>) -> bool {
    let total_pages = snapshot.entries.len().max(1).div_ceil(ENTRY_ROWS);
    let page = snapshot.page.min(total_pages.saturating_sub(1));
    let start = page * ENTRY_ROWS;
    let end = (start + ENTRY_ROWS).min(snapshot.entries.len());
    let visible_entries = &snapshot.entries[start..end];

    let mut patch = UiStatePatch::new(snapshot.frame_index, ASSET_INSPECTOR_SURFACE_ID)
        .with_change("shell", "path", json!(display_path(snapshot.current_path)))
        .with_change("shell", "mode", json!(snapshot.mode.label()))
        .with_change("shell", "status", json!(snapshot.status))
        .with_change("shell", "entry_count", json!(snapshot.entries.len()))
        .with_change(
            "shell",
            "page_label",
            json!(format!("Page {} / {}", page + 1, total_pages)),
        )
        .with_change(
            "mode_all",
            "active",
            json!(snapshot.mode == AssetInspectorMode::All),
        )
        .with_change(
            "mode_runtime",
            "active",
            json!(snapshot.mode == AssetInspectorMode::Runtime),
        )
        .with_change(
            "mode_source",
            "active",
            json!(snapshot.mode == AssetInspectorMode::Source),
        );

    for row in 0..ENTRY_ROWS {
        let source = format!("entry_{row:02}");
        if let Some(entry) = visible_entries.get(row) {
            let absolute_index = start + row;
            let marker = if entry.is_directory {
                "DIR"
            } else if entry.source_asset {
                "SRC"
            } else {
                "RUN"
            };
            let detail = if entry.extension.is_empty() {
                entry.kind.clone()
            } else {
                format!("{} · .{}", entry.kind, entry.extension)
            };
            patch = patch
                .with_change(&source, "visible", json!(true))
                .with_change(&source, "name", json!(entry.name))
                .with_change(&source, "marker", json!(marker))
                .with_change(&source, "detail", json!(detail))
                .with_change(&source, "path", json!(entry.logical_path))
                .with_change(
                    &source,
                    "selected",
                    json!(snapshot.selected_index == Some(absolute_index)),
                );
        } else {
            patch = patch
                .with_change(&source, "visible", json!(false))
                .with_change(&source, "name", json!(""))
                .with_change(&source, "marker", json!(""))
                .with_change(&source, "detail", json!(""))
                .with_change(&source, "path", json!(""))
                .with_change(&source, "selected", json!(false));
        }
    }

    let empty_report = empty_selection_report();
    let report = snapshot.report.unwrap_or(&empty_report);
    patch = patch
        .with_change("inspect", "title", json!(report.title))
        .with_change("inspect", "asset_ref", json!(report.asset_ref))
        .with_change("inspect", "asset_kind", json!(report.asset_kind))
        .with_change("inspect", "document_kind", json!(report.document_kind))
        .with_change("inspect", "decoder", json!(report.decoder))
        .with_change("inspect", "summary", json!(report.summary))
        .with_change(
            "inspect",
            "counterpart",
            json!(report
                .counterpart
                .as_deref()
                .unwrap_or("No counterpart resolved")),
        )
        .with_change(
            "inspect",
            "counterpart_available",
            json!(report.counterpart.is_some()),
        );

    for row in 0..FIELD_ROWS {
        let source = format!("field_{row:02}");
        if let Some(field) = report.fields.get(row) {
            patch = patch
                .with_change(&source, "visible", json!(true))
                .with_change(&source, "category", json!(field.category))
                .with_change(&source, "label", json!(field.label))
                .with_change(&source, "value", json!(field.value));
        } else {
            patch = patch
                .with_change(&source, "visible", json!(false))
                .with_change(&source, "category", json!(""))
                .with_change(&source, "label", json!(""))
                .with_change(&source, "value", json!(""));
        }
    }

    for row in 0..DIAGNOSTIC_ROWS {
        let source = format!("diagnostic_{row:02}");
        if let Some(message) = report.diagnostics.get(row) {
            patch = patch
                .with_change(&source, "visible", json!(true))
                .with_change(&source, "message", json!(message));
        } else {
            patch = patch
                .with_change(&source, "visible", json!(false))
                .with_change(&source, "message", json!(""));
        }
    }

    let payload = match serde_json::to_vec(&patch) {
        Ok(payload) => payload,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "asset inspector: state patch encode failed contract='{}' err='{}'",
                ASSET_INSPECTOR_STATE_CONTRACT,
                error
            );
            return false;
        }
    };
    match newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1,
        &payload,
    ) {
        Ok(Some(_)) => {
            newengine_ulog_api::ulog::info!(
                "asset inspector: state published surface='{}' changes={} path='{}' mode={}",
                ASSET_INSPECTOR_SURFACE_ID,
                patch.changes.len(),
                snapshot.current_path,
                snapshot.mode.label(),
            );
            true
        }
        Ok(None) => {
            newengine_ulog_api::ulog::warn!(
                "asset inspector: engine.ui route unavailable source='{}'",
                ASSET_INSPECTOR_STATE_SOURCE
            );
            false
        }
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "asset inspector: state patch failed source='{}' err='{}'",
                ASSET_INSPECTOR_STATE_SOURCE,
                error
            );
            false
        }
    }
}

fn empty_selection_report() -> AssetInspectorReport {
    AssetInspectorReport {
        title: "No asset selected".to_owned(),
        asset_ref: "Select an asset from the VFS list".to_owned(),
        asset_kind: "—".to_owned(),
        document_kind: "—".to_owned(),
        decoder: "Native decoder ready".to_owned(),
        summary: "Select a runtime or source asset to inspect its native metadata, source pairing and diagnostics.".to_owned(),
        fields: vec![
            InspectorField::categorized("SUMMARY", "Type", "—"),
            InspectorField::categorized("SUMMARY", "Path", "—"),
            InspectorField::categorized("SUMMARY", "Container", "—"),
            InspectorField::categorized("SUMMARY", "Runtime", "—"),
            InspectorField::categorized("SUMMARY", "Source", "—"),
            InspectorField::categorized("SUMMARY", "References", "—"),
            InspectorField::categorized("NATIVE", "Codec", "Ready"),
            InspectorField::categorized("NATIVE", "VFS", "Mounted"),
            InspectorField::categorized("NATIVE", "Inspection", "Provider-owned"),
            InspectorField::categorized("STATUS", "State", "Waiting for selection"),
        ],
        ..AssetInspectorReport::default()
    }
}

fn display_path(path: &str) -> String {
    if path.trim().is_empty() {
        "gameAssets:/".to_owned()
    } else {
        format!("gameAssets:/{}", path.trim_matches('/'))
    }
}
