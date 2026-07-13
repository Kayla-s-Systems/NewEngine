//! Runtime implementation for `engine.assets.graph`.
//!
//! This module is intentionally split out of `lib.rs`: graph hydration is a
//! provider implementation detail, while `lib.rs` keeps the public model gateway
//! facade and adapter code.

use super::*;

#[derive(Clone)]
struct AssetGraphGatewayState {
    host: HostApiV1,
    client: AssetServiceClient,
}

impl AssetGraphGatewayState {
    fn resolve(&self, root_ref: &str) -> ResolvedAssetGraphV2 {
        let graph = RuntimeAssetGraphResolver::new(self.host.clone(), self.client.clone())
            .resolve(root_ref);
        log::info!(
            "engine.assets.graph: resolved root='{}' nodes={} edges={} missing={} cycles={} warnings={} cache_key='{}'",
            graph.root_ref,
            graph.nodes.len(),
            graph.edges.len(),
            graph.missing_refs.len(),
            graph.cycle_errors.len(),
            graph.format_warnings.len(),
            graph.stable_cache_key
        );
        if !graph.missing_refs.is_empty() || !graph.cycle_errors.is_empty() {
            log::warn!(
                "engine.assets.graph: incomplete graph root='{}' missing={} cycles={} policy='demo can continue only if downstream feature degrades explicitly'",
                graph.root_ref,
                graph.missing_refs.len(),
                graph.cycle_errors.len()
            );
        }
        graph
    }
}

struct RuntimeAssetGraphResolver {
    host: HostApiV1,
    client: AssetServiceClient,
}

impl RuntimeAssetGraphResolver {
    fn new(host: HostApiV1, client: AssetServiceClient) -> Self {
        Self { host, client }
    }

    fn resolve(&self, root_ref: &str) -> ResolvedAssetGraphV2 {
        let root_ref = normalize_asset_ref(root_ref);
        let mut graph = AssetGraphResolver::resolve_root_ref(&root_ref);
        graph.debug_log.push(format!(
            "assets.graph.resolve_v1: hydration begin root_ref='{root_ref}'"
        ));
        let mut visiting = Vec::<String>::new();
        let mut visited = std::collections::BTreeSet::<String>::new();
        self.resolve_ref(&mut graph, &root_ref, &mut visiting, &mut visited);
        finalize_graph(&mut graph);
        graph
    }

    fn resolve_ref(
        &self,
        graph: &mut ResolvedAssetGraphV2,
        asset_ref: &str,
        visiting: &mut Vec<String>,
        visited: &mut std::collections::BTreeSet<String>,
    ) {
        let asset_ref = normalize_asset_ref(asset_ref);
        if asset_ref.is_empty() {
            return;
        }
        if visiting.iter().any(|item| item == &asset_ref) {
            let mut cycle = visiting.clone();
            cycle.push(asset_ref.clone());
            graph.cycle_errors.push(cycle.join(" -> "));
            return;
        }
        if !visited.insert(asset_ref.clone()) {
            return;
        }
        visiting.push(asset_ref.clone());
        self.attach_source_and_hash(graph, &asset_ref);

        let deps = match extension_of_ref(&asset_ref).as_deref() {
            Some("ytyp") => self.resolve_ytyp_entry(graph, &asset_ref),
            Some("ydd") => self.resolve_ydd_manifest(graph, &asset_ref),
            Some("ytyd") => {
                self.resolve_generic_manifest(graph, &asset_ref, "uv_layout_dependency")
            }
            Some("nemat") => self.resolve_nemat_graph(graph, &asset_ref),
            Some("ytd") => self.validate_ytd_ref(graph, &asset_ref),
            Some("ymap") | Some("ymf") | Some("ymt") | Some("ywr") | Some("ysc") | Some("ybn")
            | Some("ybd") | Some("ycol") | Some("ydr") | Some("yft") | Some("ycd")
            | Some("yed") | Some("yfd") | Some("yld") | Some("ypdb") | Some("yvr")
            | Some("ytf") => {
                self.resolve_generic_manifest(graph, &asset_ref, "listfile_dependency")
            }
            Some("nebrain") | Some("nepat") | Some("nemem") | Some("negoal") | Some("nebt")
            | Some("nebehavior") | Some("neutility") | Some("nebb") => {
                self.resolve_generic_manifest(graph, &asset_ref, "ai_dependency")
            }
            Some(other) => {
                graph.format_warnings.push(format!("assets.graph.resolve_v1: no semantic resolver for ref='{asset_ref}' extension='.{other}'"));
                Vec::new()
            }
            None => Vec::new(),
        };

        for (dep, role, required) in deps {
            push_manifest_dependency(graph, &asset_ref, &dep, &role, required);
            self.resolve_ref(graph, &dep, visiting, visited);
        }
        visiting.pop();
    }

    fn attach_source_and_hash(&self, graph: &mut ResolvedAssetGraphV2, asset_ref: &str) {
        let (path, selector) = split_asset_ref(asset_ref);
        if path.is_empty() {
            return;
        }

        let defer_missing_to_semantic_resolver = selector.is_some()
            && extension_of_ref(asset_ref)
                .as_deref()
                .map(|extension| extension.eq_ignore_ascii_case("ytyp"))
                .unwrap_or(false);

        if defer_missing_to_semantic_resolver {
            attach_vfs_source(
                graph,
                asset_ref,
                AssetGraphVfsSource {
                    source_kind: "semantic_sidecar_pending".to_owned(),
                    logical_path: path.clone(),
                    ..Default::default()
                },
            );
            graph.debug_log.push(format!(
                "assets.graph.resolve_v1: deferred canonical .ytyp VFS lookup ref='{asset_ref}' path='{path}' policy='definitions gateway resolves concrete sidecar source'"
            ));
            return;
        }

        self.attach_source_and_hash_from_path(graph, asset_ref, &path, true);
    }

    fn attach_source_and_hash_from_path(
        &self,
        graph: &mut ResolvedAssetGraphV2,
        asset_ref: &str,
        logical_path: &str,
        record_missing: bool,
    ) {
        let logical_path = logical_path.trim().replace('\\', "/");
        if logical_path.is_empty() {
            return;
        }
        match self.client.raw_bytes_v1(&logical_path) {
            Ok(bytes) => attach_content_hash(
                graph,
                asset_ref,
                format!("fnv1a64:{:016x}", fnv1a64(&bytes)),
            ),
            Err(err) => {
                if record_missing {
                    graph
                        .missing_refs
                        .push(format!("{asset_ref}: VFS bytes unavailable: {err}"));
                    attach_node_warning(graph, asset_ref, format!("VFS bytes unavailable: {err}"));
                } else {
                    graph.debug_log.push(format!(
                        "assets.graph.resolve_v1: non-fatal VFS bytes miss ref='{asset_ref}' path='{logical_path}' err='{err}'"
                    ));
                }
            }
        }
        match self.client.resolve_trace_json_v1(&logical_path) {
            Ok(trace) => attach_vfs_source(
                graph,
                asset_ref,
                vfs_source_from_trace(&logical_path, &trace),
            ),
            Err(err) => {
                if record_missing {
                    attach_node_warning(
                        graph,
                        asset_ref,
                        format!("VFS source trace unavailable: {err}"),
                    );
                }
                attach_vfs_source(
                    graph,
                    asset_ref,
                    AssetGraphVfsSource {
                        source_kind: if record_missing {
                            "unresolved".to_owned()
                        } else {
                            "semantic_sidecar_pending".to_owned()
                        },
                        logical_path,
                        ..Default::default()
                    },
                );
            }
        }
    }

    fn resolve_ytyp_entry(
        &self,
        graph: &mut ResolvedAssetGraphV2,
        asset_ref: &str,
    ) -> Vec<(String, String, bool)> {
        let request = serde_json::json!({ "definition_ref": asset_ref });
        match self.call_gateway_json(
            newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            newengine_assets_api::definitions_method::ENTRY_JSON_V1,
            request,
        ) {
            Ok(value) => {
                collect_metadata_namespaces(graph, asset_ref, &value);
                if let Some(source) = value
                    .pointer("/identity/source")
                    .and_then(|value| value.as_str())
                    .map(|source| source.trim().replace('\\', "/"))
                    .filter(|source| !source.is_empty())
                {
                    let (canonical_path, _) = split_asset_ref(asset_ref);
                    if !source.eq_ignore_ascii_case(&canonical_path) {
                        graph.debug_log.push(format!(
                            "assets.graph.resolve_v1: .ytyp semantic sidecar source ref='{asset_ref}' canonical='{canonical_path}' source='{source}'"
                        ));
                    }
                    self.attach_source_and_hash_from_path(graph, asset_ref, &source, false);
                }
                // Do not scrape arbitrary strings out of `.ytyp` metadata. A
                // Definition Entry may mention a sky mesh, player model or editor
                // asset as descriptive knowledge; that does not make it a spawn
                // command. engine.assets.definitions owns the semantic ref projection and
                // provides explicit buckets. AssetGraph preserves those as graph
                // dependencies only; scene/apply systems decide what to instantiate.
                definition_entry_refs_to_edges(value.get("refs"), asset_ref)
            }
            Err(err) => {
                graph.missing_refs.push(format!(
                    "{asset_ref}: assets.definitions.entry_v1 failed: {err}"
                ));
                attach_node_warning(
                    graph,
                    asset_ref,
                    format!("assets.definitions.entry_v1 failed: {err}"),
                );
                Vec::new()
            }
        }
    }

    fn resolve_ydd_manifest(
        &self,
        graph: &mut ResolvedAssetGraphV2,
        asset_ref: &str,
    ) -> Vec<(String, String, bool)> {
        let (source, selector) = split_asset_ref(asset_ref);
        let request = serde_json::json!({ "source": source, "selector": selector });
        match self.call_gateway_json(
            ENGINE_ASSETS_MODELS_SERVICE_ID,
            MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1,
            request,
        ) {
            Ok(value) => {
                collect_metadata_namespaces(graph, asset_ref, &value);
                let mut deps = collect_ref_strings(&value);
                deps.retain(|dep| dep != asset_ref);
                refs_to_edges(deps, "drawable_dependency")
            }
            Err(err) => {
                graph.missing_refs.push(format!(
                    "{asset_ref}: assets.models.drawable_manifest_v1 failed: {err}"
                ));
                attach_node_warning(
                    graph,
                    asset_ref,
                    format!("assets.models.drawable_manifest_v1 failed: {err}"),
                );
                Vec::new()
            }
        }
    }

    fn resolve_nemat_graph(
        &self,
        graph: &mut ResolvedAssetGraphV2,
        asset_ref: &str,
    ) -> Vec<(String, String, bool)> {
        let request =
            serde_json::json!({ "logical_path": asset_ref, "selector": serde_json::Value::Null });
        match self.call_gateway_json(
            newengine_materials::ENGINE_ASSETS_MATERIALS_SERVICE_ID,
            newengine_materials::method::RESOLVE_GRAPH_V1,
            request,
        ) {
            Ok(value) => {
                collect_metadata_namespaces(graph, asset_ref, &value);
                let mut deps = collect_ref_strings(&value);
                deps.retain(|dep| dep != asset_ref);
                refs_to_edges(deps, "material_texture")
            }
            Err(err) => {
                graph.missing_refs.push(format!(
                    "{asset_ref}: assets.materials.resolve_graph_v1 failed: {err}"
                ));
                attach_node_warning(
                    graph,
                    asset_ref,
                    format!("assets.materials.resolve_graph_v1 failed: {err}"),
                );
                Vec::new()
            }
        }
    }

    fn validate_ytd_ref(
        &self,
        graph: &mut ResolvedAssetGraphV2,
        asset_ref: &str,
    ) -> Vec<(String, String, bool)> {
        let (path, selector) = split_asset_ref(asset_ref);
        if selector.is_none() {
            let request = AssetDecodeRequest {
                logical_path: path.clone(),
                output_kind: newengine_assets_api::method::LIST_FILE_MANIFEST.to_owned(),
                selector: serde_json::Value::Null,
            };
            if let Err(err) = self.client.decode_v1(&request) {
                graph.missing_refs.push(format!(
                    "{asset_ref}: texture dictionary manifest unavailable: {err}"
                ));
                attach_node_warning(
                    graph,
                    asset_ref,
                    format!("texture dictionary manifest unavailable: {err}"),
                );
            }
            return Vec::new();
        }
        let request = serde_json::json!({ "texture_ref": asset_ref });
        if let Err(err) = self.call_gateway_json(
            newengine_assets_api::ENGINE_ASSETS_TEXTURES_SERVICE_ID,
            newengine_assets_api::textures_method::VALIDATE_REF_V1,
            request,
        ) {
            graph.missing_refs.push(format!(
                "{asset_ref}: assets.textures.validate_ref_v1 failed: {err}"
            ));
            attach_node_warning(
                graph,
                asset_ref,
                format!("assets.textures.validate_ref_v1 failed: {err}"),
            );
        }
        Vec::new()
    }

    fn resolve_generic_manifest(
        &self,
        graph: &mut ResolvedAssetGraphV2,
        asset_ref: &str,
        role: &str,
    ) -> Vec<(String, String, bool)> {
        let (path, _) = split_asset_ref(asset_ref);
        let request = AssetDecodeRequest {
            logical_path: path.clone(),
            output_kind: newengine_assets_api::method::LIST_FILE_MANIFEST.to_owned(),
            selector: serde_json::Value::Null,
        };
        match self.client.decode_v1(&request) {
            Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
                Ok(value) => refs_to_edges(collect_ref_strings(&value), role),
                Err(err) => {
                    graph.metadata_warnings.push(format!(
                        "{asset_ref}: generic manifest decode returned non-json: {err}"
                    ));
                    Vec::new()
                }
            },
            Err(err) => {
                attach_node_warning(
                    graph,
                    asset_ref,
                    format!("generic manifest unavailable: {err}"),
                );
                Vec::new()
            }
        }
    }

    fn call_gateway_json(
        &self,
        service_id: &str,
        method_name: &str,
        request: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let payload = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
        let bytes = (self.host.call_service_v1)(
            RString::from(service_id),
            MethodName::from(method_name),
            Blob::from(payload),
        )
        .into_result()
        .map(|value| value.into_vec())
        .map_err(|err| err.to_string())?;
        serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|e| {
            format!("service='{service_id}' method='{method_name}' returned non-json: {e}")
        })
    }
}

fn extension_of_ref(reference: &str) -> Option<String> {
    let (path, _) = split_asset_ref(reference);
    path.rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
}

fn refs_to_edges(mut refs: Vec<String>, default_role: &str) -> Vec<(String, String, bool)> {
    refs.sort();
    refs.dedup();
    refs.into_iter()
        .map(|reference| {
            let role = match extension_of_ref(&reference).as_deref() {
                Some("ydd") => "drawable_dictionary",
                Some("nemat") => "material_library",
                Some("ytd") => "texture_dictionary",
                Some("ybn") | Some("ycol") => "physics_dictionary",
                Some("nebrain") => "ai_brain",
                Some("nepat") => "ai_pattern",
                Some("nemem") => "ai_memory",
                Some("ytyp") => "model_properties_descriptor",
                _ => default_role,
            };
            (reference, role.to_owned(), true)
        })
        .collect()
}

fn definition_entry_refs_to_edges(
    refs_value: Option<&serde_json::Value>,
    owner_ref: &str,
) -> Vec<(String, String, bool)> {
    let Some(refs_value) = refs_value.and_then(|value| value.as_object()) else {
        return Vec::new();
    };
    let mut edges = Vec::new();
    for (field, role) in [
        ("drawable_refs", "definition/drawable_dependency"),
        ("material_refs", "definition/material_dependency"),
        ("texture_refs", "definition/texture_dependency"),
        ("uv_layout_refs", "definition/uv_layout_dependency"),
        ("physics_refs", "definition/physics_dependency"),
        ("collision_refs", "definition/collision_dependency"),
        ("ai_refs", "definition/ai_dependency"),
        ("streaming_refs", "definition/streaming_dependency"),
        ("editor_refs", "definition/editor_dependency"),
        ("other_refs", "definition/other_dependency"),
    ] {
        let Some(items) = refs_value.get(field).and_then(|value| value.as_array()) else {
            continue;
        };
        for item in items {
            let Some(text) = item.as_str() else {
                continue;
            };
            let reference = normalize_asset_ref(text);
            if reference.is_empty() || reference == owner_ref {
                continue;
            }
            edges.push((reference, role.to_owned(), true));
        }
    }
    edges.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    edges.dedup();
    edges
}

fn collect_ref_strings(value: &serde_json::Value) -> Vec<String> {
    let mut refs = Vec::new();
    collect_ref_strings_into(value, &mut refs);
    refs.sort();
    refs.dedup();
    refs
}

fn collect_ref_strings_into(value: &serde_json::Value, refs: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => {
            let normalized = normalize_asset_ref(text);
            if looks_like_runtime_asset_ref(&normalized) {
                refs.push(normalized);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_ref_strings_into(item, refs);
            }
        }
        serde_json::Value::Object(map) => {
            for value in map.values() {
                collect_ref_strings_into(value, refs);
            }
        }
        _ => {}
    }
}

fn looks_like_runtime_asset_ref(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        ".ytyp",
        ".ydd@",
        ".ytyd@",
        ".ydr@",
        ".yft@",
        ".nemat@",
        ".ytd@",
        ".ymap@",
        ".ymf@",
        ".ymt@",
        ".ybn@",
        ".ybd@",
        ".ycol@",
        ".ycd@",
        ".yed@",
        ".yfd@",
        ".yld@",
        ".ypdb@",
        ".yvr@",
        ".ywr@",
        ".ysc@",
        ".ytf@",
        ".nebrain@",
        ".nepat@",
        ".nemem@",
        ".negoal@",
        ".nebt@",
        ".nebehavior@",
        ".neutility@",
        ".nebb@",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn collect_metadata_namespaces(
    graph: &mut ResolvedAssetGraphV2,
    owner_ref: &str,
    value: &serde_json::Value,
) {
    if let Some(namespaces) = value
        .get("metadata_namespaces")
        .or_else(|| value.get("metadata"))
        .and_then(|v| v.as_array())
    {
        for namespace in namespaces {
            if let Some(name) = namespace
                .get("namespace")
                .or_else(|| namespace.get("name"))
                .and_then(|v| v.as_str())
            {
                attach_metadata_namespace(graph, owner_ref, name);
            }
        }
    }
    if let Some(side_effects) = value.get("side_effects").and_then(|v| v.as_object()) {
        for key in side_effects.keys() {
            attach_metadata_namespace(graph, owner_ref, format!("side_effect:{key}"));
        }
    }
}

fn vfs_source_from_trace(path: &str, trace: &serde_json::Value) -> AssetGraphVfsSource {
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

fn asset_graph_gateway_info() -> serde_json::Value {
    serde_json::json!({
        "service_id": newengine_model_domain_api::ASSET_GRAPH_SERVICE_ID,
        "gateway": newengine_model_domain_api::ENGINE_ASSETS_GRAPH_SERVICE_ID,
        "provider": "StarVaultAssetGraphResolverProviderV2",
        "contract": "newengine.assets.graph.runtime.v1",
        "methods": newengine_model_domain_api::ASSET_GRAPH_METHODS,
        "schema": newengine_model_domain_api::ASSET_GRAPH_RESOLVED_SCHEMA_V2,
    })
}

fn asset_graph_invoke(state: &mut AssetGraphGatewayState, payload: Blob) -> RResult<Blob, RString> {
    let value = match payload_json(&payload) {
        Ok(value) => value,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let method = value
        .get("method")
        .and_then(|value| value.as_str())
        .unwrap_or(newengine_model_domain_api::ASSET_GRAPH_METHOD_RESOLVE_V1);
    let request_value = value
        .get("request")
        .cloned()
        .unwrap_or_else(|| value.clone());
    let request = serde_json::from_value::<newengine_model_domain_api::AssetGraphResolveRequest>(
        request_value,
    )
    .unwrap_or_default();
    match method {
        newengine_model_domain_api::ASSET_GRAPH_METHOD_RESOLVE_V1 => {
            ok_json(state.resolve(request.root()))
        }
        newengine_model_domain_api::ASSET_GRAPH_METHOD_VALIDATE_V1 => {
            let graph = state.resolve(request.root());
            ok_json(newengine_model_domain_api::AssetGraphResolver::validate_graph(graph))
        }
        newengine_model_domain_api::ASSET_GRAPH_METHOD_DUMP_JSON_V1 => {
            match serde_json::to_value(state.resolve(request.root())) {
                Ok(value) => ok_json(value),
                Err(e) => RResult::RErr(RString::from(e.to_string())),
            }
        }
        other => RResult::RErr(RString::from(format!(
            "engine.assets.graph: unknown invoke method '{other}'"
        ))),
    }
}

fn asset_graph_service(
    host: HostApiV1,
    client: AssetServiceClient,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = newengine_service_kit::engine_gateway_provider_service_description(
        newengine_model_domain_api::ASSET_GRAPH_SERVICE_ID,
        "newengine-asset-graph-runtime.hydrated-resolver-v2",
        newengine_model_domain_api::ASSET_GRAPH_BACKEND_CAPABILITY_ID,
        newengine_model_domain_api::ASSET_GRAPH_METHODS.iter().copied(),
    )
    .protocol("newengine.assets.graph.runtime.v1")
    .features(["assets-graph-resolver-v2", "hydrated-dependencies", "vfs-source-trace", "stable-cache-key"])
    .gateway("engine.assets.starvault.graph resolver")
    .notes("Hydrates dependency graphs through engine.assets.definitions, engine.assets.models, engine.assets.materials, engine.assets.textures and engine.assets/VFS diagnostics.");

    newengine_service_kit::JsonServiceRouter::with_state(
        newengine_model_domain_api::ASSET_GRAPH_SERVICE_ID,
        AssetGraphGatewayState { host, client },
    )
    .describe_json(&description)
    .get_json(newengine_service_api::SERVICE_METHOD_INFO_JSON, |_state| asset_graph_gateway_info())
    .post_json_result::<newengine_model_domain_api::AssetGraphResolveRequest, newengine_model_domain_api::ResolvedAssetGraphV2, _>(
        newengine_model_domain_api::ASSET_GRAPH_METHOD_RESOLVE_V1,
        |state, request| Ok(state.resolve(request.root())),
    )
    .post_json_result::<newengine_model_domain_api::AssetGraphResolveRequest, newengine_model_domain_api::AssetGraphValidationResult, _>(
        newengine_model_domain_api::ASSET_GRAPH_METHOD_VALIDATE_V1,
        |state, request| {
            let graph = state.resolve(request.root());
            Ok(newengine_model_domain_api::AssetGraphResolver::validate_graph(graph))
        },
    )
    .post_json_result::<newengine_model_domain_api::AssetGraphResolveRequest, serde_json::Value, _>(
        newengine_model_domain_api::ASSET_GRAPH_METHOD_DUMP_JSON_V1,
        |state, request| serde_json::to_value(state.resolve(request.root())).map_err(|e| e.to_string()),
    )
    .blob(newengine_service_api::SERVICE_METHOD_INVOKE_JSON, asset_graph_invoke)
    .blob(newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1, |_state, _payload| newengine_service_kit::ok_empty_blob())
    .into_service_v1()
}

pub fn register_asset_graph_gateway_best_effort(
    host: HostApiV1,
    client: AssetServiceClient,
) -> bool {
    let registered = newengine_service_kit::register_engine_gateway_provider_service_best_effort(
        newengine_service_kit::EngineGatewayProviderDecl {
            gateway: newengine_model_domain_api::ENGINE_ASSETS_GRAPH_SERVICE_ID,
            service_kind: newengine_service_api::EngineServiceKind::AssetGraph,
            provider_service: newengine_model_domain_api::ASSET_GRAPH_SERVICE_ID,
            provider_route: "engine.assets.starvault.graph",
            capability: newengine_model_domain_api::ASSET_GRAPH_BACKEND_CAPABILITY_ID,
            priority: 0,
            owner: "newengine-asset-graph-runtime.hydrated-resolver-v2",
            service: asset_graph_service(host, client),
        },
    );
    log::info!(
        "engine.assets.graph: provider registration registered={} gateway='{}' service='{}' capability='{}'",
        registered,
        newengine_model_domain_api::ENGINE_ASSETS_GRAPH_SERVICE_ID,
        newengine_model_domain_api::ASSET_GRAPH_SERVICE_ID,
        newengine_model_domain_api::ASSET_GRAPH_BACKEND_CAPABILITY_ID,
    );
    registered
}
