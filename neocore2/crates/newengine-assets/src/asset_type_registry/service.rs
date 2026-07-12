use super::*;

pub fn asset_types_service_info() -> AssetTypesServiceInfo {
    AssetTypeRegistryState::default().service_info()
}

pub fn register_asset_type_descriptor_best_effort(
    host: &HostApiV1,
    descriptor: AssetFileTypeDescriptor,
) -> bool {
    let payload = match serde_json::to_vec(&AssetFileTypeRegisterRequest { descriptor }) {
        Ok(payload) => payload,
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "asset type registry: failed to serialize descriptor registration: {e}"
            );
            return false;
        }
    };
    let result = (host.call_service_v1)(
        RString::from(ENGINE_ASSET_TYPES_SERVICE_ID),
        MethodName::from(file_type_method::REGISTER_JSON_V1),
        Blob::from(payload),
    );
    match result.into_result() {
        Ok(_) => true,
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "asset type registry: descriptor self-registration failed: {e}"
            );
            false
        }
    }
}

pub fn asset_types_gateway_service() -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        ASSET_TYPES_SERVICE_ID,
        "newengine-assets.file-type-registry",
        ASSET_TYPES_BACKEND_CAPABILITY_ID,
        ASSET_TYPES_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSET_TYPES_SERVICE_ID)
    .protocol("json")
    .features(["codec-descriptor-registry", "self-registration", "provider-owned-format-descriptors"])
    .notes("Descriptor registry starts empty. Format crates/codecs/providers self-register descriptors; the registry only stores, validates and resolves them.");

    JsonServiceRouter::with_state(ASSET_TYPES_SERVICE_ID, AssetTypeRegistryState::default())
        .describe_json(&description)
        .info(asset_types_service_info)
        .get_json(file_type_method::MANIFEST_JSON_V1, |state| state.manifest())
        .post_json::<AssetFileTypeRegisterRequest, AssetFileTypeDescriptor, _>(
            file_type_method::REGISTER_JSON_V1,
            |state, request| state.register(request),
        )
        .post_json::<AssetFileTypeProbeRequest, AssetFileTypeProbeResult, _>(
            file_type_method::PROBE_JSON_V1,
            |state, request| state.probe(request),
        )
        .post_json::<AssetFileTypeProbeRequest, AssetFileTypeProbeResult, _>(
            file_type_method::RESOLVE_JSON_V1,
            |state, request| state.probe(request),
        )
        .blob(file_type_method::INVOKE_JSON, |state, payload| {
            state.invoke_json(payload)
        })
        .blob(file_type_method::SHUTDOWN_V1, |_state, _payload| {
            ok_empty_blob()
        })
        .into_service_v1()
}

pub fn register_asset_types_gateway_best_effort() -> bool {
    register_engine_gateway_provider_service_best_effort(EngineGatewayProviderDecl {
        gateway: ENGINE_ASSET_TYPES_SERVICE_ID,
        service_kind: EngineServiceKind::AssetTypes,
        provider_service: ASSET_TYPES_SERVICE_ID,
        provider_route: "engine.assets.starvault.types",
        capability: ASSET_TYPES_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: "newengine-assets.file-type-registry",
        service: asset_types_gateway_service(),
    })
}
