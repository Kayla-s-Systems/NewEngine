use super::*;

use super::refs::{
    collect_metadata_namespaces, collect_ref_strings, definition_entry_refs_to_edges,
    extension_of_ref, list_file_manifest_dependency_edges, refs_to_edges,
};
use super::vfs::vfs_source_from_trace;

pub(super) struct RuntimeAssetGraphResolver {
    host: HostApiV1,
    client: AssetServiceClient,
}

impl RuntimeAssetGraphResolver {
    pub(super) fn new(host: HostApiV1, client: AssetServiceClient) -> Self {
        Self { host, client }
    }

    pub(super) fn resolve(&self, root_ref: &str) -> ResolvedAssetGraphV2 {
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
                Ok(value) => list_file_manifest_dependency_edges(&value, role),
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
