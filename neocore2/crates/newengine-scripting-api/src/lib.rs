#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Engine-facing scripting service gateway id.
///
/// Runtime consumers call this facade. The host resolves it to the active
/// scripting provider by descriptor/capability metadata. The engine does not
/// know which scripting language, VM, graph runtime or bytecode format is used
/// behind this gateway.
pub const ENGINE_SCRIPTING_SERVICE_ID: &str = "engine.scripting";

/// Generic provider service id for the opaque scripting contract.
pub const SCRIPTING_SERVICE_ID: &str = "scripting.api";

/// Generic backend capability root. This is intentionally not a family of
/// language-specific capability ids; provider implementation details stay
/// provider-owned and opaque to core/runtime.
pub const SCRIPTING_BACKEND_CAPABILITY_ID: &str = "scripting.backend";

pub const SCRIPTING_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const SCRIPTING_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const SCRIPTING_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;

/// Primary opaque hot-path methods.
pub const SCRIPTING_SERVICE_METHOD_LOAD_MODULE_BYTES_V1: &str = "scripting.load_module_bytes_v1";
pub const SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1: &str = "scripting.invoke_bytes_v1";
pub const SCRIPTING_SERVICE_METHOD_FRAME_BYTES_V1: &str = "scripting.frame_bytes_v1";
pub const SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1: &str = "scripting.dump_state_json_v1";
pub const SCRIPTING_SERVICE_METHOD_VALIDATE_MODULE_REF_JSON_V1: &str = "scripting.validate_module_ref_json_v1";
pub const SCRIPTING_SERVICE_METHOD_UNLOAD_MODULE_JSON_V1: &str = "scripting.unload_module_json_v1";

/// Deprecated compatibility methods kept so existing engine callers do not
/// break while the scripting domain migrates to request-bytes/response-bytes.
/// They are adapters over the same opaque boundary and must not introduce
/// language/VM knowledge.
pub const SCRIPTING_SERVICE_METHOD_FRAME_JSON_V1: &str = "scripting.frame_json_v1";
pub const SCRIPTING_SERVICE_METHOD_LOAD_MODULE_JSON_V1: &str = "scripting.load_module_json_v1";
pub const SCRIPTING_SERVICE_METHOD_MODULE_MANIFEST_JSON_V1: &str = "scripting.module_manifest_json_v1";
pub const SCRIPTING_SERVICE_METHOD_DISPATCH_EVENT_JSON_V1: &str = "scripting.dispatch_event_json_v1";

/// Generic backend-family declaration for scripting providers.
pub const SCRIPTING_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "scripting",
        ENGINE_SCRIPTING_SERVICE_ID,
        SCRIPTING_SERVICE_ID,
        SCRIPTING_BACKEND_CAPABILITY_ID,
    );

/// Startup validation contract for the engine-facing scripting gateway.
pub const SCRIPTING_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_SCRIPTING_SERVICE_ID,
        "newengine.scripting-api >= 0.1.x opaque-bytes",
        newengine_service_api::JSON_CONTROL_SERVICE_METHODS_V1,
    );

/// Declarative startup requirement for scripting. Missing scripting degrades by
/// default; strict profiles may opt in to a fatal requirement through this env.
pub const SCRIPTING_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        SCRIPTING_RUNTIME_CONTRACT_SPEC,
        Some(SCRIPTING_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_SCRIPTING_BACKEND"),
    );

pub const SCRIPTING_SERVICE_METHODS: &[&str] = &[
    SCRIPTING_SERVICE_METHOD_INFO,
    SCRIPTING_SERVICE_METHOD_INVOKE,
    SCRIPTING_SERVICE_METHOD_SHUTDOWN_V1,
    SCRIPTING_SERVICE_METHOD_LOAD_MODULE_BYTES_V1,
    SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1,
    SCRIPTING_SERVICE_METHOD_FRAME_BYTES_V1,
    SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1,
    SCRIPTING_SERVICE_METHOD_VALIDATE_MODULE_REF_JSON_V1,
    SCRIPTING_SERVICE_METHOD_UNLOAD_MODULE_JSON_V1,
    SCRIPTING_SERVICE_METHOD_FRAME_JSON_V1,
    SCRIPTING_SERVICE_METHOD_LOAD_MODULE_JSON_V1,
    SCRIPTING_SERVICE_METHOD_MODULE_MANIFEST_JSON_V1,
    SCRIPTING_SERVICE_METHOD_DISPATCH_EVENT_JSON_V1,
];

#[inline]
pub const fn scripting_service_methods() -> &'static [&'static str] {
    SCRIPTING_SERVICE_METHODS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptModuleState {
    Declared,
    Loaded,
    Disabled,
    Failed,
}

impl Default for ScriptModuleState {
    #[inline]
    fn default() -> Self { Self::Declared }
}

pub type ScriptingModuleState = ScriptModuleState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptDiagnosticSeverity {
    Trace,
    Info,
    Warning,
    Error,
}

impl Default for ScriptDiagnosticSeverity {
    #[inline]
    fn default() -> Self { Self::Info }
}

pub type ScriptingDiagnosticSeverity = ScriptDiagnosticSeverity;

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
    fn default() -> Self { Self::Ok }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingServiceInfo {
    pub protocol: String,
    pub gateway: String,
    pub provider: String,
    /// Generic backend capability only. This must not encode language/VM names.
    pub backend_capability: String,
    /// Compatibility/debug label for the selected provider implementation.
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
            protocol: "newengine.scripting-api/opaque-bytes-v1".to_owned(),
            gateway: ENGINE_SCRIPTING_SERVICE_ID.to_owned(),
            provider: SCRIPTING_SERVICE_ID.to_owned(),
            backend_capability: SCRIPTING_BACKEND_CAPABILITY_ID.to_owned(),
            backend: "none".to_owned(),
            features: vec![
                "opaque-request-response".to_owned(),
                "ysc-entry-bytes".to_owned(),
                "provider-owned-interpretation".to_owned(),
                "no-language-whitelist".to_owned(),
                "no-direct-world-access".to_owned(),
                "json-compat-adapters".to_owned(),
            ],
            methods: SCRIPTING_SERVICE_METHODS.iter().map(|it| (*it).to_owned()).collect(),
            provider_metadata: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingInvokeEnvelope {
    pub method: String,
    /// Primary opaque request payload used by `*_bytes_v1` methods.
    pub request_bytes: Vec<u8>,
    /// Deprecated compatibility payload for previous JSON control callers.
    pub request: Value,
}

impl Default for ScriptingInvokeEnvelope {
    #[inline]
    fn default() -> Self {
        Self { method: String::new(), request_bytes: Vec::new(), request: Value::Null }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptModuleRef {
    /// Canonical runtime asset selector, usually `scripts/foo.ysc@entry`.
    pub reference: String,
    /// Optional normalized module id used by tooling/runtime caches. It does not
    /// imply any language/runtime identity.
    pub module_id: String,
}

impl Default for ScriptModuleRef {
    #[inline]
    fn default() -> Self { Self { reference: String::new(), module_id: String::new() } }
}

impl ScriptModuleRef {
    #[inline]
    pub fn new(reference: impl Into<String>) -> Self {
        let reference = reference.into();
        Self { module_id: default_module_id_from_ref(&reference), reference }
    }

    #[inline]
    pub fn is_ysc_entry_ref(&self) -> bool {
        self.reference.to_ascii_lowercase().contains(".ysc@")
    }
}

pub type ScriptingModuleRef = ScriptModuleRef;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptPermission {
    /// Engine-facing permission id. It must describe what the response asks the
    /// engine to do, not which provider/private language API was used.
    pub id: String,
    pub scope: String,
}

impl Default for ScriptPermission {
    #[inline]
    fn default() -> Self { Self { id: String::new(), scope: String::new() } }
}

impl ScriptPermission {
    #[inline]
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into(), scope: String::new() }
    }

    #[inline]
    pub fn scoped(id: impl Into<String>, scope: impl Into<String>) -> Self {
        Self { id: id.into(), scope: scope.into() }
    }
}

pub type ScriptingPermission = ScriptPermission;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptDiagnostic {
    pub severity: ScriptDiagnosticSeverity,
    pub code: String,
    pub message: String,
    /// Deprecated compatibility module field. New code should prefer
    /// `script_ref`; both remain generic `.ysc@entry` selectors.
    pub module: String,
    pub script_ref: String,
    pub payload: Value,
    pub payload_bytes: Vec<u8>,
}

impl Default for ScriptDiagnostic {
    #[inline]
    fn default() -> Self {
        Self {
            severity: ScriptDiagnosticSeverity::Info,
            code: String::new(),
            message: String::new(),
            module: String::new(),
            script_ref: String::new(),
            payload: Value::Null,
            payload_bytes: Vec::new(),
        }
    }
}

impl ScriptDiagnostic {
    #[inline]
    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { severity: ScriptDiagnosticSeverity::Info, code: code.into(), message: message.into(), ..Self::default() }
    }

    #[inline]
    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { severity: ScriptDiagnosticSeverity::Warning, code: code.into(), message: message.into(), ..Self::default() }
    }

    #[inline]
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { severity: ScriptDiagnosticSeverity::Error, code: code.into(), message: message.into(), ..Self::default() }
    }
}

pub type ScriptingDiagnostic = ScriptDiagnostic;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for ScriptingRequestBytes {
    #[inline]
    fn default() -> Self {
        Self {
            request_id: String::new(),
            script_ref: String::new(),
            operation: String::new(),
            payload_bytes: Vec::new(),
            context_bytes: Vec::new(),
            permissions: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingResponseBytes {
    pub request_id: String,
    pub status: ScriptingResponseStatus,
    pub payload_bytes: Vec<u8>,
    pub diagnostics: Vec<ScriptingDiagnostic>,
    pub trace_id: String,
    pub metadata: BTreeMap<String, String>,
}

impl Default for ScriptingResponseBytes {
    #[inline]
    fn default() -> Self {
        Self {
            request_id: String::new(),
            status: ScriptingResponseStatus::Ok,
            payload_bytes: Vec::new(),
            diagnostics: Vec::new(),
            trace_id: String::new(),
            metadata: BTreeMap::new(),
        }
    }
}

impl ScriptingResponseBytes {
    #[inline]
    pub fn empty_for(request: &ScriptingRequestBytes) -> Self {
        Self { request_id: request.request_id.clone(), status: ScriptingResponseStatus::Empty, ..Self::default() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingModuleLoadBytesRequest {
    pub module_ref: ScriptingModuleRef,
    /// Raw selected `.ysc@entry` bytes or provider-specific module bytes. Core
    /// stores/forwards these bytes and does not interpret them.
    pub module_bytes: Vec<u8>,
    pub permissions: Vec<ScriptingPermission>,
    pub metadata: BTreeMap<String, String>,
}

impl Default for ScriptingModuleLoadBytesRequest {
    #[inline]
    fn default() -> Self {
        Self { module_ref: ScriptingModuleRef::default(), module_bytes: Vec::new(), permissions: Vec::new(), metadata: BTreeMap::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingModuleUnloadRequest {
    pub module_ref: ScriptingModuleRef,
}

impl Default for ScriptingModuleUnloadRequest {
    #[inline]
    fn default() -> Self { Self { module_ref: ScriptingModuleRef::default() } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingModuleRecord {
    pub schema: String,
    pub module_ref: ScriptingModuleRef,
    pub state: ScriptingModuleState,
    pub permissions: Vec<ScriptingPermission>,
    pub module_bytes_len: u64,
    pub metadata: BTreeMap<String, String>,
    pub diagnostics: Vec<ScriptingDiagnostic>,
}

impl Default for ScriptingModuleRecord {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.scripting.module_record.v1".to_owned(),
            module_ref: ScriptingModuleRef::default(),
            state: ScriptingModuleState::Declared,
            permissions: Vec::new(),
            module_bytes_len: 0,
            metadata: BTreeMap::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingModuleLoadBytesResponse {
    pub ok: bool,
    pub module: ScriptingModuleRecord,
    pub diagnostics: Vec<ScriptingDiagnostic>,
}

impl Default for ScriptingModuleLoadBytesResponse {
    #[inline]
    fn default() -> Self { Self { ok: false, module: ScriptingModuleRecord::default(), diagnostics: Vec::new() } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingModuleRefValidationResponse {
    pub ok: bool,
    pub module_ref: ScriptingModuleRef,
    pub diagnostics: Vec<ScriptingDiagnostic>,
}

impl Default for ScriptingModuleRefValidationResponse {
    #[inline]
    fn default() -> Self { Self { ok: false, module_ref: ScriptingModuleRef::default(), diagnostics: Vec::new() } }
}

pub type ScriptModuleRefValidationResponse = ScriptingModuleRefValidationResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptModuleDescriptor {
    pub schema: String,
    pub module_ref: ScriptModuleRef,
    pub state: ScriptModuleState,
    pub entry_points: Vec<String>,
    pub permissions: Vec<ScriptPermission>,
    pub dependencies: Vec<String>,
    pub source_hash: String,
    pub api_version: u32,
    pub metadata: BTreeMap<String, Value>,
    pub diagnostics: Vec<ScriptDiagnostic>,
}

impl Default for ScriptModuleDescriptor {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.scripting.module_descriptor.compat.v1".to_owned(),
            module_ref: ScriptModuleRef::default(),
            state: ScriptModuleState::Declared,
            entry_points: Vec::new(),
            permissions: Vec::new(),
            dependencies: Vec::new(),
            source_hash: String::new(),
            api_version: 1,
            metadata: BTreeMap::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptModuleManifest {
    pub schema: String,
    pub gateway: String,
    pub modules: Vec<ScriptModuleDescriptor>,
    pub warnings: Vec<String>,
}

impl Default for ScriptModuleManifest {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.scripting.module_manifest.compat.v1".to_owned(),
            gateway: ENGINE_SCRIPTING_SERVICE_ID.to_owned(),
            modules: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptModuleLoadRequest {
    pub module_ref: ScriptModuleRef,
    pub permissions: Vec<ScriptPermission>,
    pub metadata: BTreeMap<String, Value>,
}

impl Default for ScriptModuleLoadRequest {
    #[inline]
    fn default() -> Self { Self { module_ref: ScriptModuleRef::default(), permissions: Vec::new(), metadata: BTreeMap::new() } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptModuleUnloadRequest {
    pub module_ref: ScriptModuleRef,
}

impl Default for ScriptModuleUnloadRequest {
    #[inline]
    fn default() -> Self { Self { module_ref: ScriptModuleRef::default() } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptModuleLoadResponse {
    pub ok: bool,
    pub module: ScriptModuleDescriptor,
    pub diagnostics: Vec<ScriptDiagnostic>,
}

impl Default for ScriptModuleLoadResponse {
    #[inline]
    fn default() -> Self { Self { ok: false, module: ScriptModuleDescriptor::default(), diagnostics: Vec::new() } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptBudget {
    pub max_commands: u32,
    pub max_events: u32,
    pub max_time_ms: f32,
    pub max_memory_bytes: u64,
}

impl Default for ScriptBudget {
    #[inline]
    fn default() -> Self {
        Self { max_commands: 1024, max_events: 1024, max_time_ms: 2.0, max_memory_bytes: 8 * 1024 * 1024 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptEvent {
    pub name: String,
    pub target: String,
    pub source: String,
    pub payload: Value,
}

impl Default for ScriptEvent {
    #[inline]
    fn default() -> Self { Self { name: String::new(), target: String::new(), source: String::new(), payload: Value::Null } }
}

impl ScriptEvent {
    #[inline]
    pub fn new(name: impl Into<String>) -> Self { Self { name: name.into(), ..Self::default() } }

    #[inline]
    pub fn with_payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptFact {
    pub kind: String,
    pub subject: String,
    pub payload: Value,
}

impl Default for ScriptFact {
    #[inline]
    fn default() -> Self { Self { kind: String::new(), subject: String::new(), payload: Value::Null } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptFrameInput {
    pub version: u32,
    pub frame_index: u64,
    pub dt_sec: f32,
    pub fixed_time_sec: f64,
    pub deterministic_seed: u64,
    pub modules: Vec<ScriptModuleRef>,
    pub events: Vec<ScriptEvent>,
    pub facts: Vec<ScriptFact>,
    pub budget: ScriptBudget,
}

impl Default for ScriptFrameInput {
    #[inline]
    fn default() -> Self {
        Self {
            version: 1,
            frame_index: 0,
            dt_sec: 0.0,
            fixed_time_sec: 0.0,
            deterministic_seed: 0,
            modules: Vec::new(),
            events: Vec::new(),
            facts: Vec::new(),
            budget: ScriptBudget::default(),
        }
    }
}

impl ScriptFrameInput {
    #[inline]
    pub fn new(frame_index: u64, dt_sec: f32, fixed_time_sec: f64, deterministic_seed: u64) -> Self {
        Self { frame_index, dt_sec, fixed_time_sec, deterministic_seed, ..Self::default() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptCommand {
    /// Engine-facing command kind, e.g. `entity.spawn`, `ui.emit`, `audio.play`.
    /// Command names describe requested engine work, not provider language APIs.
    pub kind: String,
    pub target: String,
    pub source_module: String,
    pub deterministic_key: String,
    pub payload: Value,
}

impl Default for ScriptCommand {
    #[inline]
    fn default() -> Self {
        Self {
            kind: String::new(),
            target: String::new(),
            source_module: String::new(),
            deterministic_key: String::new(),
            payload: Value::Null,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptTraceEvent {
    pub phase: String,
    pub module: String,
    pub message: String,
    pub payload: Value,
}

impl Default for ScriptTraceEvent {
    #[inline]
    fn default() -> Self { Self { phase: String::new(), module: String::new(), message: String::new(), payload: Value::Null } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptFrameOutput {
    pub version: u32,
    pub frame_index: u64,
    pub commands: Vec<ScriptCommand>,
    pub events: Vec<ScriptEvent>,
    pub diagnostics: Vec<ScriptDiagnostic>,
    pub trace: Vec<ScriptTraceEvent>,
}

impl Default for ScriptFrameOutput {
    #[inline]
    fn default() -> Self {
        Self { version: 1, frame_index: 0, commands: Vec::new(), events: Vec::new(), diagnostics: Vec::new(), trace: Vec::new() }
    }
}

impl ScriptFrameOutput {
    #[inline]
    pub fn empty_for(input: &ScriptFrameInput) -> Self {
        Self { frame_index: input.frame_index, ..Self::default() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptDispatchEventRequest {
    pub event: ScriptEvent,
    pub modules: Vec<ScriptModuleRef>,
}

impl Default for ScriptDispatchEventRequest {
    #[inline]
    fn default() -> Self { Self { event: ScriptEvent::default(), modules: Vec::new() } }
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

#[inline]
pub fn default_module_id_from_ref(reference: &str) -> String {
    let mut id = reference.trim().replace('\\', "/");
    if id.is_empty() {
        return String::new();
    }
    id = id.trim_start_matches('/').to_ascii_lowercase();
    id.chars().map(|ch| if matches!(ch, '/' | '@' | '.') { '_' } else { ch }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_ref_detects_ysc_entry() {
        let module_ref = ScriptModuleRef::new("scripts/missions/intro.ysc@main");
        assert!(module_ref.is_ysc_entry_ref());
        assert_eq!(module_ref.module_id, "scripts_missions_intro_ysc_main");
    }

    #[test]
    fn service_info_has_no_known_language_list() {
        let info = ScriptingServiceInfo::default();
        assert!(info.features.iter().any(|it| it == "no-language-whitelist"));
        assert!(info.methods.iter().any(|it| it == SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1));
        assert!(!info.features.iter().any(|it| it.contains("known-language")));
    }

    #[test]
    fn response_preserves_request_id_only() {
        let request = ScriptingRequestBytes { request_id: "req-1".to_owned(), ..ScriptingRequestBytes::default() };
        let response = ScriptingResponseBytes::empty_for(&request);
        assert_eq!(response.request_id, "req-1");
        assert_eq!(response.status, ScriptingResponseStatus::Empty);
        assert!(response.payload_bytes.is_empty());
    }

    #[test]
    fn compatibility_frame_output_preserves_frame_index() {
        let input = ScriptFrameInput::new(42, 1.0 / 60.0, 10.0, 7);
        let output = ScriptFrameOutput::empty_for(&input);
        assert_eq!(output.frame_index, 42);
        assert!(output.commands.is_empty());
    }
}
