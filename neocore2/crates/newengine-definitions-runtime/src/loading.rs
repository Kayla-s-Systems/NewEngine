use super::*;

fn load_ytyp_semantic_body(
    state: &DefinitionsRuntimeState,
    source: &str,
) -> Result<(Vec<u8>, Vec<String>), String> {
    match state.client.raw_bytes_v1(source) {
        Ok(body)
            if authored_xml::body_is_xml(&body)
                || serde_json::from_slice::<serde_json::Value>(&body).is_ok() =>
        {
            Ok((
                body,
                vec![
                    ".ytyp loose authoring body read through engine.assets raw_bytes_v1".to_owned(),
                ],
            ))
        }
        Ok(_encoded_container) => {
            let request = AssetDecodeRequest {
                logical_path: source.to_owned(),
                output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
                selector: serde_json::Value::Null,
                format_descriptor: None,
            };
            state
                .client
                .decode_v1(&request)
                .map(|body| {
                    (
                        body,
                        vec![".ytyp encoded/source-dictionary body decoded through engine.assets".to_owned()],
                    )
                })
                .map_err(|decode_error| {
                    format!("engine.assets.definitions: encoded .ytyp source requires asset.decode_v1 source='{source}' err='{decode_error}'")
                })
        }
        Err(read_error) => {
            let request = AssetDecodeRequest {
                logical_path: source.to_owned(),
                output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
                selector: serde_json::Value::Null,
                format_descriptor: None,
            };
            state
                .client
                .decode_v1(&request)
                .map(|body| {
                    (
                        body,
                        vec![".ytyp body decoded through engine.assets after raw_bytes_v1 miss".to_owned()],
                    )
                })
                .map_err(|decode_error| {
                    format!("engine.assets.definitions: .ytyp unavailable source='{source}' read_err='{read_error}' decode_err='{decode_error}'")
                })
        }
    }
}

fn load_properties_body(
    state: &DefinitionsRuntimeState,
    source: &str,
) -> Result<(Vec<RawDefinitionEntryV1>, Vec<String>), String> {
    let (body, mut warnings) = load_ytyp_semantic_body(state, source)?;
    if authored_xml::body_is_xml(&body) {
        let (entries, mut parse_warnings) = parse_ytyp_xml_document(source, &body)?;
        warnings.append(&mut parse_warnings);
        warnings
            .push(".ytyp loose XML authoring body adapted into archetype metadata DTO".to_owned());
        return Ok((entries, warnings));
    }
    let (entries, mut parse_warnings) = parse_ytyp_json_document(source, &body)?;
    warnings.append(&mut parse_warnings);
    warnings.push(".ytyp semantic body parsed as archetype metadata DTO".to_owned());
    Ok((entries, warnings))
}

pub(super) fn load_manifest(
    state: &DefinitionsRuntimeState,
    source: &str,
) -> Result<DefinitionManifestV1, String> {
    let (raw_entries, warnings) = load_properties_body(state, source)?;
    let mut entries = Vec::with_capacity(raw_entries.len());
    for raw in raw_entries {
        let name = raw.name.trim().to_owned();
        if name.is_empty() {
            continue;
        }
        let (semantic_tags, domain_tags) = collect_tags(&raw);
        entries.push(DefinitionManifestEntryV1 {
            stable_hash: effective_hash(&raw),
            kind: effective_kind(&raw),
            definition_ref: format!("{source}@{name}"),
            name,
            semantic_tags,
            domain_tags,
        });
    }
    entries.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.stable_hash.cmp(&b.stable_hash))
    });
    Ok(DefinitionManifestV1 {
        schema: "newengine.assets.definitions.manifest.v1",
        gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        source: source.to_owned(),
        status: "definition_dictionary_manifest",
        entries,
        warnings,
    })
}

fn ytyp_sidecar_source(source: &str, entry_selector: &str) -> Option<String> {
    let entry = entry_selector.trim();
    if entry.is_empty() {
        return None;
    }
    let source = source.trim().replace('\\', "/");
    let (dir, _) = source.rsplit_once('/')?;
    let candidate = format!("{dir}/{entry}.ytyp");
    (candidate != source).then_some(candidate)
}

fn load_entry_from_source(
    state: &DefinitionsRuntimeState,
    source: &str,
    entry_selector: &str,
) -> Result<DefinitionEntryV1, String> {
    let (raw_entries, warnings) = load_properties_body(state, source)?;
    for raw in raw_entries {
        if entry_selector.trim().is_empty() || entry_matches(&raw, entry_selector) {
            return build_entry(source, raw, &warnings);
        }
    }
    Err(format!(
        "engine.assets.definitions: Definition Entry not found source='{}' selector='{}'",
        source, entry_selector
    ))
}

pub(super) fn load_entry(
    state: &DefinitionsRuntimeState,
    request: DefinitionRefRequest,
) -> Result<DefinitionEntryV1, String> {
    let reference = parse_definition_ref(&request)?;
    let entry_selector = reference.entry.as_deref().unwrap_or_default().to_owned();
    if let Some(sidecar_source) = ytyp_sidecar_source(&reference.logical_path, &entry_selector) {
        match load_entry_from_source(state, &sidecar_source, &entry_selector) {
            Ok(mut entry) => {
                entry.identity.definition_ref = reference.canonical.clone();
                entry.warnings.push(format!(
                    ".ytyp Definition Entry resolved through sidecar source='{sidecar_source}' canonical_ref='{}'",
                    reference.canonical
                ));
                return Ok(entry);
            }
            Err(sidecar_error) => {
                let primary =
                    load_entry_from_source(state, &reference.logical_path, &entry_selector);
                return primary.map_err(|primary_error| {
                    format!(
                        "engine.assets.definitions: Definition Entry not found ref='{}' sidecar='{}' sidecar_err='{}' primary='{}'",
                        reference.canonical, sidecar_source, sidecar_error, primary_error
                    )
                });
            }
        }
    }
    load_entry_from_source(state, &reference.logical_path, &entry_selector).map_err(
        |primary_error| {
            format!(
                "engine.assets.definitions: Definition Entry not found ref='{}' err='{}'",
                reference.canonical, primary_error
            )
        },
    )
}

fn flatten_refs(refs: &DefinitionRefsV1) -> Vec<String> {
    let mut all = Vec::new();
    all.extend(refs.drawable_refs.iter().cloned());
    all.extend(refs.material_refs.iter().cloned());
    all.extend(refs.texture_refs.iter().cloned());
    all.extend(refs.uv_layout_refs.iter().cloned());
    all.extend(refs.physics_refs.iter().cloned());
    all.extend(refs.collision_refs.iter().cloned());
    all.extend(refs.ai_refs.iter().cloned());
    all.extend(refs.streaming_refs.iter().cloned());
    all.extend(refs.editor_refs.iter().cloned());
    all.extend(refs.other_refs.iter().cloned());
    all.sort();
    all.dedup();
    all
}

pub(super) fn validate_entry(
    state: &DefinitionsRuntimeState,
    request: DefinitionRefRequest,
) -> DefinitionValidationV1 {
    match load_entry(state, request) {
        Ok(entry) => DefinitionValidationV1 {
            ok: true,
            gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            byte_owner: ENGINE_ASSET_SERVICE_ID,
            semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            definition_ref: entry.identity.definition_ref,
            code: "definitions.ok",
            message: ".ytyp Definition Entry is valid metadata; no imperative side-effect fields detected".to_owned(),
            warnings: entry.warnings,
        },
        Err(message) => DefinitionValidationV1 {
            ok: false,
            gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            byte_owner: ENGINE_ASSET_SERVICE_ID,
            semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            definition_ref: String::new(),
            code: "definitions.invalid_entry",
            message,
            warnings: Vec::new(),
        },
    }
}

pub(super) fn resolve_definition_refs(
    state: &mut DefinitionsRuntimeState,
    request: DefinitionRefRequest,
) -> Result<DefinitionRefResolutionV1, String> {
    let entry = load_entry(state, request)?;
    let flattened_refs = flatten_refs(&entry.refs);
    Ok(DefinitionRefResolutionV1 {
        ok: true,
        gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        definition_ref: entry.identity.definition_ref,
        refs: entry.refs,
        flattened_refs,
        resolver: ENGINE_ASSETS_GRAPH_SERVICE_ID,
        warnings: entry.warnings,
    })
}

pub(super) fn describe_definition_side_effects(
    state: &mut DefinitionsRuntimeState,
    request: DefinitionRefRequest,
) -> Result<serde_json::Value, String> {
    let entry = load_entry(state, request)?;
    Ok(serde_json::json!({
        "ok": true,
        "gateway": ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        "byte_owner": ENGINE_ASSET_SERVICE_ID,
        "semantic_owner": ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        "definition_ref": entry.identity.definition_ref,
        "side_effect_policy": "declarative-only; allowed shape is {domain,effect,target}; imperative run_code/script/call/function fields are rejected",
        "side_effects": entry.side_effects,
        "domain_tags": entry.domain_tags,
    }))
}
