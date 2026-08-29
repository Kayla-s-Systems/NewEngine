use super::*;

pub(super) fn vfs_source_from_trace(path: &str, trace: &serde_json::Value) -> AssetGraphVfsSource {
    let source = first_object(
        trace,
        &["selected", "source", "resolved", "winner", "active_source"],
    )
    .unwrap_or(trace);
    let source_kind = first_string(source, &["source_kind", "kind", "layer_kind", "type"])
        .unwrap_or_else(|| infer_source_kind(source));
    let physical_path = first_string(
        source,
        &["physical_path", "path", "resolved_path", "filesystem_path"],
    );
    let package_path = first_string(
        source,
        &["package_path", "container_path", "nepak", "package"],
    );
    let package_entry = first_string(source, &["package_entry", "entry", "virtual_path"]);
    let layer_id = first_string(source, &["layer_id", "mount_id", "source_id"]);
    let overridden_by = source
        .get("overridden_by")
        .or_else(|| source.get("shadowed_by"))
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default();
    AssetGraphVfsSource {
        source_kind,
        logical_path: path.to_owned(),
        physical_path,
        package_path,
        package_entry,
        layer_id,
        overridden_by,
    }
}

fn first_object<'a>(value: &'a serde_json::Value, keys: &[&str]) -> Option<&'a serde_json::Value> {
    for key in keys {
        if let Some(object) = value.get(*key).filter(|v| v.is_object()) {
            return Some(object);
        }
    }
    None
}

fn first_string(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = value
            .get(*key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
        {
            return Some(text.to_owned());
        }
    }
    None
}

fn infer_source_kind(value: &serde_json::Value) -> String {
    if value
        .get("package_path")
        .or_else(|| value.get("container_path"))
        .is_some()
    {
        return "nepak_package".to_owned();
    }
    if value
        .get("physical_path")
        .or_else(|| value.get("filesystem_path"))
        .is_some()
    {
        return "loose_file".to_owned();
    }
    "unresolved".to_owned()
}
