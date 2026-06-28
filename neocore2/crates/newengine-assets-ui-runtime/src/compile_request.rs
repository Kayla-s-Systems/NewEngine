use super::*;

pub(crate) fn compile_request_from_ref(request: AssetsUiRefRequest) -> AssetsUiCompileRequest {
    AssetsUiCompileRequest {
        document_ref: first_non_empty([request.document_ref.as_str(), request.ui_ref.as_str()]),
        ui_ref: String::new(),
        logical_path: request.logical_path,
        entry: request.entry,
        mount_runtime: false,
        ..Default::default()
    }
}

pub(crate) fn error_response_from_message(message: String) -> AssetsUiDiagnosticResponse {
    AssetsUiDiagnosticResponse {
        message,
        ..Default::default()
    }
}

pub(crate) fn error_response_from_compile_error(
    message: String,
    request: &AssetsUiCompileRequest,
) -> AssetsUiDiagnosticResponse {
    let combined = first_non_empty([request.document_ref.as_str(), request.ui_ref.as_str()]);
    let (path, entry) = if !combined.trim().is_empty() {
        split_ref(&combined)
    } else {
        (
            normalize_logical_path(&request.logical_path),
            normalize_entry(&request.entry),
        )
    };
    let entry = if entry.trim().is_empty() {
        "surface".to_owned()
    } else {
        entry
    };
    AssetsUiDiagnosticResponse {
        document_ref: if path.trim().is_empty() {
            String::new()
        } else {
            format!("{}@{}", path, entry)
        },
        logical_path: path.clone(),
        entry: entry.clone(),
        entry_id: entry,
        source_span: UiSourceSpan {
            source_ref: path,
            line: 0,
            column: 0,
        },
        message,
        ..Default::default()
    }
}

pub(crate) struct ResolvedUiRef {
    pub(crate) document_ref: String,
    pub(crate) logical_path: String,
    pub(crate) vfs_path: String,
    pub(crate) entry: String,
}

pub(crate) fn compile_document(
    state: &mut AssetsUiRuntimeState,
    request: AssetsUiCompileRequest,
) -> Result<AssetsUiCompileResponse, String> {
    let cache_key = canonical_request_ref(&request);
    if let Some(cached) = state.compile_cache.get(&cache_key) {
        return Ok(cached.clone());
    }

    let ref_request = AssetsUiRefRequest {
        document_ref: request.document_ref.clone(),
        ui_ref: request.ui_ref.clone(),
        logical_path: request.logical_path.clone(),
        entry: request.entry.clone(),
    };
    let (xml, mut warnings, resolved) = load_xmlcentral(state, ref_request)?;
    validate_requested_entry(&xml, &resolved.entry).map_err(|err| {
        let span = source_span_for_named_element(&xml, "Entries", &resolved.document_ref);
        format!(
            "{} entry='@{}' {}: {}",
            resolved.document_ref,
            resolved.entry,
            span.display(&resolved.document_ref),
            err
        )
    })?;

    let surface = parse_surface(&xml).ok_or_else(|| {
        let span = source_span_for_offset(&xml, 0, &resolved.document_ref);
        format!(
            "{} entry='@{}' {}: .neui document has no <Surface> entry",
            resolved.document_ref,
            resolved.entry,
            span.display(&resolved.document_ref)
        )
    })?;
    let mut dependencies = extract_dependencies(&xml);
    let inferred_style_ref = request
        .style_ref
        .clone()
        .or_else(|| surface.theme.clone())
        .or_else(|| first_dependency_with_suffix(&dependencies, ".neuis"))
        .or_else(|| first_dependency_with_suffix(&dependencies, ".neui@theme"));
    if let Some(style_ref) = &inferred_style_ref {
        if !dependencies.iter().any(|dep| dep == style_ref) {
            dependencies.push(style_ref.clone());
            dependencies.sort();
            dependencies.dedup();
        }
    }
    let style_dependencies = inferred_style_ref.iter().cloned().collect::<Vec<_>>();
    let binding_plan = parse_binding_plan(&xml, &resolved.document_ref, &surface.name);
    let component_libraries = parse_component_libraries(&xml);
    let theme_libraries = parse_theme_libraries(&xml, surface.theme.as_deref());
    let local_component_templates = parse_component_templates(&xml, &resolved.document_ref);
    let imported_component_templates =
        resolve_imported_component_templates(state, &component_libraries, &mut warnings);
    let component_templates =
        merge_component_templates(imported_component_templates, local_component_templates);
    let theme_tokens = resolve_theme_token_bundle(
        state,
        &theme_libraries,
        inferred_style_ref.as_deref(),
        &mut warnings,
    );
    let mut root = compile_surface_root(
        &xml,
        &surface,
        &resolved.document_ref,
        inferred_style_ref.as_deref(),
    )?;
    if let Some(tokens) = theme_tokens.as_ref() {
        root.props.insert(
            "theme_tokens".to_owned(),
            serde_json::to_value(tokens).unwrap_or(serde_json::Value::Null),
        );
        root.style_tags
            .push(format!("density:{}", sanitize_tag(&tokens.density)));
        root.style_tags.sort();
        root.style_tags.dedup();
    }
    warnings.push(format!(
        ".neui live root compiled source='{}' entry='@{}' surface='{}' root_node='{}' children={} component_libraries={} theme_libraries={} component_templates={} theme_tokens={}",
        resolved.document_ref,
        resolved.entry,
        surface.name,
        root.id,
        root.children.len(),
        component_libraries.len(),
        theme_libraries.len(),
        component_templates.len(),
        theme_tokens.as_ref().map(|tokens| tokens.theme_id.as_str()).unwrap_or("<none>")
    ));
    let source = UiDocumentSource {
        kind: request.source_kind,
        document_ref: resolved.document_ref.clone(),
        style_ref: inferred_style_ref.clone(),
        stream_id: request.stream_id.clone(),
        generator_id: request.generator_id.clone(),
    };
    let compiled_document = UiCompiledDocument {
        version: 1,
        source: source.clone(),
        document_ref: resolved.document_ref.clone(),
        surface_id: surface.name.clone(),
        root_id: surface.root.clone(),
        theme_ref: surface.theme.clone(),
        style_ref: inferred_style_ref.clone(),
        dependencies: dependencies.clone(),
        style_dependencies: style_dependencies.clone(),
        component_libraries,
        theme_libraries,
        component_templates,
        root: Some(root),
        binding_plan,
        validation: Default::default(),
        dependency_report: Default::default(),
    };
    let navigation_document = match parse_navigation_document(&xml)? {
        Some(document) => Some(document),
        None => derive_navigation_document_from_surface_layout(&xml, &surface)?,
    };

    let response = AssetsUiCompileResponse {
        ok: true,
        document_ref: resolved.document_ref.clone(),
        logical_path: resolved.logical_path.clone(),
        vfs_path: resolved.vfs_path.clone(),
        entry: resolved.entry.clone(),
        surface_id: surface.name,
        xmlcentral: xml,
        compiled_document,
        navigation_document,
        source_kind: request.source_kind,
        style_ref: inferred_style_ref,
        dependencies,
        style_dependencies,
        warnings,
        ..Default::default()
    };
    state.compile_cache.insert(cache_key, response.clone());
    Ok(response)
}

pub(crate) fn load_xmlcentral(
    state: &mut AssetsUiRuntimeState,
    request: AssetsUiRefRequest,
) -> Result<(String, Vec<String>, ResolvedUiRef), String> {
    let resolved = resolve_ui_ref(request)?;
    if let Some(xml) = state.xml_cache.get(&resolved.vfs_path) {
        return Ok((xml.clone(), Vec::new(), resolved));
    }

    let mut warnings = Vec::new();
    let mut last_err = None;
    for candidate in vfs_candidates(&resolved.logical_path) {
        match state.client.raw_bytes_v1(&candidate) {
            Ok(bytes) => {
                let xml = decode_neui_xmlcentral(&candidate, &bytes)?;
                state.xml_cache.insert(candidate.clone(), xml.clone());
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
    let header = parse_list_file_header_v1(bytes)?;
    if header.content_kind != LIST_FILE_CONTENT_KIND_NEUI {
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
    if body.len() != header.body_uncompressed_len as usize {
        return Err(format!(
            "NEF8 inflated body length mismatch: got={} expected={}",
            body.len(),
            header.body_uncompressed_len
        ));
    }
    let hash = blake3::hash(&body);
    if header.body_raw_hash != *hash.as_bytes() {
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
    ) {
        return Err(format!("unsupported .neui XMLcentral root '{root}'"));
    }
    Ok(xml)
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
