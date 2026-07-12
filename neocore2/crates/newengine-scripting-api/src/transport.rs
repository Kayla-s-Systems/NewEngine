use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{
    ScriptingDiagnostic, ScriptingModuleRecord, ScriptingPermission, ENGINE_SCRIPTING_SERVICE_ID,
    SCRIPTING_BACKEND_CAPABILITY_ID, SCRIPTING_SERVICE_ID, SCRIPTING_SERVICE_METHODS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptingResponseStatus {
    Ok,
    Empty,
    Rejected,
    InvalidRequest,
    ProviderError,
}

impl Default for ScriptingResponseStatus {
    #[inline]
    fn default() -> Self {
        Self::Ok
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingServiceInfo {
    pub protocol: String,
    pub gateway: String,
    pub provider: String,
    /// Generic backend capability only. This must not encode language/VM names.
    pub backend_capability: String,
    /// Debug label for the selected provider implementation.
    pub backend: String,
    pub features: Vec<String>,
    pub methods: Vec<String>,
    /// Provider-owned display metadata. Core may forward/display it, but must
    /// not branch on its inner keys or values.
    pub provider_metadata: BTreeMap<String, String>,
}

impl Default for ScriptingServiceInfo {
    #[inline]
    fn default() -> Self {
        Self {
            protocol: "newengine.scripting-api/binary-opaque-v1".to_owned(),
            gateway: ENGINE_SCRIPTING_SERVICE_ID.to_owned(),
            provider: SCRIPTING_SERVICE_ID.to_owned(),
            backend_capability: SCRIPTING_BACKEND_CAPABILITY_ID.to_owned(),
            backend: "none".to_owned(),
            features: vec![
                "binary-request-response".to_owned(),
                "ysc-entry-bytes".to_owned(),
                "provider-owned-interpretation".to_owned(),
                "no-language-whitelist".to_owned(),
                "no-direct-world-access".to_owned(),
            ],
            methods: SCRIPTING_SERVICE_METHODS
                .iter()
                .map(|method| (*method).to_owned())
                .collect(),
            provider_metadata: BTreeMap::new(),
        }
    }
}

/// Control invoke envelope. Its outer shape is JSON because it is routed through
/// the generic `invoke_json` method, while the hot-path request stays binary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingInvokeEnvelope {
    pub method: String,
    pub request_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingRequestBytes {
    pub request_id: String,
    pub script_ref: String,
    /// Provider-owned operation/event name. Core routes it but does not
    /// interpret operation semantics beyond envelope validation and budgets.
    pub operation: String,
    pub payload_bytes: Vec<u8>,
    pub context_bytes: Vec<u8>,
    pub permissions: Vec<ScriptingPermission>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingResponseBytes {
    pub request_id: String,
    pub status: ScriptingResponseStatus,
    pub payload_bytes: Vec<u8>,
    pub diagnostics: Vec<ScriptingDiagnostic>,
    pub trace_id: String,
    pub metadata: BTreeMap<String, String>,
}

impl ScriptingResponseBytes {
    #[inline]
    pub fn empty_for(request: &ScriptingRequestBytes) -> Self {
        Self {
            request_id: request.request_id.clone(),
            status: ScriptingResponseStatus::Empty,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingStateDump {
    pub schema: String,
    pub gateway: String,
    pub provider: String,
    pub backend_capability: String,
    pub backend: String,
    pub loaded_modules: Vec<ScriptingModuleRecord>,
    pub counters: BTreeMap<String, u64>,
    pub notes: Vec<String>,
    pub provider_metadata: BTreeMap<String, String>,
}

impl Default for ScriptingStateDump {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.scripting.state_dump.v1".to_owned(),
            gateway: ENGINE_SCRIPTING_SERVICE_ID.to_owned(),
            provider: SCRIPTING_SERVICE_ID.to_owned(),
            backend_capability: SCRIPTING_BACKEND_CAPABILITY_ID.to_owned(),
            backend: "none".to_owned(),
            loaded_modules: Vec::new(),
            counters: BTreeMap::new(),
            notes: Vec::new(),
            provider_metadata: BTreeMap::new(),
        }
    }
}
