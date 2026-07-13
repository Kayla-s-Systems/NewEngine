use super::*;

pub(crate) fn load_xmlcentral(
    state: &mut AssetsUiRuntimeState,
    request: AssetsUiRefRequest,
) -> Result<(String, Vec<String>, ResolvedUiRef), String> {
    let resolved = resolve_ui_ref(request)?;
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
                let xml = decode_neui_xmlcentral(&candidate, &bytes)?;
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
        "engine.assets.ui could not read .neui bytes for '{}': {}",
        resolved.document_ref,
        last_err.unwrap_or_else(|| "no candidate path".to_owned())
    ))
}

pub(crate) fn decode_neui_xmlcentral(logical_path: &str, bytes: &[u8]) -> Result<String, String> {
    let header = parse_list_file_header(bytes)?;
    if !header.content_kind_matches(LIST_FILE_CONTENT_KIND_NEUI) {
        return Err(format!(
            "{} is NEF8 content_kind='{}' ({}) not ui_dictionary ({})",
            logical_path,
            content_kind_label(header.content_kind),
            header.content_kind,
            LIST_FILE_CONTENT_KIND_NEUI
        ));
    }
    let start = usize::try_from(header.body_offset)
        .map_err(|_| "NEF8 body_offset does not fit usize".to_owned())?;
    let len = usize::try_from(header.body_len)
        .map_err(|_| "NEF8 body_len does not fit usize".to_owned())?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| "NEF8 body range overflow".to_owned())?;
    let compressed = bytes.get(start..end).ok_or_else(|| {
        format!(
            "NEF8 body range outside file: offset={} len={} file={}",
            start,
            len,
            bytes.len()
        )
    })?;

    let mut decoder = DeflateDecoder::new(compressed);
    let mut body = Vec::with_capacity(header.body_uncompressed_len as usize);
    decoder
        .read_to_end(&mut body)
        .map_err(|e| format!("NEF8 deflate body decode failed: {e}"))?;
    if header.body_uncompressed_len != 0 && body.len() != header.body_uncompressed_len as usize {
        return Err(format!(
            "NEF8 inflated body length mismatch: got={} expected={}",
            body.len(),
            header.body_uncompressed_len
        ));
    }
    let hash = blake3::hash(&body);
    if header.has_body_raw_hash() && header.body_raw_hash != *hash.as_bytes() {
        return Err("NEF8 inflated body BLAKE3 hash mismatch".to_owned());
    }
    let xml =
        String::from_utf8(body).map_err(|e| format!(".neui XMLcentral body is not UTF-8: {e}"))?;
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
