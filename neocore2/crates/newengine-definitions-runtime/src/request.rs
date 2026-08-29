use super::*;

pub(super) fn normalize_logical_ref(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Normalize separators and collapse duplicate slashes in one pass. The
    // previous replace/contains loop repeatedly rescanned and reallocated the
    // whole path for malformed input with many separators.
    let mut normalized = String::with_capacity(trimmed.len());
    let mut previous_was_slash = false;
    for character in trimmed.chars() {
        let character = if character == '\\' { '/' } else { character };
        if character == '/' {
            if previous_was_slash {
                continue;
            }
            previous_was_slash = true;
        } else {
            previous_was_slash = false;
        }
        normalized.push(character);
    }

    let mut start = 0usize;
    while normalized[start..].starts_with("./") {
        start += 2;
    }
    while normalized[start..].starts_with('/') {
        start += 1;
    }
    if start == 0 {
        normalized
    } else {
        normalized[start..].to_owned()
    }
}
pub(super) fn definition_ref_from_request(
    request: &DefinitionRefRequest,
) -> Result<String, String> {
    if !request.definition_ref.trim().is_empty() {
        return Ok(normalize_logical_ref(&request.definition_ref));
    }
    let source = normalize_logical_ref(&request.source);
    if source.is_empty() {
        return Err(
            "assets.definitions.entry_v1 requires definition_ref='definitions/foo.ytyp' or source='definitions/foo.ytyp'"
                .to_owned(),
        );
    }
    if let Some(entry) = request
        .entry
        .as_deref()
        .map(str::trim)
        .filter(|it| !it.is_empty())
    {
        Ok(format!("{source}@{entry}"))
    } else {
        Ok(source)
    }
}

pub(super) fn parse_definition_ref(
    request: &DefinitionRefRequest,
) -> Result<AssetReference, String> {
    let raw = definition_ref_from_request(request)?;
    newengine_assets_api::require_asset_reference_extension(&raw, &["ytyp"], false)
        .map_err(|e| e.to_string())
}

pub(super) fn manifest_source_from_request(
    request: &DefinitionManifestRequest,
) -> Result<String, String> {
    let raw = if !request.source.trim().is_empty() {
        request.source.trim()
    } else if !request.definition_ref.trim().is_empty() {
        request
            .definition_ref
            .split('@')
            .next()
            .unwrap_or(request.definition_ref.trim())
    } else {
        return Err("assets.definitions.manifest_v1 requires source='definitions/foo.ytyp' or definition_ref='definitions/foo.ytyp'".to_owned());
    };
    let normalized = normalize_logical_ref(raw);
    let reference =
        newengine_assets_api::require_asset_reference_extension(&normalized, &["ytyp"], false)
            .map_err(|e| e.to_string())?;
    Ok(reference.logical_path)
}

pub(super) fn ref_request_from_payload(
    payload: &[u8],
    method: &str,
) -> Result<DefinitionRefRequest, String> {
    let trimmed = std::str::from_utf8(payload)
        .map(str::trim)
        .map_err(|e| format!("{method} invalid utf-8 request: {e}"))?;
    if trimmed.is_empty() {
        return Err(format!("{method} requires definition_ref='.ytyp'"));
    }
    if trimmed.starts_with('{') {
        serde_json::from_str::<DefinitionRefRequest>(trimmed)
            .map_err(|e| format!("{method} invalid json request: {e}"))
    } else {
        Ok(DefinitionRefRequest {
            definition_ref: trimmed.trim_matches('"').to_owned(),
            ..Default::default()
        })
    }
}

pub(super) fn manifest_request_from_payload(
    payload: &[u8],
    method: &str,
) -> Result<DefinitionManifestRequest, String> {
    let trimmed = std::str::from_utf8(payload)
        .map(str::trim)
        .map_err(|e| format!("{method} invalid utf-8 request: {e}"))?;
    if trimmed.is_empty() {
        return Err(format!("{method} requires source='definitions/foo.ytyp'"));
    }
    if trimmed.starts_with('{') {
        serde_json::from_str::<DefinitionManifestRequest>(trimmed)
            .map_err(|e| format!("{method} invalid json request: {e}"))
    } else {
        Ok(DefinitionManifestRequest {
            source: trimmed.trim_matches('"').to_owned(),
            ..Default::default()
        })
    }
}
