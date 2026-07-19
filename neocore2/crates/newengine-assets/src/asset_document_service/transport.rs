use super::*;

fn inspect_service_info() -> AssetInspectServiceInfo {
    AssetInspectServiceInfo {
        id: ASSETS_INSPECT_SERVICE_ID,
        gateway: ENGINE_ASSETS_INSPECT_SERVICE_ID,
        methods: ASSETS_INSPECT_SERVICE_METHODS,
        backend: "engine.assets.starvault.asset-document-inspect",
        policy: "schema-driven DTO; Asset Browser/UI does not parse file formats",
    }
}

fn edit_service_info() -> AssetEditServiceInfo {
    AssetEditServiceInfo {
        id: ASSETS_EDIT_SERVICE_ID,
        gateway: ENGINE_ASSETS_EDIT_SERVICE_ID,
        methods: ASSETS_EDIT_SERVICE_METHODS,
        backend: "engine.assets.starvault.asset-document-edit",
        policy: "validates patch DTOs; write-back requires explicit provider edit_contract/package writer capability",
    }
}

#[derive(Deserialize)]
struct InvokeEnvelope {
    method: String,
    #[serde(default)]
    request: Value,
}

fn inspect_invoke_json(state: &mut AssetInspectState, payload: Blob) -> RResult<Blob, RString> {
    let envelope = match serde_json::from_slice::<InvokeEnvelope>(payload.as_slice()) {
        Ok(envelope) => envelope,
        Err(e) => {
            return RResult::RErr(RString::from(format!(
                "assets.inspect: invalid invoke_json payload: {e}"
            )))
        }
    };
    match envelope.method.as_str() {
        asset_inspect_method::INSPECT_DOCUMENT_JSON_V1
        | asset_inspect_method::PREVIEW_JSON_V1
        | asset_inspect_method::VALIDATE_REF_JSON_V1 => {
            let request = match serde_json::from_value::<AssetDocumentRequest>(envelope.request) {
                Ok(request) => request,
                Err(e) => {
                    return RResult::RErr(RString::from(format!(
                        "assets.inspect: invalid document request: {e}"
                    )))
                }
            };
            ok_json(state.inspect_document(request))
        }
        other => RResult::RErr(RString::from(format!(
            "assets.inspect: unknown invoke method '{other}'"
        ))),
    }
}

fn edit_invoke_json(state: &mut AssetEditState, payload: Blob) -> RResult<Blob, RString> {
    let envelope = match serde_json::from_slice::<InvokeEnvelope>(payload.as_slice()) {
        Ok(envelope) => envelope,
        Err(e) => {
            return RResult::RErr(RString::from(format!(
                "assets.edit: invalid invoke_json payload: {e}"
            )))
        }
    };
    match envelope.method.as_str() {
        asset_edit_method::VALIDATE_PATCH_JSON_V1 => {
            let patch = match serde_json::from_value::<AssetPatch>(envelope.request) {
                Ok(patch) => patch,
                Err(e) => {
                    return RResult::RErr(RString::from(format!("assets.edit: invalid patch: {e}")))
                }
            };
            ok_json(state.validate_patch(patch))
        }
        asset_edit_method::APPLY_PATCH_JSON_V1 => {
            let patch = match serde_json::from_value::<AssetPatch>(envelope.request) {
                Ok(patch) => patch,
                Err(e) => {
                    return RResult::RErr(RString::from(format!("assets.edit: invalid patch: {e}")))
                }
            };
            ok_json(state.apply_patch(patch))
        }
        asset_edit_method::STAGE_PATCH_JSON_V1 => {
            let patch = match serde_json::from_value::<AssetPatch>(envelope.request) {
                Ok(patch) => patch,
                Err(e) => {
                    return RResult::RErr(RString::from(format!(
                        "assets.edit: invalid staged patch: {e}"
                    )))
                }
            };
            ok_json(state.stage_patch(patch))
        }
        asset_edit_method::REBUILD_JSON_V1 => ok_json(state.rebuild_staged(envelope.request)),
        asset_edit_method::DISCARD_STAGED_JSON_V1 => {
            ok_json(state.discard_staged(envelope.request))
        }
        asset_edit_method::DIRTY_STATE_JSON_V1 => ok_json(state.dirty_state(envelope.request)),
        other => RResult::RErr(RString::from(format!(
            "assets.edit: unknown invoke method '{other}'"
        ))),
    }
}

pub fn asset_document_inspect_gateway_service(
    host: HostApiV1,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        ASSETS_INSPECT_SERVICE_ID,
        "newengine-assets.document-inspect",
        ASSETS_INSPECT_BACKEND_CAPABILITY_ID,
        ASSETS_INSPECT_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSETS_INSPECT_SERVICE_ID)
    .protocol("json")
    .features(["schema-driven-asset-document", "provider-routed-inspection", "ui-agnostic"])
    .notes("Asset Browser requests AssetDocument DTOs here. Format parsing belongs to provider/domain contracts, not to UI.");

    JsonServiceRouter::with_state(ASSETS_INSPECT_SERVICE_ID, AssetInspectState::new(host))
        .describe_json(&description)
        .info(inspect_service_info)
        .post_json::<AssetDocumentRequest, AssetDocument, _>(
            asset_inspect_method::INSPECT_DOCUMENT_JSON_V1,
            |state, request| state.inspect_document(request),
        )
        .post_json::<AssetDocumentRequest, AssetDocument, _>(
            asset_inspect_method::PREVIEW_JSON_V1,
            |state, request| state.inspect_document(request),
        )
        .post_json::<AssetDocumentRequest, AssetDocument, _>(
            asset_inspect_method::VALIDATE_REF_JSON_V1,
            |state, request| state.inspect_document(request),
        )
        .blob(asset_inspect_method::INVOKE_JSON, inspect_invoke_json)
        .blob(asset_inspect_method::SHUTDOWN_V1, |_state, _payload| {
            ok_empty_blob()
        })
        .into_service_v1()
}

pub fn asset_document_edit_gateway_service(
    host: HostApiV1,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        ASSETS_EDIT_SERVICE_ID,
        "newengine-assets.document-edit",
        ASSETS_EDIT_BACKEND_CAPABILITY_ID,
        ASSETS_EDIT_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSETS_EDIT_SERVICE_ID)
    .protocol("json")
    .features([
        "asset-patch-dto",
        "staged-edit-session",
        "provider-validated-writeback",
        "explicit-rebuild-commit",
        "explicit-writer-capability",
    ])
    .notes("Generic edit route validates patch transport. Real write-back is owned by format/package writer providers.");

    JsonServiceRouter::with_state(ASSETS_EDIT_SERVICE_ID, AssetEditState::new(host))
        .describe_json(&description)
        .info(edit_service_info)
        .post_json::<AssetPatch, AssetPatchResult, _>(
            asset_edit_method::VALIDATE_PATCH_JSON_V1,
            |state, patch| state.validate_patch(patch),
        )
        .post_json::<AssetPatch, AssetPatchResult, _>(
            asset_edit_method::APPLY_PATCH_JSON_V1,
            |state, patch| state.apply_patch(patch),
        )
        .post_json::<AssetPatch, AssetPatchResult, _>(
            asset_edit_method::STAGE_PATCH_JSON_V1,
            |state, patch| state.stage_patch(patch),
        )
        .post_json::<Value, AssetPatchResult, _>(
            asset_edit_method::REBUILD_JSON_V1,
            |state, payload| state.rebuild_staged(payload),
        )
        .post_json::<Value, AssetPatchResult, _>(
            asset_edit_method::DISCARD_STAGED_JSON_V1,
            |state, payload| state.discard_staged(payload),
        )
        .post_json::<Value, AssetPatchResult, _>(
            asset_edit_method::DIRTY_STATE_JSON_V1,
            |state, payload| state.dirty_state(payload),
        )
        .blob(asset_edit_method::INVOKE_JSON, edit_invoke_json)
        .blob(asset_edit_method::SHUTDOWN_V1, |_state, _payload| {
            ok_empty_blob()
        })
        .into_service_v1()
}

pub fn register_asset_document_gateways_best_effort(host: HostApiV1) -> bool {
    let inspect_ok = register_engine_gateway_provider_service_dynamic_best_effort(
        EngineGatewayProviderDeclDynamic {
            gateway: ENGINE_ASSETS_INSPECT_SERVICE_ID,
            service_kind: "assets.inspect",
            provider_service: ASSETS_INSPECT_SERVICE_ID,
            provider_route: "engine.assets.starvault.inspect",
            capability: ASSETS_INSPECT_BACKEND_CAPABILITY_ID,
            priority: 0,
            owner: "newengine-assets.document-inspect",
            service: asset_document_inspect_gateway_service(host.clone()),
        },
    );
    let edit_ok = register_engine_gateway_provider_service_dynamic_best_effort(
        EngineGatewayProviderDeclDynamic {
            gateway: ENGINE_ASSETS_EDIT_SERVICE_ID,
            service_kind: "assets.edit",
            provider_service: ASSETS_EDIT_SERVICE_ID,
            provider_route: "engine.assets.starvault.edit",
            capability: ASSETS_EDIT_BACKEND_CAPABILITY_ID,
            priority: 0,
            owner: "newengine-assets.document-edit",
            service: asset_document_edit_gateway_service(host),
        },
    );
    inspect_ok && edit_ok
}
