#![forbid(unsafe_op_in_unsafe_fn)]
use newengine_schema_api::SchemaBindingManifestV1;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod wire;
pub use wire::*;

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

/// Primary hot-path methods. Payloads for these methods use the binary wire
/// helpers in `wire.rs`; JSON is reserved for control/debug surfaces only.
pub const SCRIPTING_SERVICE_METHOD_LOAD_MODULE_BYTES_V1: &str = "scripting.load_module_bytes_v1";
pub const SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1: &str = "scripting.invoke_bytes_v1";
pub const SCRIPTING_SERVICE_METHOD_FRAME_BYTES_V1: &str = "scripting.frame_bytes_v1";

/// Debug/control methods. These are inspectable control surfaces, not runtime
/// frame/module execution APIs.
pub const SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1: &str = "scripting.dump_state_json_v1";
pub const SCRIPTING_SERVICE_METHOD_VALIDATE_MODULE_REF_JSON_V1: &str = "scripting.validate_module_ref_json_v1";
pub const SCRIPTING_SERVICE_METHOD_UNLOAD_MODULE_JSON_V1: &str = "scripting.unload_module_json_v1";
/// Generates provider-facing scripting binding modules from engine.schema manifests.
pub const SCRIPTING_SERVICE_METHOD_BINDING_MANIFEST_JSON_V1: &str = "scripting.binding_manifest_json_v1";

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
        "newengine.scripting-api >= 0.2.x binary-opaque",
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
    SCRIPTING_SERVICE_METHOD_BINDING_MANIFEST_JSON_V1,
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
            methods: SCRIPTING_SERVICE_METHODS.iter().map(|it| (*it).to_owned()).collect(),
            provider_metadata: BTreeMap::new(),
        }
    }
}

/// Control invoke envelope. Its outer shape is JSON because it is routed through
/// the generic `invoke_json` service method, but the selected hot-path request is
/// carried as binary bytes and decoded by `wire.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingInvokeEnvelope {
    pub method: String,
    pub request_bytes: Vec<u8>,
}

impl Default for ScriptingInvokeEnvelope {
    #[inline]
    fn default() -> Self { Self { method: String::new(), request_bytes: Vec::new() } }
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
    pub fn new(id: impl Into<String>) -> Self { Self { id: id.into(), scope: String::new() } }

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
    pub script_ref: String,
    pub payload_bytes: Vec<u8>,
}

impl Default for ScriptDiagnostic {
    #[inline]
    fn default() -> Self {
        Self {
            severity: ScriptDiagnosticSeverity::Info,
            code: String::new(),
            message: String::new(),
            script_ref: String::new(),
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
        assert!(!info.features.iter().any(|it| it.contains("compat")));
    }

    #[test]
    fn response_preserves_request_id_only() {
        let request = ScriptingRequestBytes { request_id: "req-1".to_owned(), ..ScriptingRequestBytes::default() };
        let response = ScriptingResponseBytes::empty_for(&request);
        assert_eq!(response.request_id, "req-1");
        assert_eq!(response.status, ScriptingResponseStatus::Empty);
        assert!(response.payload_bytes.is_empty());
    }
}

/// Request used by scripting providers/tools to generate bindings from the shared schema registry.
///
/// The schema manifest is the source of truth. Scripting providers may choose a
/// target language or bytecode surface, but they must not invent a separate
/// reflection/type model for engine objects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingBindingGenerationRequest {
    pub schema: String,
    pub target_language: String,
    pub module_id: String,
    pub manifest: SchemaBindingManifestV1,
    pub requester: String,
}

impl Default for ScriptingBindingGenerationRequest {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.scripting.binding_generation.request.v1".to_owned(),
            target_language: String::new(),
            module_id: String::new(),
            manifest: SchemaBindingManifestV1::default(),
            requester: "engine.scripting".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingBindingGenerationResponse {
    pub schema: String,
    pub accepted: bool,
    /// Generated source/module payloads keyed by provider-owned module path.
    pub generated_modules: BTreeMap<String, String>,
    pub manifest: SchemaBindingManifestV1,
    pub diagnostics: Vec<String>,
}

impl Default for ScriptingBindingGenerationResponse {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.scripting.binding_generation.response.v1".to_owned(),
            accepted: false,
            generated_modules: BTreeMap::new(),
            manifest: SchemaBindingManifestV1::default(),
            diagnostics: Vec::new(),
        }
    }
}
