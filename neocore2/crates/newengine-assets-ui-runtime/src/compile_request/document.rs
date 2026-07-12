use super::*;

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
    let dialect_ref =
        extract_dialect_ref(&xml).unwrap_or_else(|| DEFAULT_NEUI_DIALECT_REF.to_owned());
    let dialect = load_neui_dialect(state, &dialect_ref, &mut warnings);
    let mut root = compile_surface_root(
        &xml,
        &surface,
        &resolved.document_ref,
        inferred_style_ref.as_deref(),
        &dialect,
    )?;
    root.props.insert(
        "dialect_ref".to_owned(),
        serde_json::Value::String(dialect_ref.clone()),
    );
    root.props.insert(
        "dialect_id".to_owned(),
        serde_json::Value::String(dialect.id.clone()),
    );
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
