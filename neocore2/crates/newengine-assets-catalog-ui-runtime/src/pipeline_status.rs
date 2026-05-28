//! Editor-facing asset pipeline status queries.
//!
//! Content Browser consumes these statuses from `engine.assets` subroutes. It is
//! not a new API domain and does not duplicate import, UID, thumbnail or package
//! writer ownership.

use crate::{AssetsCatalogEntry, AssetsCatalogRuntimeState, MAX_VISIBLE_ENTRIES};
use crate::entry_presentation::preview_plan_label;
use crate::path::normalize_catalog_path;
use crate::value_helpers::string_field;
use newengine_assets_api::AssetService;
use serde_json::{json, Value};

pub(crate) fn hydrate_preview_plans_for_entries(
    state: &mut AssetsCatalogRuntimeState,
    entries: &mut [AssetsCatalogEntry],
    warnings: &mut Vec<String>,
) {
    let mut failures = 0usize;
    for entry in entries.iter_mut().filter(|entry| !entry.is_directory()).take(MAX_VISIBLE_ENTRIES) {
        if !entry.thumbnail.trim().is_empty() {
            continue;
        }
        match state.client.thumbnail_json_v1(json!({ "logical_path": entry.logical_path.as_str() })) {
            Ok(value) => {
                entry.thumbnail = thumbnail_label_from_value(&value)
                    .unwrap_or_else(|| preview_plan_label(entry).to_owned());
            }
            Err(error) => {
                failures += 1;
                if failures <= 2 {
                    warnings.push(format!("engine.assets.thumbnail_v1 unavailable for '{}': {error}", entry.logical_path));
                }
                entry.thumbnail = preview_plan_label(entry).to_owned();
            }
        }
    }
    if failures > 2 {
        warnings.push(format!("engine.assets.thumbnail_v1 unavailable for {} additional assets", failures - 2));
    }
}

fn thumbnail_label_from_value(value: &Value) -> Option<String> {
    let thumbnail = value.get("thumbnail")?;
    let kind = string_field(thumbnail, &["kind", "strategy", "label"])?;
    let state = string_field(thumbnail, &["state"]).unwrap_or_else(|| "planned".to_owned());
    let icon = string_field(thumbnail, &["icon_ref", "icon", "asset_icon"]);
    let cache_key = string_field(thumbnail, &["cache_key"]);
    Some(match (icon, cache_key) {
        (Some(icon), Some(cache_key)) => format!("{kind} / {state} / {icon} / {cache_key}"),
        (Some(icon), None) => format!("{kind} / {state} / {icon}"),
        (None, Some(cache_key)) => format!("{kind} / {state} / {cache_key}"),
        (None, None) => format!("{kind} / {state}"),
    })
}

pub(crate) fn apply_import_lifecycle_rows(
    state: &mut AssetsCatalogRuntimeState,
    logical_path: &str,
    entries: &mut [AssetsCatalogEntry],
    warnings: &mut Vec<String>,
) {
    let response = match state.client.dirty_scan_json_v1(json!({
        "root": logical_path,
        "recursive": false,
        "max_entries": 256,
    })) {
        Ok(value) => value,
        Err(error) => {
            warnings.push(format!("engine.assets.dirty_scan_v1 unavailable: {error}"));
            return;
        }
    };
    let rows = response.get("rows").and_then(Value::as_array).cloned().unwrap_or_default();
    for row in rows {
        let Some(path) = string_field(&row, &["logical_path", "path"]) else { continue; };
        let normalized = normalize_catalog_path(&path);
        if let Some(entry) = entries.iter_mut().find(|entry| entry.logical_path == normalized) {
            entry.import_stage = string_field(&row, &["stage"]).unwrap_or_else(|| "unknown".to_owned());
            entry.import_action = string_field(&row, &["recommended_action"]).unwrap_or_else(|| "none".to_owned());
            entry.dirty = row.get("dirty").and_then(Value::as_bool).unwrap_or(false);
            entry.uid = string_field(&row, &["uid"]).unwrap_or_default();
            entry.thumbnail = row
                .get("thumbnail")
                .and_then(|thumbnail| string_field(thumbnail, &["kind", "strategy", "label"]))
                .unwrap_or_default();
        }
    }
}

pub(crate) fn package_writer_summary(state: &mut AssetsCatalogRuntimeState) -> Result<String, String> {
    let value = state.client.package_writer_info_json_v1(json!({}))?;
    let ops = value.get("operations").and_then(Value::as_object);
    let loose = ops.and_then(|o| o.get("loose_vfs_write_back")).and_then(Value::as_bool).unwrap_or(false);
    let listfile = ops.and_then(|o| o.get("nef8_listfile_repack")).and_then(Value::as_bool).unwrap_or(false);
    let nepak = ops.and_then(|o| o.get("nepak_container_write_back")).and_then(Value::as_bool).unwrap_or(false);
    Ok(format!("package writer: loose={} listfile={} nepak={}", loose, listfile, nepak))
}

pub(crate) fn import_queue_summary(state: &mut AssetsCatalogRuntimeState) -> Result<String, String> {
    let value = state.client.import_queue_json_v1(json!({}))?;
    if let Some(summary) = value.get("summary") {
        let queued = summary.get("queued").or_else(|| summary.get("queue_len")).and_then(Value::as_u64).unwrap_or(0);
        let active = summary.get("active").and_then(Value::as_u64).unwrap_or(0);
        return Ok(format!("import queue: queued={} active={}", queued, active));
    }
    let queued = value.get("queued").and_then(Value::as_array).map(|v| v.len()).unwrap_or(0);
    Ok(format!("import queue: queued={} active=0", queued))
}

pub(crate) fn import_summary_for_entries(entries: &[AssetsCatalogEntry]) -> String {
    let dirty = entries.iter().filter(|entry| entry.dirty).count();
    let reimport = entries.iter().filter(|entry| entry.import_action == "reimport").count();
    let import = entries.iter().filter(|entry| entry.import_action == "import").count();
    format!("Import status: {} dirty · {} reimport · {} new import", dirty, reimport, import)
}
