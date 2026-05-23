#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_scripting_api::{
    ScriptDiagnostic, ScriptDispatchEventRequest, ScriptFrameInput, ScriptFrameOutput,
    ScriptModuleDescriptor, ScriptModuleLoadRequest, ScriptModuleLoadResponse,
    ScriptModuleManifest, ScriptModuleRef, ScriptModuleRefValidationResponse,
    ScriptModuleState, ScriptingInvokeEnvelope, ScriptingServiceInfo, ScriptingStateDump,
    ENGINE_SCRIPTING_SERVICE_ID, SCRIPTING_BACKEND_CAPABILITY_ID, SCRIPTING_SERVICE_ID,
    SCRIPTING_SERVICE_METHOD_DISPATCH_EVENT_JSON_V1, SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1,
    SCRIPTING_SERVICE_METHOD_FRAME_JSON_V1, SCRIPTING_SERVICE_METHOD_INVOKE,
    SCRIPTING_SERVICE_METHOD_INFO, SCRIPTING_SERVICE_METHOD_LOAD_MODULE_JSON_V1,
    SCRIPTING_SERVICE_METHOD_MODULE_MANIFEST_JSON_V1, SCRIPTING_SERVICE_METHOD_SHUTDOWN_V1,
    SCRIPTING_SERVICE_METHOD_UNLOAD_MODULE_JSON_V1,
    SCRIPTING_SERVICE_METHOD_VALIDATE_MODULE_REF_JSON_V1,
};
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_owned_service_description, ok_empty_blob, ok_json,
    register_engine_owned_gateway_service_best_effort, EngineOwnedGatewayDecl, JsonServiceRouter,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize)]
pub struct ScriptingRuntimeServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub backend: &'static str,
    pub methods: Vec<String>,
    pub loaded_module_count: usize,
}

#[derive(Clone, Debug, Default)]
pub struct ScriptingRuntimeState {
    modules: BTreeMap<String, ScriptModuleDescriptor>,
    frame_count: u64,
    event_count: u64,
}

impl ScriptingRuntimeState {
    #[inline]
    pub fn service_info(&self) -> ScriptingServiceInfo {
        let mut info = ScriptingServiceInfo::default();
        info.provider = SCRIPTING_SERVICE_ID.to_owned();
        info.backend = "engine-owned.null-scripting".to_owned();
        info.features.push("null-provider-empty-output".to_owned());
        info
    }

    #[inline]
    pub fn runtime_info(&self) -> ScriptingRuntimeServiceInfo {
        ScriptingRuntimeServiceInfo {
            id: SCRIPTING_SERVICE_ID,
            gateway: ENGINE_SCRIPTING_SERVICE_ID,
            backend: "engine-owned.null-scripting",
            methods: newengine_scripting_api::SCRIPTING_SERVICE_METHODS.iter().map(|it| (*it).to_owned()).collect(),
            loaded_module_count: self.modules.len(),
        }
    }

    pub fn module_manifest(&self) -> ScriptModuleManifest {
        ScriptModuleManifest {
            modules: self.modules.values().cloned().collect(),
            warnings: vec![
                "No scripting VM is embedded by newengine-scripting-runtime; loaded modules are declarations only.".to_owned(),
            ],
            ..ScriptModuleManifest::default()
        }
    }

    pub fn load_module(&mut self, request: ScriptModuleLoadRequest) -> ScriptModuleLoadResponse {
        let validation = validate_script_module_ref(request.module_ref.clone());
        if !validation.ok {
            return ScriptModuleLoadResponse {
                ok: false,
                module: ScriptModuleDescriptor { module_ref: request.module_ref, state: ScriptModuleState::Failed, diagnostics: validation.diagnostics.clone(), ..ScriptModuleDescriptor::default() },
                diagnostics: validation.diagnostics,
            };
        }

        let module_id = normalized_module_key(&request.module_ref);
        let module = ScriptModuleDescriptor {
            module_ref: request.module_ref,
            state: ScriptModuleState::Declared,
            permissions: request.permissions,
            metadata: request.metadata,
            diagnostics: vec![ScriptDiagnostic::info(
                "SCRIPTING_MODULE_DECLARED_ONLY",
                "Module accepted as a declaration; no VM/runtime execution is active yet.",
            )],
            ..ScriptModuleDescriptor::default()
        };
        self.modules.insert(module_id, module.clone());
        ScriptModuleLoadResponse { ok: true, module, diagnostics: Vec::new() }
    }

    pub fn unload_module(&mut self, module_ref: ScriptModuleRef) -> ScriptModuleLoadResponse {
        let module_id = normalized_module_key(&module_ref);
        match self.modules.remove(&module_id) {
            Some(mut module) => {
                module.state = ScriptModuleState::Disabled;
                ScriptModuleLoadResponse { ok: true, module, diagnostics: Vec::new() }
            }
            None => ScriptModuleLoadResponse {
                ok: false,
                module: ScriptModuleDescriptor { module_ref, state: ScriptModuleState::Failed, ..ScriptModuleDescriptor::default() },
                diagnostics: vec![ScriptDiagnostic::warning(
                    "SCRIPTING_MODULE_NOT_LOADED",
                    "Requested script module is not loaded in the declaration cache.",
                )],
            },
        }
    }

    pub fn frame(&mut self, input: ScriptFrameInput) -> ScriptFrameOutput {
        self.frame_count = self.frame_count.saturating_add(1);
        let mut output = ScriptFrameOutput::empty_for(&input);
        if input.modules.is_empty() && self.modules.is_empty() {
            output.diagnostics.push(ScriptDiagnostic::info(
                "SCRIPTING_NO_MODULES",
                "No script modules were submitted and no modules are loaded; returning empty output.",
            ));
        } else {
            output.diagnostics.push(ScriptDiagnostic::info(
                "SCRIPTING_VM_NOT_ATTACHED",
                "Scripting gateway is active, but no language provider is attached; returning empty output.",
            ));
        }
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

    pub fn dump_state(&self) -> ScriptingStateDump {
        ScriptingStateDump {
            backend: "engine-owned.null-scripting".to_owned(),
            loaded_modules: self.modules.values().cloned().collect(),
            notes: vec![
                format!("frames_processed={}", self.frame_count),
                format!("events_processed={}", self.event_count),
                "This crate prepares engine.scripting without embedding a script VM.".to_owned(),
            ],
            ..ScriptingStateDump::default()
        }
    }

    fn invoke_json(&mut self, payload: Blob) -> RResult<Blob, RString> {
        let envelope = match serde_json::from_slice::<ScriptingInvokeEnvelope>(payload.as_slice()) {
            Ok(envelope) => envelope,
            Err(e) => return RResult::RErr(RString::from(format!("scripting.api: invalid invoke_json payload: {e}"))),
        };

        match envelope.method.as_str() {
            SCRIPTING_SERVICE_METHOD_FRAME_JSON_V1 => {
                let request = match serde_json::from_value::<ScriptFrameInput>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("scripting.api: invalid frame request: {e}"))),
                };
                ok_json(self.frame(request))
            }
            SCRIPTING_SERVICE_METHOD_LOAD_MODULE_JSON_V1 => {
                let request = match serde_json::from_value::<ScriptModuleLoadRequest>(envelope.request) {
                    Ok(request) => request,
                    Err(e) => return RResult::RErr(RString::from(format!("scripting.api: invalid module load request: {e}"))),
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
            other => RResult::RErr(RString::from(format!("scripting.api: unknown invoke method '{other}'"))),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ScriptModuleUnloadRequest {
    #[serde(default)]
    module_ref: ScriptModuleRef,
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

pub fn scripting_gateway_service() -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_owned_service_description(
        SCRIPTING_SERVICE_ID,
        "newengine-scripting-runtime.null-provider",
        SCRIPTING_BACKEND_CAPABILITY_ID,
        newengine_scripting_api::SCRIPTING_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_SCRIPTING_SERVICE_ID)
    .protocol("json")
    .features([
        "engine-scripting-gateway-v1",
        "null-scripting-provider",
        "script-frame-dto-v1",
        "script-module-declaration-cache-v1",
        "script-command-output-contract-v1",
    ])
    .notes("Baseline scripting provider intentionally does not embed Lua/Visual/WASM. It accepts declarations and returns empty frame output until a real provider overrides engine.scripting.");

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
        .blob(SCRIPTING_SERVICE_METHOD_INVOKE, |state, payload| state.invoke_json(payload))
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
    use newengine_scripting_api::ScriptModuleRef;

    #[test]
    fn null_runtime_returns_empty_frame_output() {
        let mut state = ScriptingRuntimeState::default();
        let out = state.frame(ScriptFrameInput::new(7, 1.0 / 60.0, 1.0, 123));
        assert_eq!(out.frame_index, 7);
        assert!(out.commands.is_empty());
        assert!(out.events.is_empty());
    }

    #[test]
    fn validation_requires_ysc_entry_ref() {
        let bad = validate_script_module_ref(ScriptModuleRef::new("scripts/foo.lua"));
        assert!(!bad.ok);
        let good = validate_script_module_ref(ScriptModuleRef::new("scripts/foo.ysc@main"));
        assert!(good.ok);
    }

    #[test]
    fn declaration_loads_without_vm_execution() {
        let mut state = ScriptingRuntimeState::default();
        let response = state.load_module(ScriptModuleLoadRequest {
            module_ref: ScriptModuleRef::new("scripts/foo.ysc@main"),
            ..ScriptModuleLoadRequest::default()
        });
        assert!(response.ok);
        assert_eq!(state.module_manifest().modules.len(), 1);
    }
}
