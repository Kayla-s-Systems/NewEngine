use super::*;

use super::resolver::RuntimeAssetGraphResolver;

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
