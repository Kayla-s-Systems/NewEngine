use super::*;

pub(crate) fn snapshot(
    state: &mut AssetsCatalogRuntimeState,
    logical_path: &str,
    _selected_index: usize,
) -> Result<AssetsCatalogSnapshot, String> {
    let logical_path = normalize_catalog_path(logical_path);
    let listing = match state.client.vfs_list_json_v1(&logical_path) {
        Ok(listing) => listing,
        Err(vfs_error) => {
            return snapshot_from_list_file(state, &logical_path).map_err(|entry_error| {
                format!("engine.assets catalog path unavailable: vfs='{vfs_error}' listFile='{entry_error}'")
            });
        }
    };
    let mut warnings = value_warnings(&listing);
    let mut entries = listing
        .get("entries")
        .and_then(|value| value.as_array())
        .map(|items| items.iter().map(entry_from_vfs_value).collect::<Vec<_>>())
        .unwrap_or_default();
    entries.sort_by(|a, b| {
        b.is_directory().cmp(&a.is_directory()).then_with(|| {
            a.name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase())
        })
    });

    apply_import_lifecycle_rows(state, &logical_path, &mut entries, &mut warnings);
    hydrate_preview_plans_for_entries(state, &mut entries, &mut warnings);

    let sources = match state.client.sources_json_v1() {
        Ok(value) => source_labels(&value),
        Err(error) => {
            warnings.push(format!("engine.assets sources unavailable: {error}"));
            Vec::new()
        }
    };
    let formats = match state.client.formats_json_v1() {
        Ok(value) => format_labels(&value),
        Err(error) => {
            warnings.push(format!("engine.assets formats unavailable: {error}"));
            Vec::new()
        }
    };

    let package_writer_summary = package_writer_summary(state).unwrap_or_else(|error| {
        warnings.push(format!("engine.assets.package_writer unavailable: {error}"));
        "package writer unavailable".to_owned()
    });
    let import_queue_summary = import_queue_summary(state).unwrap_or_else(|error| {
        warnings.push(format!("engine.assets.import_queue unavailable: {error}"));
        "import queue unavailable".to_owned()
    });
    let import_summary = import_summary_for_entries(&entries);
    let route_diagnostics = "routes: engine.assets · engine.assets.types · engine.assets.inspect · engine.assets.edit · engine.ui surface node".to_owned();

    Ok(AssetsCatalogSnapshot {
        logical_path,
        entries,
        sources,
        formats,
        warnings,
        import_summary,
        import_queue_summary,
        package_writer_summary,
        route_diagnostics,
    })
}

pub(crate) fn snapshot_from_list_file(
    state: &mut AssetsCatalogRuntimeState,
    logical_path: &str,
) -> Result<AssetsCatalogSnapshot, String> {
    let logical_path = normalize_catalog_path(logical_path);
    if logical_path.is_empty() || logical_path.contains('@') {
        return Err(
            "entry directory requires a concrete ListFile path without @entry selector".to_owned(),
        );
    }
    let request = AssetDecodeRequest {
        logical_path: logical_path.clone(),
        output_kind: ASSET_LIST_FILE_MANIFEST_OUTPUT.to_owned(),
        selector: json!({}),
        format_descriptor: None,
    };
    let bytes = state.client.decode_v1(&request)?;
    let manifest = serde_json::from_slice::<AssetFileManifest>(&bytes)
        .map_err(|error| format!("provider returned invalid AssetFileManifest: {error}"))?;
    if manifest.entries.is_empty() {
        return Err("provider manifest contains no addressable entries".to_owned());
    }

    let source_extension = path_extension_from_ref(&logical_path);
    let mut entries = manifest
        .entries
        .iter()
        .map(|entry| AssetsCatalogEntry {
            name: entry.name.clone(),
            kind: "asset_entry".to_owned(),
            logical_path: entry.entry_ref.clone(),
            extension: source_extension.clone(),
            semantic_gateway: if entry.route.gateway.trim().is_empty() {
                "engine.assets.inspect".to_owned()
            } else {
                entry.route.gateway.clone()
            },
            asset_kind: if entry.asset_kind.trim().is_empty() {
                manifest.file_kind.clone()
            } else {
                entry.asset_kind.clone()
            },
            import_stage: "listfile_entry".to_owned(),
            import_action: "inspect/edit".to_owned(),
            dirty: false,
            uid: entry.stable_id.clone(),
            thumbnail: String::new(),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });

    let mut warnings = manifest.warnings.clone();
    warnings.extend(
        manifest
            .policy
            .iter()
            .map(|policy| format!("policy: {policy}")),
    );
    let sources = match state.client.sources_json_v1() {
        Ok(value) => source_labels(&value),
        Err(error) => {
            warnings.push(format!("engine.assets sources unavailable: {error}"));
            Vec::new()
        }
    };
    let formats = match state.client.formats_json_v1() {
        Ok(value) => format_labels(&value),
        Err(error) => {
            warnings.push(format!("engine.assets formats unavailable: {error}"));
            Vec::new()
        }
    };
    let package_writer_summary = package_writer_summary(state).unwrap_or_else(|error| {
        warnings.push(format!("engine.assets.package_writer unavailable: {error}"));
        "package writer unavailable".to_owned()
    });
    let import_queue_summary = import_queue_summary(state).unwrap_or_else(|error| {
        warnings.push(format!("engine.assets.import_queue unavailable: {error}"));
        "import queue unavailable".to_owned()
    });
    let import_summary = format!(
        "{} addressable entries from {}",
        entries.len(),
        manifest.file_kind
    );
    let route_diagnostics = format!(
        "ListFile directory: {} -> entries as file@entry refs · inspect=engine.assets.inspect · edit=engine.assets.edit",
        logical_path
    );

    Ok(AssetsCatalogSnapshot {
        logical_path,
        entries,
        sources,
        formats,
        warnings,
        import_summary,
        import_queue_summary,
        package_writer_summary,
        route_diagnostics,
    })
}

pub(crate) fn path_extension_from_ref(path: &str) -> String {
    path.split('@')
        .next()
        .unwrap_or(path)
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

pub(crate) fn entry_from_vfs_value(value: &Value) -> AssetsCatalogEntry {
    let name = string_field(value, &["name", "file_name", "display_name"])
        .unwrap_or_else(|| "<unnamed>".to_owned());
    let logical_path = string_field(value, &["logical_path", "path", "id", "reference"])
        .unwrap_or_else(|| name.clone());
    let kind = string_field(value, &["kind", "node_kind", "entry_kind"]).unwrap_or_else(|| {
        (if bool_field(value, &["is_dir", "directory", "is_directory"]) {
            "directory"
        } else {
            "asset"
        })
        .to_owned()
    });
    let extension = extension_from(&name, value);
    AssetsCatalogEntry {
        name,
        kind,
        logical_path: normalize_catalog_path(&logical_path),
        extension,
        semantic_gateway: string_field(value, &["semantic_gateway", "gateway"])
            .unwrap_or_else(|| "engine.assets".to_owned()),
        asset_kind: string_field(value, &["asset_kind", "content_kind", "type"])
            .unwrap_or_else(|| "asset".to_owned()),
        import_stage: "unknown".to_owned(),
        import_action: "scan".to_owned(),
        dirty: false,
        uid: String::new(),
        thumbnail: String::new(),
    }
}

pub(crate) fn component_id_fragment(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').to_owned();
    if trimmed.is_empty() {
        "node".to_owned()
    } else {
        trimmed
    }
}
