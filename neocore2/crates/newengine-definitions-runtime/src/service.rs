use super::*;

#[derive(Clone)]
pub struct DefinitionsRuntimeState {
    pub(super) client: AssetServiceClient,
}

impl DefinitionsRuntimeState {
    #[inline]
    pub fn new(client: AssetServiceClient) -> Self {
        Self { client }
    }
}

pub fn definitions_service_info() -> DefinitionsServiceInfo {
    DefinitionsServiceInfo {
        id: DEFINITIONS_SERVICE_ID,
        gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        provider: "StarVaultDefinitionsRuntimeProvider",
        contract: DEFINITIONS_RUNTIME_CONTRACT,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        methods: DEFINITIONS_SERVICE_METHODS,
        ownership_policy: ".ytyp Definition Entry metadata is owned by engine.assets.definitions; scene/model may consume refs but never decode or own .ytyp; AssetManager only exposes NEF8 envelope/body bytes",
    }
}

fn manifest_blob(state: &mut DefinitionsRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    if payload.is_empty() {
        return ok_json(serde_json::json!({
            "schema": "newengine.assets.definitions.service_manifest.v1",
            "gateway": ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            "provider": "StarVaultDefinitionsRuntimeProvider",
            "byte_owner": ENGINE_ASSET_SERVICE_ID,
            "semantic_owner": ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            "methods": DEFINITIONS_SERVICE_METHODS,
            "entry_schema": "newengine.assets.definitions.entry.v1",
            "ownership_policy": ".ytyp is metadata owned by engine.assets.definitions; not scene and not model"
        }));
    }
    let request = match manifest_request_from_payload(
        payload.as_slice(),
        definitions_method::MANIFEST_JSON_V1,
    ) {
        Ok(request) => request,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let source = match manifest_source_from_request(&request) {
        Ok(source) => source,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    match load_manifest(state, &source) {
        Ok(value) => ok_json(value),
        Err(e) => RResult::RErr(RString::from(e)),
    }
}

fn entry_blob(state: &mut DefinitionsRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    let request =
        match ref_request_from_payload(payload.as_slice(), definitions_method::ENTRY_JSON_V1) {
            Ok(request) => request,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
    match load_entry(state, request) {
        Ok(value) => ok_json(value),
        Err(e) => RResult::RErr(RString::from(e)),
    }
}

fn invoke_json(state: &mut DefinitionsRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    let value = match payload_json(&payload) {
        Ok(value) => value,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let method = value
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or(definitions_method::VALIDATE_V1);
    match method {
        definitions_method::VALIDATE_V1 => {
            let request = serde_json::from_value::<DefinitionRefRequest>(
                value.get("request").cloned().unwrap_or_default(),
            )
            .unwrap_or_default();
            ok_json(validate_entry(state, request))
        }
        definitions_method::ENTRY_JSON_V1 => {
            let request = serde_json::from_value::<DefinitionRefRequest>(
                value.get("request").cloned().unwrap_or_default(),
            )
            .unwrap_or_default();
            match load_entry(state, request) {
                Ok(entry) => ok_json(entry),
                Err(e) => ok_json(StableDiagnostic {
                    ok: false,
                    code: "definitions.entry_unavailable",
                    message: e,
                    gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                    byte_owner: ENGINE_ASSET_SERVICE_ID,
                    semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                }),
            }
        }
        definitions_method::RESOLVE_REFS_V1 => {
            let request = serde_json::from_value::<DefinitionRefRequest>(
                value.get("request").cloned().unwrap_or_default(),
            )
            .unwrap_or_default();
            match resolve_definition_refs(state, request) {
                Ok(resolution) => ok_json(resolution),
                Err(e) => ok_json(StableDiagnostic {
                    ok: false,
                    code: "definitions.refs_unavailable",
                    message: e,
                    gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                    byte_owner: ENGINE_ASSET_SERVICE_ID,
                    semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                }),
            }
        }
        definitions_method::DESCRIBE_SIDE_EFFECTS_V1 => {
            let request = serde_json::from_value::<DefinitionRefRequest>(
                value.get("request").cloned().unwrap_or_default(),
            )
            .unwrap_or_default();
            match describe_definition_side_effects(state, request) {
                Ok(description) => ok_json(description),
                Err(e) => ok_json(StableDiagnostic {
                    ok: false,
                    code: "definitions.side_effects_unavailable",
                    message: e,
                    gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                    byte_owner: ENGINE_ASSET_SERVICE_ID,
                    semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                }),
            }
        }
        definitions_method::MANIFEST_JSON_V1 => {
            let request = serde_json::from_value::<DefinitionManifestRequest>(
                value.get("request").cloned().unwrap_or_default(),
            )
            .unwrap_or_default();
            match manifest_source_from_request(&request)
                .and_then(|source| load_manifest(state, &source))
            {
                Ok(manifest) => ok_json(manifest),
                Err(e) => ok_json(StableDiagnostic {
                    ok: false,
                    code: "definitions.manifest_unavailable",
                    message: e,
                    gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                    byte_owner: ENGINE_ASSET_SERVICE_ID,
                    semantic_owner: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                }),
            }
        }
        other => RResult::RErr(RString::from(format!(
            "engine.assets.definitions: unknown invoke method '{other}'"
        ))),
    }
}

pub fn definitions_gateway_service(
    client: AssetServiceClient,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        DEFINITIONS_SERVICE_ID,
        DEFINITIONS_GATEWAY_OWNER,
        DEFINITIONS_BACKEND_CAPABILITY_ID,
        DEFINITIONS_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSETS_DEFINITIONS_SERVICE_ID)
    .protocol(DEFINITIONS_RUNTIME_CONTRACT)
    .features(["definition-entry-v1", "metadata-namespace-preservation", "declarative-side-effects", "strict-ytyp-ownership"])
    .notes("Engine definitions runtime service. .ytyp semantics live in engine.assets.definitions; engine.assets exposes only VFS bytes and the generic NEF8 ListFile envelope.");

    JsonServiceRouter::with_state(DEFINITIONS_SERVICE_ID, DefinitionsRuntimeState::new(client))
        .describe_json(&description)
        .info(definitions_service_info)
        .blob(definitions_method::MANIFEST_JSON_V1, manifest_blob)
        .blob(definitions_method::ENTRY_JSON_V1, entry_blob)
        .post_json_result::<DefinitionRefRequest, DefinitionValidationV1, _>(
            definitions_method::VALIDATE_V1,
            |state, request| Ok(validate_entry(state, request)),
        )
        .post_json_result::<DefinitionRefRequest, DefinitionRefResolutionV1, _>(
            definitions_method::RESOLVE_REFS_V1,
            resolve_definition_refs,
        )
        .post_json_result::<DefinitionRefRequest, serde_json::Value, _>(
            definitions_method::DESCRIBE_SIDE_EFFECTS_V1,
            describe_definition_side_effects,
        )
        .blob(definitions_method::INVOKE_JSON, invoke_json)
        .blob(definitions_method::SHUTDOWN_V1, |_state, _payload| {
            ok_empty_blob()
        })
        .into_service_v1()
}

pub fn register_definitions_gateway_best_effort(client: AssetServiceClient) -> bool {
    register_engine_gateway_provider_service_best_effort(EngineGatewayProviderDecl {
        gateway: ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        service_kind: EngineServiceKind::Definitions,
        provider_service: DEFINITIONS_SERVICE_ID,
        provider_route: "engine.assets.starvault.definitions",
        capability: DEFINITIONS_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: DEFINITIONS_GATEWAY_OWNER,
        service: definitions_gateway_service(client),
    })
}
