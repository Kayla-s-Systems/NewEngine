use super::*;

pub(crate) fn load_neui_dialect(
    state: &mut AssetsUiRuntimeState,
    dialect_ref: &str,
    warnings: &mut Vec<String>,
) -> NeUiDialect {
    let (path, entry) = split_ref(dialect_ref);
    let path = normalize_logical_path(&path);
    let entry = if entry.trim().is_empty() {
        "dialect".to_owned()
    } else {
        entry
    };
    let cache_key = format!("{}@{}", path, entry);
    if let Some(cached) = state.dialect_cache.get(&cache_key) {
        return cached.clone();
    }

    let mut last_err = None;
    for candidate in vfs_candidates(&path) {
        match state.client.raw_bytes_v1(&candidate) {
            Ok(bytes) => {
                match decode_neui_xmlcentral(&candidate, &bytes, LIST_FILE_CONTENT_KIND_NEUI)
                    .and_then(|xml| NeUiDialect::from_xml(&xml, &cache_key))
                {
                    Ok(dialect) => {
                        warnings.push(format!(
                        ".neui dialect loaded ref='{}' id='{}' source='{}' policy='asset-backed compiler dialect'",
                        cache_key, dialect.id, candidate
                    ));
                        state
                            .dialect_cache
                            .insert(cache_key.clone(), dialect.clone());
                        return dialect;
                    }
                    Err(error) => {
                        last_err = Some(format!("{}: {}", candidate, error));
                    }
                }
            }
            Err(error) => {
                last_err = Some(format!("{}: {}", candidate, error));
            }
        }
    }

    let dialect = NeUiDialect::builtin();
    warnings.push(format!(
        ".neui dialect fallback ref='{}' id='{}' err='{}' policy='bootstrap fallback only; supply ui/dialects/runtime.neui to avoid static dialect'",
        cache_key,
        dialect.id,
        last_err.unwrap_or_else(|| "no candidate path".to_owned())
    ));
    state.dialect_cache.insert(cache_key, dialect.clone());
    dialect
}

pub(crate) fn extract_dialect_ref(xml: &str) -> Option<String> {
    first_element(xml, "DialectRef")
        .and_then(|element| {
            attr_value(&element.open, "ref").or_else(|| attr_value(&element.open, "document_ref"))
        })
        .filter(|it| !it.trim().is_empty())
}

pub(crate) fn inspect_dialect(
    state: &mut AssetsUiRuntimeState,
    request: AssetsUiDialectInspectRequest,
) -> serde_json::Value {
    let dialect_ref = if request.dialect_ref.trim().is_empty() {
        DEFAULT_NEUI_DIALECT_REF.to_owned()
    } else {
        request.dialect_ref.trim().to_owned()
    };
    let mut warnings = Vec::new();
    let dialect = load_neui_dialect(state, &dialect_ref, &mut warnings);
    dialect.inspect_json(
        &canonical_dialect_ref(&dialect_ref),
        "engine.assets.ui",
        warnings,
    )
}
pub(crate) fn canonical_dialect_ref(value: &str) -> String {
    let (path, entry) = split_ref(value);
    let path = normalize_logical_path(&path);
    let entry = if entry.trim().is_empty() {
        "dialect".to_owned()
    } else {
        entry
    };
    format!("{}@{}", path, entry)
}
