#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_scripting_api::{
    ScriptDiagnostic, ScriptDispatchEventRequest, ScriptFrameInput, ScriptFrameOutput,
    ScriptModuleDescriptor, ScriptModuleLoadRequest, ScriptModuleLoadResponse,
    ScriptModuleManifest, ScriptModuleRef, ScriptModuleRefValidationResponse,
    ScriptModuleState, ScriptModuleUnloadRequest, ScriptingInvokeEnvelope,
    ScriptingModuleLoadBytesRequest, ScriptingModuleLoadBytesResponse, ScriptingModuleRecord,
    ScriptingRequestBytes, ScriptingResponseBytes, ScriptingResponseStatus, ScriptingServiceInfo,
    ScriptingStateDump, ENGINE_SCRIPTING_SERVICE_ID, SCRIPTING_BACKEND_CAPABILITY_ID,
    SCRIPTING_SERVICE_ID, SCRIPTING_SERVICE_METHOD_DISPATCH_EVENT_JSON_V1,
    SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1, SCRIPTING_SERVICE_METHOD_FRAME_BYTES_V1,
    SCRIPTING_SERVICE_METHOD_FRAME_JSON_V1, SCRIPTING_SERVICE_METHOD_INFO,
    SCRIPTING_SERVICE_METHOD_INVOKE, SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1,
    SCRIPTING_SERVICE_METHOD_LOAD_MODULE_BYTES_V1, SCRIPTING_SERVICE_METHOD_LOAD_MODULE_JSON_V1,
    SCRIPTING_SERVICE_METHOD_MODULE_MANIFEST_JSON_V1, SCRIPTING_SERVICE_METHOD_SHUTDOWN_V1,
    SCRIPTING_SERVICE_METHOD_UNLOAD_MODULE_JSON_V1,
    SCRIPTING_SERVICE_METHOD_VALIDATE_MODULE_REF_JSON_V1,
};
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_owned_service_description, ok_empty_blob, ok_json,
    register_engine_owned_gateway_service_best_effort, EngineOwnedGatewayDecl, JsonServiceRouter,
};
use serde::Serialize;
use serde_json::Value;

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
    event_count: u64,
}

impl ScriptingRuntimeState {
    #[inline]
    pub fn service_info(&self) -> ScriptingServiceInfo {
        let mut info = ScriptingServiceInfo::default();
        info.provider = SCRIPTING_SERVICE_ID.to_owned();
        info.backend = "engine-owned.null-scripting".to_owned();
        info.features.push("null-provider-empty-response".to_owned());
        info.features.push("legacy-json-compat-adapters".to_owned());
        info
    }

    #[inline]
    pub fn runtime_info(&self) -> ScriptingRuntimeServiceInfo {
        ScriptingRuntimeServiceInfo {
            id: SCRIPTING_SERVICE_ID,
            gateway: ENGINE_SCRIPTING_SERVICE_ID,
            backend_capability: SCRIPTING_BACKEND_CAPABILITY_ID,
            provider_label: "engine-owned.null-scripting",
            methods: newengine_scripting_api::SCRIPTING_SERVICE_METHODS.iter().map(|it| (*it).to_owned()).collect(),
            loaded_module_count: self.modules.len(),
        }
    }

    pub fn module_manifest(&self) -> ScriptModuleManifest {
        ScriptModuleManifest {
            modules: self.modules.values().map(record_to_legacy_descriptor).collect(),
            warnings: vec![
                "No scripting implementation is embedded by newengine-scripting-runtime; loaded modules are opaque declarations only.".to_owned(),
                "JSON frame/module methods are compatibility adapters over the opaque bytes boundary.".to_owned(),
            ],
            ..ScriptModuleManifest::default()
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

    pub fn load_module(&mut self, request: ScriptModuleLoadRequest) -> ScriptModuleLoadResponse {
        let response = self.load_module_bytes(ScriptingModuleLoadBytesRequest {
            module_ref: request.module_ref,
            module_bytes: Vec::new(),
            permissions: request.permissions,
            metadata: value_metadata_to_strings(request.metadata),
        });
        ScriptModuleLoadResponse {
            ok: response.ok,
            module: record_to_legacy_descriptor(&response.module),
            diagnostics: response.diagnostics,
        }
    }

    pub fn unload_module(&mut self, module_ref: ScriptModuleRef) -> ScriptModuleLoadResponse {
        let module_id = normalized_module_key(&module_ref);
        match self.modules.remove(&module_id) {
            Some(mut module) => {
                module.state = ScriptModuleState::Disabled;
                ScriptModuleLoadResponse { ok: true, module: record_to_legacy_descriptor(&module), diagnostics: Vec::new() }
            }
            None => ScriptModuleLoadResponse {
                ok: false,
                module: ScriptModuleDescriptor { module_ref, state: ScriptModuleState::Failed, ..ScriptModuleDescriptor::default() },
                diagnostics: vec![ScriptDiagnostic::warning(
                    "SCRIPTING_MODULE_NOT_LOADED",
                    "Requested script module is not loaded in the opaque declaration cache.",
                )],
            },
        }
    }

    pub fn frame(&mut self, input: ScriptFrameInput) -> ScriptFrameOutput {
        self.frame_count = self.frame_count.saturating_add(1);
        let mut output = ScriptFrameOutput::empty_for(&input);
        output.diagnostics.push(if input.modules.is_empty() && self.modules.is_empty() {
            ScriptDiagnostic::info(
                "SCRIPTING_NO_MODULES",
                "No script modules were submitted and no modules are loaded; returning empty compatibility output.",
            )
        } else {
            ScriptDiagnostic::info(
                "SCRIPTING_PROVIDER_EMPTY_RESPONSE",
                "engine.scripting is routed to the baseline provider; compatibility JSON frame accepted but no script implementation is attached.",
            )
        });
        output
    }

    pub fn dispatch_event(&mut self, request: ScriptDispatchEventRequest) -> ScriptFrameOutput {
        self.event_count = self.event_count.saturating_add(1);
        let input = ScriptFrameInput {
            events: vec![request.event],
            modules: request.modules,
            ..ScriptFrameInput::default()
        };
        self.frame(input)
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
        counters.insert("events_processed".to_owned(), self.event_count);
        ScriptingStateDump {
            backend: "engine-owned.null-scripting".to_owned(),
            loaded_modules: self.modules.values().cloned().collect(),
            counters,
            notes: vec![
                "This crate exposes an opaque engine.scripting boundary without embedding or naming any script implementation.".to_owned(),
                "Legacy JSON methods are compatibility adapters and do not add language/VM knowledge to core.".to_owned(),
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
            SCRIPTING_SERVICE_METHOD_FRAME_JSON_V1 => {
                let request = match serde_json::from_value::<ScriptFrameInput>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("scripting.api: invalid compatibility frame request: {e}"))),
                };
                ok_json(self.frame(request))
            }
            SCRIPTING_SERVICE_METHOD_LOAD_MODULE_JSON_V1 => {
                let request = match serde_json::from_value::<ScriptModuleLoadRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("scripting.api: invalid compatibility module load request: {e}"))),
                };
                ok_json(self.load_module(request))
            }
            SCRIPTING_SERVICE_METHOD_UNLOAD_MODULE_JSON_V1 => {
                let request = match serde_json::from_value::<ScriptModuleUnloadRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("scripting.api: invalid module unload request: {e}"))),
                };
                ok_json(self.unload_module(request.module_ref))
            }
            SCRIPTING_SERVICE_METHOD_MODULE_MANIFEST_JSON_V1 => ok_json(self.module_manifest()),
            SCRIPTING_SERVICE_METHOD_VALIDATE_MODULE_REF_JSON_V1 => {
                let request = match serde_json::from_value::<ScriptModuleRef>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("scripting.api: invalid module ref request: {e}"))),
                };
                ok_json(validate_script_module_ref(request))
            }
            SCRIPTING_SERVICE_METHOD_DISPATCH_EVENT_JSON_V1 => {
                let request = match serde_json::from_value::<ScriptDispatchEventRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("scripting.api: invalid event dispatch request: {e}"))),
                };
                ok_json(self.dispatch_event(request))
            }
            SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1 => ok_json(self.dump_state()),
            other => RResult::RErr(RString::from(format!("scripting.api: unknown control method '{other}'"))),
        }
    }

    fn load_module_payload(&mut self, payload: Vec<u8>) -> RResult<Blob, RString> {
        let request = match serde_json::from_slice::<ScriptingModuleLoadBytesRequest>(&payload) {
            Ok(request) => request,
            Err(e) => return RResult::RErr(RString::from(format!("scripting.api: invalid module load bytes envelope: {e}"))),
        };
        ok_json(self.load_module_bytes(request))
    }

    fn invoke_request_payload(&mut self, payload: Vec<u8>, frame: bool) -> RResult<Blob, RString> {
        let request = match serde_json::from_slice::<ScriptingRequestBytes>(&payload) {
            Ok(request) => request,
            Err(e) => return RResult::RErr(RString::from(format!("scripting.api: invalid scripting request bytes envelope: {e}"))),
        };
        let response = if frame { self.frame_bytes(request) } else { self.invoke_bytes(request) };
        ok_json(response)
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
fn normalized_module_key(module_ref: &ScriptModuleRef) -> String {
    if !module_ref.module_id.trim().is_empty() {
        return module_ref.module_id.trim().to_ascii_lowercase();
    }
    module_ref.reference.trim().replace('\\', "/").trim_start_matches('/').to_ascii_lowercase()
}

fn value_metadata_to_strings(metadata: BTreeMap<String, Value>) -> BTreeMap<String, String> {
    metadata.into_iter().map(|(key, value)| {
        let text = match value {
            Value::String(text) => text,
            other => serde_json::to_string(&other).unwrap_or_else(|_| String::new()),
        };
        (key, text)
    }).collect()
}

fn string_metadata_to_values(metadata: &BTreeMap<String, String>) -> BTreeMap<String, Value> {
    metadata.iter().map(|(key, value)| (key.clone(), Value::String(value.clone()))).collect()
}

fn record_to_legacy_descriptor(record: &ScriptingModuleRecord) -> ScriptModuleDescriptor {
    ScriptModuleDescriptor {
        module_ref: record.module_ref.clone(),
        state: record.state,
        permissions: record.permissions.clone(),
        metadata: string_metadata_to_values(&record.metadata),
        diagnostics: record.diagnostics.clone(),
        ..ScriptModuleDescriptor::default()
    }
}

fn decode_module_load_blob(payload: Blob) -> RResult<ScriptingModuleLoadBytesRequest, RString> {
    match serde_json::from_slice::<ScriptingModuleLoadBytesRequest>(payload.as_slice()) {
        Ok(request) => RResult::ROk(request),
        Err(e) => RResult::RErr(RString::from(format!("scripting.api: invalid module load bytes payload: {e}"))),
    }
}

fn decode_request_blob(payload: Blob) -> RResult<ScriptingRequestBytes, RString> {
    match serde_json::from_slice::<ScriptingRequestBytes>(payload.as_slice()) {
        Ok(request) => RResult::ROk(request),
        Err(e) => RResult::RErr(RString::from(format!("scripting.api: invalid scripting request bytes payload: {e}"))),
    }
}

pub fn scripting_gateway_service() -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_owned_service_description(
        SCRIPTING_SERVICE_ID,
        "newengine-scripting-runtime.null-provider",
        SCRIPTING_BACKEND_CAPABILITY_ID,
        newengine_scripting_api::SCRIPTING_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_SCRIPTING_SERVICE_ID)
    .protocol("opaque-bytes+json-compat")
    .features([
        "engine-scripting-gateway-v1",
        "opaque-request-response-v1",
        "ysc-entry-bytes-v1",
        "provider-owned-interpretation",
        "no-language-whitelist",
        "legacy-json-compat-adapters",
        "null-scripting-provider",
    ])
    .notes("Baseline scripting provider intentionally does not name, embed or whitelist any scripting implementation. It accepts opaque .ysc@entry/module request bytes, keeps deprecated JSON methods as adapters, and returns an empty response until a real provider overrides engine.scripting.");

    JsonServiceRouter::with_state(SCRIPTING_SERVICE_ID, ScriptingRuntimeState::default())
        .describe_json(&description)
        .get_json(SCRIPTING_SERVICE_METHOD_INFO, |state| state.service_info())
        .get_json(SCRIPTING_SERVICE_METHOD_MODULE_MANIFEST_JSON_V1, |state| state.module_manifest())
        .post_json::<ScriptFrameInput, ScriptFrameOutput, _>(
            SCRIPTING_SERVICE_METHOD_FRAME_JSON_V1,
            |state, request| state.frame(request),
        )
        .post_json::<ScriptModuleLoadRequest, ScriptModuleLoadResponse, _>(
            SCRIPTING_SERVICE_METHOD_LOAD_MODULE_JSON_V1,
            |state, request| state.load_module(request),
        )
        .post_json::<ScriptModuleUnloadRequest, ScriptModuleLoadResponse, _>(
            SCRIPTING_SERVICE_METHOD_UNLOAD_MODULE_JSON_V1,
            |state, request| state.unload_module(request.module_ref),
        )
        .post_json::<ScriptModuleRef, ScriptModuleRefValidationResponse, _>(
            SCRIPTING_SERVICE_METHOD_VALIDATE_MODULE_REF_JSON_V1,
            |_state, request| validate_script_module_ref(request),
        )
        .post_json::<ScriptDispatchEventRequest, ScriptFrameOutput, _>(
            SCRIPTING_SERVICE_METHOD_DISPATCH_EVENT_JSON_V1,
            |state, request| state.dispatch_event(request),
        )
        .get_json(SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1, |state| state.dump_state())
        .blob(SCRIPTING_SERVICE_METHOD_LOAD_MODULE_BYTES_V1, |state, payload| match decode_module_load_blob(payload) {
            RResult::ROk(request) => ok_json(state.load_module_bytes(request)),
            RResult::RErr(err) => RResult::RErr(err),
        })
        .blob(SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1, |state, payload| match decode_request_blob(payload) {
            RResult::ROk(request) => ok_json(state.invoke_bytes(request)),
            RResult::RErr(err) => RResult::RErr(err),
        })
        .blob(SCRIPTING_SERVICE_METHOD_FRAME_BYTES_V1, |state, payload| match decode_request_blob(payload) {
            RResult::ROk(request) => ok_json(state.frame_bytes(request)),
            RResult::RErr(err) => RResult::RErr(err),
        })
        .blob(SCRIPTING_SERVICE_METHOD_INVOKE, |state, payload| state.invoke_control_json(payload))
        .blob(SCRIPTING_SERVICE_METHOD_SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
        .into_service_v1()
}

pub fn register_scripting_gateway_best_effort() -> bool {
    register_engine_owned_gateway_service_best_effort(EngineOwnedGatewayDecl {
        gateway: ENGINE_SCRIPTING_SERVICE_ID,
        service_kind: EngineServiceKind::Scripting,
        provider_service: SCRIPTING_SERVICE_ID,
        capability: SCRIPTING_BACKEND_CAPABILITY_ID,
        priority: -100,
        owner: "newengine-scripting-runtime.null-provider",
        service: scripting_gateway_service(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn legacy_json_declaration_loads_without_vm_execution() {
        let mut state = ScriptingRuntimeState::default();
        let response = state.load_module(ScriptModuleLoadRequest {
            module_ref: ScriptModuleRef::new("scripts/foo.ysc@main"),
            ..ScriptModuleLoadRequest::default()
        });
        assert!(response.ok);
        assert_eq!(state.module_manifest().modules.len(), 1);
    }

    #[test]
    fn bytes_load_stores_opaque_byte_count() {
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
}
