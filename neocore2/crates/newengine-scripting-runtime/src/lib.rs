#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_scripting_api::{
    decode_scripting_module_load_bytes_request, decode_scripting_request_bytes,
    encode_scripting_module_load_bytes_response, encode_scripting_response_bytes, ScriptDiagnostic,
    ScriptModuleRef, ScriptModuleRefValidationResponse, ScriptModuleState, ScriptingInvokeEnvelope,
    ScriptingModuleLoadBytesRequest, ScriptingModuleLoadBytesResponse, ScriptingModuleRecord,
    ScriptingModuleRef, ScriptingModuleUnloadRequest, ScriptingRequestBytes, ScriptingResponseBytes,
    ScriptingResponseStatus, ScriptingServiceInfo, ScriptingStateDump, ENGINE_SCRIPTING_SERVICE_ID,
    SCRIPTING_BACKEND_CAPABILITY_ID, SCRIPTING_SERVICE_ID, SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1,
    SCRIPTING_SERVICE_METHOD_FRAME_BYTES_V1, SCRIPTING_SERVICE_METHOD_INFO,
    SCRIPTING_SERVICE_METHOD_INVOKE, SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1,
    SCRIPTING_SERVICE_METHOD_LOAD_MODULE_BYTES_V1, SCRIPTING_SERVICE_METHOD_SHUTDOWN_V1,
    SCRIPTING_SERVICE_METHOD_UNLOAD_MODULE_JSON_V1, SCRIPTING_SERVICE_METHOD_VALIDATE_MODULE_REF_JSON_V1,
};
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json,
    register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl, JsonServiceRouter,
};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct ScriptingRuntimeServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub backend_capability: &'static str,
    pub provider_label: &'static str,
    pub methods: Vec<String>,
    pub loaded_module_count: usize,
}

#[derive(Clone, Debug, Default)]
pub struct ScriptingRuntimeState {
    modules: BTreeMap<String, ScriptingModuleRecord>,
    request_count: u64,
    frame_count: u64,
}

impl ScriptingRuntimeState {
    #[inline]
    pub fn service_info(&self) -> ScriptingServiceInfo {
        let mut info = ScriptingServiceInfo::default();
        info.provider = SCRIPTING_SERVICE_ID.to_owned();
        info.backend = "engine.scripting.nullstar".to_owned();
        info.features.push("null-provider-empty-response".to_owned());
        info
    }

    #[inline]
    pub fn runtime_info(&self) -> ScriptingRuntimeServiceInfo {
        ScriptingRuntimeServiceInfo {
            id: SCRIPTING_SERVICE_ID,
            gateway: ENGINE_SCRIPTING_SERVICE_ID,
            backend_capability: SCRIPTING_BACKEND_CAPABILITY_ID,
            provider_label: "engine.scripting.nullstar",
            methods: newengine_scripting_api::SCRIPTING_SERVICE_METHODS.iter().map(|it| (*it).to_owned()).collect(),
            loaded_module_count: self.modules.len(),
        }
    }

    pub fn load_module_bytes(&mut self, request: ScriptingModuleLoadBytesRequest) -> ScriptingModuleLoadBytesResponse {
        let validation = validate_script_module_ref(request.module_ref.clone());
        if !validation.ok {
            return ScriptingModuleLoadBytesResponse {
                ok: false,
                module: ScriptingModuleRecord {
                    module_ref: request.module_ref,
                    state: ScriptModuleState::Failed,
                    diagnostics: validation.diagnostics.clone(),
                    ..ScriptingModuleRecord::default()
                },
                diagnostics: validation.diagnostics,
            };
        }

        let module_id = normalized_module_key(&request.module_ref);
        let module = ScriptingModuleRecord {
            module_ref: request.module_ref,
            state: ScriptModuleState::Declared,
            permissions: request.permissions,
            module_bytes_len: request.module_bytes.len() as u64,
            metadata: request.metadata,
            diagnostics: vec![ScriptDiagnostic::info(
                "SCRIPTING_MODULE_OPAQUE_DECLARATION_ONLY",
                "Module bytes accepted as opaque provider-owned payload; no script implementation is embedded in the baseline runtime.",
            )],
            ..ScriptingModuleRecord::default()
        };
        self.modules.insert(module_id, module.clone());
        ScriptingModuleLoadBytesResponse { ok: true, module, diagnostics: Vec::new() }
    }

    pub fn unload_module(&mut self, module_ref: ScriptModuleRef) -> ScriptingModuleLoadBytesResponse {
        let module_id = normalized_module_key(&module_ref);
        match self.modules.remove(&module_id) {
            Some(mut module) => {
                module.state = ScriptModuleState::Disabled;
                ScriptingModuleLoadBytesResponse { ok: true, module, diagnostics: Vec::new() }
            }
            None => ScriptingModuleLoadBytesResponse {
                ok: false,
                module: ScriptingModuleRecord { module_ref, state: ScriptModuleState::Failed, ..ScriptingModuleRecord::default() },
                diagnostics: vec![ScriptDiagnostic::warning(
                    "SCRIPTING_MODULE_NOT_LOADED",
                    "Requested script module is not loaded in the opaque declaration cache.",
                )],
            },
        }
    }

    pub fn invoke_bytes(&mut self, request: ScriptingRequestBytes) -> ScriptingResponseBytes {
        self.request_count = self.request_count.saturating_add(1);
        let mut response = ScriptingResponseBytes::empty_for(&request);
        response.status = ScriptingResponseStatus::Empty;
        response.diagnostics.push(ScriptDiagnostic::info(
            "SCRIPTING_PROVIDER_EMPTY_RESPONSE",
            "engine.scripting is routed to the baseline provider; request bytes were accepted but no script implementation is attached.",
        ));
        response
    }

    pub fn frame_bytes(&mut self, request: ScriptingRequestBytes) -> ScriptingResponseBytes {
        self.frame_count = self.frame_count.saturating_add(1);
        self.invoke_bytes(request)
    }

    pub fn dump_state(&self) -> ScriptingStateDump {
        let mut counters = BTreeMap::new();
        counters.insert("requests_processed".to_owned(), self.request_count);
        counters.insert("frames_processed".to_owned(), self.frame_count);
        ScriptingStateDump {
            backend: "engine.scripting.nullstar".to_owned(),
            loaded_modules: self.modules.values().cloned().collect(),
            counters,
            notes: vec![
                "This crate exposes an opaque engine.scripting boundary without embedding or naming any script implementation.".to_owned(),
                "Runtime load/invoke/frame methods use binary envelopes; JSON is reserved for control/debug methods.".to_owned(),
            ],
            ..ScriptingStateDump::default()
        }
    }

    fn invoke_control_json(&mut self, payload: Blob) -> RResult<Blob, RString> {
        let envelope = match serde_json::from_slice::<ScriptingInvokeEnvelope>(payload.as_slice()) {
            Ok(envelope) => envelope,
            Err(e) => return RResult::RErr(RString::from(format!("scripting.api: invalid invoke envelope: {e}"))),
        };

        match envelope.method.as_str() {
            SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1 => self.invoke_request_payload(envelope.request_bytes, false),
            SCRIPTING_SERVICE_METHOD_FRAME_BYTES_V1 => self.invoke_request_payload(envelope.request_bytes, true),
            SCRIPTING_SERVICE_METHOD_LOAD_MODULE_BYTES_V1 => self.load_module_payload(envelope.request_bytes),
            SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1 => ok_json(self.dump_state()),
            other => RResult::RErr(RString::from(format!("scripting.api: unknown control method '{other}'"))),
        }
    }

    fn load_module_payload(&mut self, payload: Vec<u8>) -> RResult<Blob, RString> {
        let request = match decode_scripting_module_load_bytes_request(&payload) {
            Ok(request) => request,
            Err(e) => return RResult::RErr(RString::from(format!("scripting.api: invalid binary module load envelope: {e}"))),
        };
        RResult::ROk(Blob::from(encode_scripting_module_load_bytes_response(&self.load_module_bytes(request))))
    }

    fn invoke_request_payload(&mut self, payload: Vec<u8>, frame: bool) -> RResult<Blob, RString> {
        let request = match decode_scripting_request_bytes(&payload) {
            Ok(request) => request,
            Err(e) => return RResult::RErr(RString::from(format!("scripting.api: invalid binary scripting request envelope: {e}"))),
        };
        let response = if frame { self.frame_bytes(request) } else { self.invoke_bytes(request) };
        RResult::ROk(Blob::from(encode_scripting_response_bytes(&response)))
    }
}

#[inline]
pub fn validate_script_module_ref(module_ref: ScriptModuleRef) -> ScriptModuleRefValidationResponse {
    let mut diagnostics = Vec::new();
    let reference = module_ref.reference.trim();
    if reference.is_empty() {
        diagnostics.push(ScriptDiagnostic::error(
            "SCRIPTING_EMPTY_MODULE_REF",
            "Script module reference must not be empty.",
        ));
    }
    if !module_ref.is_ysc_entry_ref() {
        diagnostics.push(ScriptDiagnostic::error(
            "SCRIPTING_MODULE_REF_NOT_YSC_ENTRY",
            "Runtime script modules must be addressed as file.ysc@entry.",
        ));
    }
    if reference.contains("..") || reference.contains('\\') || reference.starts_with('/') {
        diagnostics.push(ScriptDiagnostic::error(
            "SCRIPTING_UNSAFE_MODULE_REF",
            "Script module references must be normalized VFS logical paths.",
        ));
    }
    ScriptModuleRefValidationResponse { ok: diagnostics.is_empty(), module_ref, diagnostics }
}

#[inline]
fn normalized_module_key(module_ref: &ScriptingModuleRef) -> String {
    if !module_ref.module_id.trim().is_empty() {
        return module_ref.module_id.trim().to_ascii_lowercase();
    }
    module_ref.reference.trim().replace('\\', "/").trim_start_matches('/').to_ascii_lowercase()
}

fn decode_module_load_blob(payload: Blob) -> RResult<ScriptingModuleLoadBytesRequest, RString> {
    match decode_scripting_module_load_bytes_request(payload.as_slice()) {
        Ok(request) => RResult::ROk(request),
        Err(e) => RResult::RErr(RString::from(format!("scripting.api: invalid binary module load payload: {e}"))),
    }
}

fn decode_request_blob(payload: Blob) -> RResult<ScriptingRequestBytes, RString> {
    match decode_scripting_request_bytes(payload.as_slice()) {
        Ok(request) => RResult::ROk(request),
        Err(e) => RResult::RErr(RString::from(format!("scripting.api: invalid binary scripting request payload: {e}"))),
    }
}

pub fn scripting_gateway_service() -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        SCRIPTING_SERVICE_ID,
        "newengine-scripting-runtime.null-provider",
        SCRIPTING_BACKEND_CAPABILITY_ID,
        newengine_scripting_api::SCRIPTING_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_SCRIPTING_SERVICE_ID)
    .protocol("binary-opaque")
    .features([
        "engine-scripting-gateway-v1",
        "binary-request-response-v1",
        "ysc-entry-bytes-v1",
        "provider-owned-interpretation",
        "no-language-whitelist",
        "null-scripting-provider",
    ])
    .notes("Baseline scripting provider intentionally does not name, embed or whitelist any scripting implementation. It accepts binary .ysc@entry/module envelopes and returns an empty binary response until a real provider overrides engine.scripting.");

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
        .get_json(SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1, |state| state.dump_state())
        .blob(SCRIPTING_SERVICE_METHOD_LOAD_MODULE_BYTES_V1, |state, payload| match decode_module_load_blob(payload) {
            RResult::ROk(request) => RResult::ROk(Blob::from(encode_scripting_module_load_bytes_response(&state.load_module_bytes(request)))),
            RResult::RErr(err) => RResult::RErr(err),
        })
        .blob(SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1, |state, payload| match decode_request_blob(payload) {
            RResult::ROk(request) => RResult::ROk(Blob::from(encode_scripting_response_bytes(&state.invoke_bytes(request)))),
            RResult::RErr(err) => RResult::RErr(err),
        })
        .blob(SCRIPTING_SERVICE_METHOD_FRAME_BYTES_V1, |state, payload| match decode_request_blob(payload) {
            RResult::ROk(request) => RResult::ROk(Blob::from(encode_scripting_response_bytes(&state.frame_bytes(request)))),
            RResult::RErr(err) => RResult::RErr(err),
        })
        .blob(SCRIPTING_SERVICE_METHOD_INVOKE, |state, payload| state.invoke_control_json(payload))
        .blob(SCRIPTING_SERVICE_METHOD_SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
        .into_service_v1()
}

pub fn register_scripting_gateway_best_effort() -> bool {
    register_engine_gateway_provider_service_best_effort(EngineGatewayProviderDecl {
        gateway: ENGINE_SCRIPTING_SERVICE_ID,
        service_kind: EngineServiceKind::Scripting,
        provider_service: SCRIPTING_SERVICE_ID,
        provider_route: "engine.scripting.nullstar",
        capability: SCRIPTING_BACKEND_CAPABILITY_ID,
        priority: -100,
        owner: "newengine-scripting-runtime.null-provider",
        service: scripting_gateway_service(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_scripting_api::{
        encode_scripting_module_load_bytes_request, encode_scripting_request_bytes,
    };

    #[test]
    fn baseline_runtime_returns_empty_opaque_response() {
        let mut state = ScriptingRuntimeState::default();
        let out = state.invoke_bytes(ScriptingRequestBytes { request_id: "r1".to_owned(), ..ScriptingRequestBytes::default() });
        assert_eq!(out.request_id, "r1");
        assert_eq!(out.status, ScriptingResponseStatus::Empty);
        assert!(out.payload_bytes.is_empty());
    }

    #[test]
    fn validation_requires_ysc_entry_ref() {
        let bad = validate_script_module_ref(ScriptModuleRef::new("scripts/foo.source"));
        assert!(!bad.ok);
        let good = validate_script_module_ref(ScriptModuleRef::new("scripts/foo.ysc@main"));
        assert!(good.ok);
    }

    #[test]
    fn binary_load_stores_opaque_byte_count() {
        let mut state = ScriptingRuntimeState::default();
        let response = state.load_module_bytes(ScriptingModuleLoadBytesRequest {
            module_ref: ScriptModuleRef::new("scripts/foo.ysc@main"),
            module_bytes: vec![1, 2, 3, 4],
            ..ScriptingModuleLoadBytesRequest::default()
        });
        assert!(response.ok);
        assert_eq!(response.module.module_bytes_len, 4);
        assert_eq!(state.modules.len(), 1);
    }

    #[test]
    fn binary_wire_methods_accept_binary_envelopes() {
        let mut state = ScriptingRuntimeState::default();
        let load = ScriptingModuleLoadBytesRequest {
            module_ref: ScriptModuleRef::new("scripts/foo.ysc@main"),
            module_bytes: vec![1, 2, 3],
            ..ScriptingModuleLoadBytesRequest::default()
        };
        assert!(matches!(decode_module_load_blob(Blob::from(encode_scripting_module_load_bytes_request(&load))), RResult::ROk(_)));

        let request = ScriptingRequestBytes { request_id: "r1".to_owned(), ..ScriptingRequestBytes::default() };
        assert!(matches!(decode_request_blob(Blob::from(encode_scripting_request_bytes(&request))), RResult::ROk(_)));
    }
}
