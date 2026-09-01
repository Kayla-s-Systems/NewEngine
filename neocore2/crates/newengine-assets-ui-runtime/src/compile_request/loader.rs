use super::*;

pub(crate) fn load_xmlcentral(
    state: &mut AssetsUiRuntimeState,
    request: AssetsUiRefRequest,
) -> Result<(String, Vec<String>, ResolvedUiRef), String> {
    let resolved = resolve_ui_ref(request)?;
    let descriptor = state.client.resolve_file_type_v1(&resolved.logical_path)?;
    if !descriptor.semantic_gateway.eq_ignore_ascii_case("engine.assets.ui") {
        return Err(format!(
            "engine.assets.ui rejected format module='{}' kind='{}' gateway='{}' path='{}'",
            descriptor.module_id,
            descriptor.asset_kind,
            descriptor.semantic_gateway,
            resolved.logical_path
        ));
    }
    let content_kind = descriptor.content_kind.ok_or_else(|| {
        format!(
            "engine.assets.ui format module='{}' has no ListFile content_kind",
            descriptor.module_id
        )
    })?;
    let cache_key = resolved.logical_path.clone();
    if let Some(cached) = state.xml_cache.get(&cache_key) {
        let actual = ResolvedUiRef {
            vfs_path: cached.vfs_path.clone(),
            ..resolved
        };
        return Ok((cached.xml.clone(), Vec::new(), actual));
    }

    let mut warnings = Vec::new();
    let mut last_err = None;
    for candidate in vfs_candidates(&resolved.logical_path) {
        match state.client.raw_bytes_v1(&candidate) {
            Ok(bytes) => {
                let xml = decode_neui_xmlcentral(&candidate, &bytes, content_kind)?;
                state.xml_cache.insert(
                    cache_key,
                    CachedXmlCentral {
                        xml: xml.clone(),
                        vfs_path: candidate.clone(),
                    },
                );
                let actual = ResolvedUiRef {
                    vfs_path: candidate,
                    ..resolved
                };
                return Ok((xml, warnings, actual));
            }
            Err(e) => {
                last_err = Some(format!("{}: {}", candidate, e));
            }
        }
    }
    warnings.push("VFS lookup tried both literal and assets/-stripped paths".to_owned());
    Err(format!(
        "engine.assets.ui could not read registered UI dictionary bytes for '{}': {}",
        resolved.document_ref,
        last_err.unwrap_or_else(|| "no candidate path".to_owned())
    ))
}

pub(crate) fn decode_neui_xmlcentral(
    logical_path: &str,
    bytes: &[u8],
    expected_content_kind: u32,
) -> Result<String, String> {
    let decoded = newengine_assets_api::decode_list_file_envelope(
        bytes,
        expected_content_kind,
        logical_path,
    )?;
    let xml = String::from_utf8(decoded.body)
        .map_err(|e| format!(".neui XMLcentral body is not UTF-8: {e}"))?;
    let Some(root) = root_name(&xml) else {
        return Err(".neui XMLcentral body has no root element".to_owned());
    };
    if !matches!(
        root,
        "NeUiDictionary"
            | "NeUiRegistry"
            | "NeUiThemeLibrary"
            | "NeUiComponentLibrary"
            | "NeUiBindingLibrary"
            | "NeUiDialect"
    ) {
        return Err(format!("unsupported .neui XMLcentral root '{root}'"));
    }
    Ok(xml)
}
