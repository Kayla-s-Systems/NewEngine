use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_scripting_api::{
    decode_scripting_module_load_bytes_request, decode_scripting_request_bytes,
    encode_scripting_module_load_bytes_response, encode_scripting_response_bytes, ScriptModuleRef,
    ScriptModuleRefValidationResponse, ScriptingInvokeEnvelope, ScriptingModuleLoadBytesResponse,
    ScriptingModuleUnloadRequest, ENGINE_SCRIPTING_SERVICE_ID, SCRIPTING_BACKEND_CAPABILITY_ID,
    SCRIPTING_SERVICE_ID, SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1,
    SCRIPTING_SERVICE_METHOD_FRAME_BYTES_V1, SCRIPTING_SERVICE_METHOD_INFO,
    SCRIPTING_SERVICE_METHOD_INVOKE, SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1,
    SCRIPTING_SERVICE_METHOD_LOAD_MODULE_BYTES_V1, SCRIPTING_SERVICE_METHOD_SHUTDOWN_V1,
    SCRIPTING_SERVICE_METHOD_UNLOAD_MODULE_JSON_V1,
    SCRIPTING_SERVICE_METHOD_VALIDATE_MODULE_REF_JSON_V1,
};
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json,
    register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl,
    JsonServiceRouter,
};

use crate::codec::{decode_json, handle_binary};
use crate::constants::{OWNER, PROVIDER_ROUTE};
use crate::state::ScriptingRuntimeState;
use crate::validation::validate_script_module_ref;

fn invoke_control_json(state: &mut ScriptingRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    let envelope = match decode_json::<ScriptingInvokeEnvelope>(
        &payload,
        "scripting.api: invalid invoke envelope",
    ) {
        Ok(envelope) => envelope,
        Err(error) => return RResult::RErr(error),
    };

    match envelope.method.as_str() {
        SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1 => handle_binary(
            &envelope.request_bytes,
            decode_scripting_request_bytes,
            |request| state.invoke_bytes(request),
            encode_scripting_response_bytes,
            "scripting.api: invalid binary scripting request envelope",
        ),
        SCRIPTING_SERVICE_METHOD_FRAME_BYTES_V1 => handle_binary(
            &envelope.request_bytes,
            decode_scripting_request_bytes,
            |request| state.frame_bytes(request),
            encode_scripting_response_bytes,
            "scripting.api: invalid binary scripting request envelope",
        ),
        SCRIPTING_SERVICE_METHOD_LOAD_MODULE_BYTES_V1 => handle_binary(
            &envelope.request_bytes,
            decode_scripting_module_load_bytes_request,
            |request| state.load_module_bytes(request),
            encode_scripting_module_load_bytes_response,
            "scripting.api: invalid binary module load envelope",
        ),
        SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1 => ok_json(state.dump_state()),
        other => RResult::RErr(RString::from(format!(
            "scripting.api: unknown control method '{other}'"
        ))),
    }
}

pub fn scripting_gateway_service() -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        SCRIPTING_SERVICE_ID,
        OWNER,
        SCRIPTING_BACKEND_CAPABILITY_ID,
        newengine_scripting_api::SCRIPTING_SERVICE_METHODS
            .iter()
            .copied(),
    )
    .gateway(ENGINE_SCRIPTING_SERVICE_ID)
    .protocol("binary-opaque")
    .features([
        "engine-scripting-gateway-v1",
        "binary-request-response-v1",
        "ysc-module-bytes-v1",
        "provider-owned-interpretation",
        "no-language-whitelist",
        "null-scripting-provider",
    ])
    .notes("Baseline scripting provider intentionally does not name, embed or whitelist any scripting implementation. It accepts binary .ysc module envelopes and returns an empty binary response until a real provider overrides engine.scripting.");

    JsonServiceRouter::with_state(SCRIPTING_SERVICE_ID, ScriptingRuntimeState::default())
        .describe_json(&description)
        .get_json(SCRIPTING_SERVICE_METHOD_INFO, |state| state.service_info())
        .post_json::<ScriptingModuleUnloadRequest, ScriptingModuleLoadBytesResponse, _>(
            SCRIPTING_SERVICE_METHOD_UNLOAD_MODULE_JSON_V1,
            |state, request| state.unload_module(request.module_ref),
        )
        .post_json::<ScriptModuleRef, ScriptModuleRefValidationResponse, _>(
            SCRIPTING_SERVICE_METHOD_VALIDATE_MODULE_REF_JSON_V1,
            |_state, request| validate_script_module_ref(request),
        )
        .get_json(SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1, |state| {
            state.dump_state()
        })
        .blob(
            SCRIPTING_SERVICE_METHOD_LOAD_MODULE_BYTES_V1,
            |state, payload| {
                handle_binary(
                    payload.as_slice(),
                    decode_scripting_module_load_bytes_request,
                    |request| state.load_module_bytes(request),
                    encode_scripting_module_load_bytes_response,
                    "scripting.api: invalid binary module load payload",
                )
            },
        )
        .blob(
            SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1,
            |state, payload| {
                handle_binary(
                    payload.as_slice(),
                    decode_scripting_request_bytes,
                    |request| state.invoke_bytes(request),
                    encode_scripting_response_bytes,
                    "scripting.api: invalid binary scripting request payload",
                )
            },
        )
        .blob(SCRIPTING_SERVICE_METHOD_FRAME_BYTES_V1, |state, payload| {
            handle_binary(
                payload.as_slice(),
                decode_scripting_request_bytes,
                |request| state.frame_bytes(request),
                encode_scripting_response_bytes,
                "scripting.api: invalid binary scripting request payload",
            )
        })
        .blob(SCRIPTING_SERVICE_METHOD_INVOKE, invoke_control_json)
        .blob(SCRIPTING_SERVICE_METHOD_SHUTDOWN_V1, |_state, _payload| {
            ok_empty_blob()
        })
        .into_service_v1()
}

pub fn register_scripting_gateway_best_effort() -> bool {
    register_engine_gateway_provider_service_best_effort(EngineGatewayProviderDecl {
        gateway: ENGINE_SCRIPTING_SERVICE_ID,
        service_kind: EngineServiceKind::Scripting,
        provider_service: SCRIPTING_SERVICE_ID,
        provider_route: PROVIDER_ROUTE,
        capability: SCRIPTING_BACKEND_CAPABILITY_ID,
        priority: -100,
        owner: OWNER,
        service: scripting_gateway_service(),
    })
}
