//! Small JSON projection helpers for editor UI surfaces.
//!
//! These helpers keep `lib.rs` focused on module lifecycle and event handling;
//! they intentionally do not interpret asset semantics beyond DTO field names
//! returned by existing `engine.assets` routes.

use serde_json::Value;

pub(crate) fn source_labels(value: &Value) -> Vec<String> {
    value
        .get("sources")
        .and_then(Value::as_array)
        .or_else(|| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|item| string_field(item, &["id", "name", "root", "logical_root"]))
        .take(64)
        .collect()
}

pub(crate) fn format_labels(value: &Value) -> Vec<String> {
    value
        .get("formats")
        .and_then(Value::as_array)
        .or_else(|| value.get("descriptors").and_then(Value::as_array))
        .or_else(|| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|item| string_field(item, &["extension", "id", "asset_kind", "content_kind"]))
        .take(64)
        .collect()
}

pub(crate) fn value_warnings(value: &Value) -> Vec<String> {
    value
        .get("warnings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

pub(crate) fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

pub(crate) fn bool_field(value: &Value, keys: &[&str]) -> bool {
    keys.iter()
        .any(|key| value.get(*key).and_then(Value::as_bool).unwrap_or(false))
}

pub(crate) fn extension_from(name: &str, value: &Value) -> String {
    if let Some(ext) = string_field(value, &["extension", "ext"]) {
        return ext.trim_start_matches('.').to_ascii_lowercase();
    }
    name.rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}
