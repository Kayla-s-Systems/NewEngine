use super::*;

pub fn assets_ui_service_info() -> AssetsUiServiceInfo {
    AssetsUiServiceInfo {
        id: ASSETS_UI_SERVICE_ID,
        gateway: ENGINE_ASSETS_UI_SERVICE_ID,
        provider: "StarVaultAssetsUiRuntimeProvider",
        contract: ASSETS_UI_RUNTIME_CONTRACT,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_ASSETS_UI_SERVICE_ID,
        runtime_owner: newengine_ui_api::ENGINE_UI_SERVICE_ID,
        methods: ASSETS_UI_SERVICE_METHODS,
        policy: ".neui is a binary NEF8/ListFile envelope with no raw JSON metadata payload; engine.assets.ui owns semantic decode and consumers receive compiled DTOs",
    }
}

pub fn assets_ui_gateway_service(
    client: AssetServiceClient,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        ASSETS_UI_SERVICE_ID,
        ASSETS_UI_GATEWAY_OWNER,
        ASSETS_UI_BACKEND_CAPABILITY_ID,
        ASSETS_UI_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSETS_UI_SERVICE_ID)
    .protocol(ASSETS_UI_RUNTIME_CONTRACT)
    .features([
        "neui-nef8-binary-envelope",
        "neui-no-json-runtime-metadata",
        "compile-document-v1",
        "ui-node-navigation-dto",
        "dependency-extraction",
        "dialect-asset-inspection",
        "explicit-cache-invalidation",
    ])
    .notes("Engine UI asset semantic service. Consumers call engine.assets.ui and receive runtime DTOs; engine.ui owns only live mount/state/input/draw runtime.");

    JsonServiceRouter::with_state(ASSETS_UI_SERVICE_ID, AssetsUiRuntimeState::new(client))
        .describe_json(&description)
        .info(assets_ui_service_info)
        .post_json_result::<AssetsUiCompileRequest, AssetsUiCompileResponse, _>(
            assets_ui_method::COMPILE_DOCUMENT_V1,
            handlers::compile_document,
        )
        .post_json_result::<AssetsUiRefRequest, serde_json::Value, _>(
            assets_ui_method::DOCUMENT_V1,
            |state, request| {
                handlers::document(state, request, "newengine.assets.ui.document.response.v1")
            },
        )
        .post_json_result::<AssetsUiRefRequest, serde_json::Value, _>(
            assets_ui_method::DUMP_XMLCENTRAL_V1,
            |state, request| {
                handlers::document(state, request, "newengine.assets.ui.xmlcentral_dump.v1")
            },
        )
        .post_json_result::<AssetsUiRefRequest, AssetsUiDiagnosticResponse, _>(
            assets_ui_method::VALIDATE_V1,
            handlers::validate,
        )
        .post_json_result::<AssetsUiRefRequest, serde_json::Value, _>(
            assets_ui_method::DEPENDENCIES_V1,
            handlers::dependencies,
        )
        .post_json_result::<AssetsUiRefRequest, AssetsUiCompileResponse, _>(
            assets_ui_method::MANIFEST_V1,
            handlers::compile_from_ref,
        )
        .post_json_result::<AssetsUiRefRequest, AssetsUiCompileResponse, _>(
            assets_ui_method::ENTRY_V1,
            handlers::compile_from_ref,
        )
        .post_json_result::<AssetsUiRefRequest, AssetsUiCompileResponse, _>(
            assets_ui_method::REGISTRY_V1,
            handlers::compile_from_ref,
        )
        .post_json_result::<AssetsUiRefRequest, AssetsUiCompileResponse, _>(
            assets_ui_method::BINDING_PLAN_V1,
            handlers::compile_from_ref,
        )
        .post_json::<AssetsUiDialectInspectRequest, serde_json::Value, _>(
            assets_ui_method::INSPECT_DIALECT_V1,
            compile_request::inspect_dialect,
        )
        .post_json::<AssetsUiInvalidateRequest, serde_json::Value, _>(
            assets_ui_method::INVALIDATE_V1,
            compile_request::invalidate_caches,
        )
        .blob(assets_ui_method::INVOKE_JSON, invoke::invoke_json)
        .blob(assets_ui_method::SHUTDOWN_V1, |_state, _payload| {
            ok_empty_blob()
        })
        .into_service_v1()
}

pub fn register_assets_ui_gateway_best_effort(client: AssetServiceClient) -> bool {
    register_engine_gateway_provider_service_best_effort(EngineGatewayProviderDecl {
        gateway: ENGINE_ASSETS_UI_SERVICE_ID,
        service_kind: EngineServiceKind::AssetUi,
        provider_service: ASSETS_UI_SERVICE_ID,
        provider_route: "engine.assets.starvault.ui",
        capability: ASSETS_UI_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: ASSETS_UI_GATEWAY_OWNER,
        service: assets_ui_gateway_service(client),
    })
}
