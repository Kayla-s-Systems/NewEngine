use std::collections::BTreeMap;

use newengine_scripting_api::{
    ScriptDiagnostic, ScriptModuleRef, ScriptModuleState, ScriptingModuleLoadBytesRequest,
    ScriptingModuleLoadBytesResponse, ScriptingModuleRecord, ScriptingRequestBytes,
    ScriptingResponseBytes, ScriptingResponseStatus, ScriptingServiceInfo, ScriptingStateDump,
    ENGINE_SCRIPTING_SERVICE_ID, SCRIPTING_BACKEND_CAPABILITY_ID, SCRIPTING_SERVICE_ID,
    SCRIPTING_SERVICE_METHODS,
};
use serde::Serialize;

use crate::constants::PROVIDER_LABEL;
use crate::validation::{normalized_module_key, validate_script_module_ref};

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
        let mut info = ScriptingServiceInfo {
            provider: SCRIPTING_SERVICE_ID.to_owned(),
            backend: PROVIDER_LABEL.to_owned(),
            ..ScriptingServiceInfo::default()
        };
        info.features
            .push("null-provider-empty-response".to_owned());
        info
    }

    #[inline]
    pub fn runtime_info(&self) -> ScriptingRuntimeServiceInfo {
        ScriptingRuntimeServiceInfo {
            id: SCRIPTING_SERVICE_ID,
            gateway: ENGINE_SCRIPTING_SERVICE_ID,
            backend_capability: SCRIPTING_BACKEND_CAPABILITY_ID,
            provider_label: PROVIDER_LABEL,
            methods: SCRIPTING_SERVICE_METHODS
                .iter()
                .map(|method| (*method).to_owned())
                .collect(),
            loaded_module_count: self.modules.len(),
        }
    }

    pub fn load_module_bytes(
        &mut self,
        request: ScriptingModuleLoadBytesRequest,
    ) -> ScriptingModuleLoadBytesResponse {
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
        ScriptingModuleLoadBytesResponse {
            ok: true,
            module,
            diagnostics: Vec::new(),
        }
    }

    pub fn unload_module(
        &mut self,
        module_ref: ScriptModuleRef,
    ) -> ScriptingModuleLoadBytesResponse {
        let module_id = normalized_module_key(&module_ref);
        match self.modules.remove(&module_id) {
            Some(mut module) => {
                module.state = ScriptModuleState::Disabled;
                ScriptingModuleLoadBytesResponse {
                    ok: true,
                    module,
                    diagnostics: Vec::new(),
                }
            }
            None => ScriptingModuleLoadBytesResponse {
                ok: false,
                module: ScriptingModuleRecord {
                    module_ref,
                    state: ScriptModuleState::Failed,
                    ..ScriptingModuleRecord::default()
                },
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
        let counters = BTreeMap::from([
            ("requests_processed".to_owned(), self.request_count),
            ("frames_processed".to_owned(), self.frame_count),
        ]);
        ScriptingStateDump {
            backend: PROVIDER_LABEL.to_owned(),
            loaded_modules: self.modules.values().cloned().collect(),
            counters,
            notes: vec![
                "This crate exposes an opaque engine.scripting boundary without embedding or naming any script implementation.".to_owned(),
                "Runtime load/invoke/frame methods use binary envelopes; JSON is reserved for control/debug methods.".to_owned(),
            ],
            ..ScriptingStateDump::default()
        }
    }
}
