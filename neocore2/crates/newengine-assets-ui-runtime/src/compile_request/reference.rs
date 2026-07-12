use super::*;

pub(crate) struct ResolvedUiRef {
    pub(crate) document_ref: String,
    pub(crate) logical_path: String,
    pub(crate) vfs_path: String,
    pub(crate) entry: String,
}

pub(crate) fn resolve_ui_ref(request: AssetsUiRefRequest) -> Result<ResolvedUiRef, String> {
    let combined = first_non_empty([request.document_ref.as_str(), request.ui_ref.as_str()]);
    let (path, entry) = if !combined.trim().is_empty() {
        split_ref(&combined)
    } else {
        let path = normalize_logical_path(&request.logical_path);
        let entry = normalize_entry(&request.entry);
        (path, entry)
    };
    if path.is_empty() {
        return Err(
            "engine.assets.ui request requires document_ref='path.neui@entry' or logical_path"
                .to_owned(),
        );
    }
    if !path.to_ascii_lowercase().ends_with(&format!(
        ".{}",
        newengine_asset_format_nef8::neui::EXTENSION
    )) {
        return Err(format!(
            "engine.assets.ui accepts only .{} dictionaries, got '{path}'",
            newengine_asset_format_nef8::neui::EXTENSION
        ));
    }
    let entry = if entry.is_empty() {
        "surface".to_owned()
    } else {
        entry
    };
    Ok(ResolvedUiRef {
        document_ref: format!("{}@{}", path, entry),
        logical_path: path.clone(),
        vfs_path: path,
        entry,
    })
}

pub(crate) fn canonical_request_ref(request: &AssetsUiCompileRequest) -> String {
    let combined = first_non_empty([request.document_ref.as_str(), request.ui_ref.as_str()]);
    if !combined.is_empty() {
        let (path, entry) = split_ref(&combined);
        return format!(
            "{}@{}",
            path,
            if entry.is_empty() { "surface" } else { &entry }
        );
    }
    format!(
        "{}@{}",
        normalize_logical_path(&request.logical_path),
        normalize_entry(&request.entry)
    )
}

pub(crate) fn split_ref(value: &str) -> (String, String) {
    let normalized = normalize_logical_path(value);
    if let Some((path, entry)) = normalized.split_once('@') {
        (normalize_logical_path(path), normalize_entry(entry))
    } else {
        (normalized, String::new())
    }
}

pub(crate) fn normalize_logical_path(value: &str) -> String {
    let mut out = value.trim().replace('\\', "/");
    while let Some(rest) = out.strip_prefix("./") {
        out = rest.to_owned();
    }
    out = out.trim_start_matches('/').to_owned();
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    out
}

pub(crate) fn normalize_entry(value: &str) -> String {
    value.trim().trim_start_matches('@').trim().to_owned()
}

pub(crate) fn vfs_candidates(path: &str) -> Vec<String> {
    let normalized = normalize_logical_path(path);
    let mut out = Vec::with_capacity(2);
    if let Some(stripped) = normalized.strip_prefix("assets/") {
        out.push(stripped.to_owned());
        out.push(normalized);
    } else {
        out.push(normalized.clone());
        out.push(format!("assets/{normalized}"));
    }
    out.dedup();
    out
}

pub(crate) fn validate_requested_entry(xml: &str, entry: &str) -> Result<(), String> {
    if entry.trim().is_empty() || entry == "surface" {
        return Ok(());
    }
    let entries_section = section(xml, "Entries").unwrap_or_default();
    let entries = elements(&entries_section, "Entry");
    if entries
        .iter()
        .any(|element| attr_value(&element.open, "name").as_deref() == Some(entry))
    {
        Ok(())
    } else {
        Err(format!(
            ".neui entry '@{}' is not declared in <Entries>",
            entry
        ))
    }
}
