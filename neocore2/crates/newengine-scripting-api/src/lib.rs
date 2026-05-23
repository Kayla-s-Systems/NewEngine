#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Engine-facing scripting service gateway id. Runtime consumers call this facade;
/// the host resolves it to the active scripting provider by descriptor metadata.
pub const ENGINE_SCRIPTING_SERVICE_ID: &str = "engine.scripting";

/// Optional subdomain ids reserved for future language/tooling surfaces. The base
/// runtime should prefer `engine.scripting` + backend capabilities; these ids are
/// not registered by the base runtime crate.
pub const ENGINE_SCRIPTING_MODULES_SERVICE_ID: &str = "engine.scripting.modules";
pub const ENGINE_SCRIPTING_DEBUG_SERVICE_ID: &str = "engine.scripting.debug";

/// Default/first-party provider service id for scripting backends.
pub const SCRIPTING_SERVICE_ID: &str = "scripting.api";
pub const SCRIPTING_BACKEND_CAPABILITY_ID: &str = "scripting.backend";
pub const SCRIPTING_MODULES_BACKEND_CAPABILITY_ID: &str = "scripting.modules.backend";
pub const SCRIPTING_DEBUG_BACKEND_CAPABILITY_ID: &str = "scripting.debug.backend";

/// Capability ids for future provider families. These are options behind the
/// neutral `engine.scripting` gateway, not core dependencies.
pub const SCRIPTING_BACKEND_LUA_CAPABILITY_ID: &str = "scripting.backend.lua";
pub const SCRIPTING_BACKEND_VISUAL_CAPABILITY_ID: &str = "scripting.backend.visual";
pub const SCRIPTING_BACKEND_WASM_CAPABILITY_ID: &str = "scripting.backend.wasm";
pub const SCRIPTING_BACKEND_NATIVE_CAPABILITY_ID: &str = "scripting.backend.native";

pub const SCRIPTING_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const SCRIPTING_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const SCRIPTING_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const SCRIPTING_SERVICE_METHOD_FRAME_JSON_V1: &str = "scripting.frame_json_v1";
pub const SCRIPTING_SERVICE_METHOD_LOAD_MODULE_JSON_V1: &str = "scripting.load_module_json_v1";
pub const SCRIPTING_SERVICE_METHOD_UNLOAD_MODULE_JSON_V1: &str = "scripting.unload_module_json_v1";
pub const SCRIPTING_SERVICE_METHOD_MODULE_MANIFEST_JSON_V1: &str = "scripting.module_manifest_json_v1";
pub const SCRIPTING_SERVICE_METHOD_VALIDATE_MODULE_REF_JSON_V1: &str = "scripting.validate_module_ref_json_v1";
pub const SCRIPTING_SERVICE_METHOD_DISPATCH_EVENT_JSON_V1: &str = "scripting.dispatch_event_json_v1";
pub const SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1: &str = "scripting.dump_state_json_v1";

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
        "newengine.scripting-api >= 0.1.x",
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
    SCRIPTING_SERVICE_METHOD_FRAME_JSON_V1,
    SCRIPTING_SERVICE_METHOD_LOAD_MODULE_JSON_V1,
    SCRIPTING_SERVICE_METHOD_UNLOAD_MODULE_JSON_V1,
    SCRIPTING_SERVICE_METHOD_MODULE_MANIFEST_JSON_V1,
    SCRIPTING_SERVICE_METHOD_VALIDATE_MODULE_REF_JSON_V1,
    SCRIPTING_SERVICE_METHOD_DISPATCH_EVENT_JSON_V1,
    SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1,
];

#[inline]
pub const fn scripting_service_methods() -> &'static [&'static str] {
    SCRIPTING_SERVICE_METHODS
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptLanguage {
    Neutral,
    Lua,
    Visual,
    Wasm,
    Native,
    External,
}

impl Default for ScriptLanguage {
    #[inline]
    fn default() -> Self { Self::Neutral }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingServiceInfo {
    pub protocol: String,
    pub gateway: String,
    pub provider: String,
    pub backend: String,
    pub features: Vec<String>,
    pub methods: Vec<String>,
    pub supported_languages: Vec<ScriptLanguage>,
}

impl Default for ScriptingServiceInfo {
    #[inline]
    fn default() -> Self {
        Self {
            protocol: "newengine.scripting-api/v1".to_owned(),
            gateway: ENGINE_SCRIPTING_SERVICE_ID.to_owned(),
            provider: SCRIPTING_SERVICE_ID.to_owned(),
            backend: "none".to_owned(),
            features: vec![
                "dto-frame-contract".to_owned(),
                "script-command-output".to_owned(),
                "script-event-output".to_owned(),
                "module-manifest-v1".to_owned(),
                "permission-descriptor-v1".to_owned(),
                "no-direct-world-access".to_owned(),
            ],
            methods: SCRIPTING_SERVICE_METHODS.iter().map(|it| (*it).to_owned()).collect(),
            supported_languages: vec![ScriptLanguage::Neutral],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingInvokeEnvelope {
    pub method: String,
    pub request: Value,
}

impl Default for ScriptingInvokeEnvelope {
    #[inline]
    fn default() -> Self {
        Self { method: String::new(), request: Value::Null }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptModuleRef {
    /// Canonical asset selector, usually `scripts/foo.ysc@entry`.
    pub reference: String,
    /// Optional normalized module id used by tooling/runtime caches.
    pub module_id: String,
    pub language: ScriptLanguage,
}

impl Default for ScriptModuleRef {
    #[inline]
    fn default() -> Self {
        Self { reference: String::new(), module_id: String::new(), language: ScriptLanguage::Neutral }
    }
}

impl ScriptModuleRef {
    #[inline]
    pub fn new(reference: impl Into<String>) -> Self {
        let reference = reference.into();
        Self { module_id: default_module_id_from_ref(&reference), reference, language: ScriptLanguage::Neutral }
    }

    #[inline]
    pub fn with_language(mut self, language: ScriptLanguage) -> Self {
        self.language = language;
        self
    }

    #[inline]
    pub fn is_ysc_entry_ref(&self) -> bool {
        let lower = self.reference.to_ascii_lowercase();
        lower.contains(".ysc@")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptPermission {
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
            schema: "newengine.scripting.module_descriptor.v1".to_owned(),
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
            schema: "newengine.scripting.module_manifest.v1".to_owned(),
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
    fn default() -> Self {
        Self { module_ref: ScriptModuleRef::default(), permissions: Vec::new(), metadata: BTreeMap::new() }
    }
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
    fn default() -> Self {
        Self { ok: false, module: ScriptModuleDescriptor::default(), diagnostics: Vec::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptModuleRefValidationResponse {
    pub ok: bool,
    pub module_ref: ScriptModuleRef,
    pub diagnostics: Vec<ScriptDiagnostic>,
}

impl Default for ScriptModuleRefValidationResponse {
    #[inline]
    fn default() -> Self {
        Self { ok: false, module_ref: ScriptModuleRef::default(), diagnostics: Vec::new() }
    }
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
    fn default() -> Self {
        Self { name: String::new(), target: String::new(), source: String::new(), payload: Value::Null }
    }
}

impl ScriptEvent {
    #[inline]
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Self::default() }
    }

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
    fn default() -> Self {
        Self { kind: String::new(), subject: String::new(), payload: Value::Null }
    }
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
    /// Command kind routed by the engine runtime, e.g. `entity.spawn`, `ui.emit`, `audio.play`.
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
    fn default() -> Self {
        Self { phase: String::new(), module: String::new(), message: String::new(), payload: Value::Null }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptDiagnostic {
    pub severity: ScriptDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub module: String,
    pub payload: Value,
}

impl Default for ScriptDiagnostic {
    #[inline]
    fn default() -> Self {
        Self {
            severity: ScriptDiagnosticSeverity::Info,
            code: String::new(),
            message: String::new(),
            module: String::new(),
            payload: Value::Null,
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
    pub backend: String,
    pub loaded_modules: Vec<ScriptModuleDescriptor>,
    pub notes: Vec<String>,
}

impl Default for ScriptingStateDump {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.scripting.state_dump.v1".to_owned(),
            gateway: ENGINE_SCRIPTING_SERVICE_ID.to_owned(),
            provider: SCRIPTING_SERVICE_ID.to_owned(),
            backend: "none".to_owned(),
            loaded_modules: Vec::new(),
            notes: Vec::new(),
        }
    }
}

#[inline]
fn default_module_id_from_ref(reference: &str) -> String {
    let mut id = reference.trim().replace('\\', "/");
    if id.is_empty() {
        return String::new();
    }
    id = id.trim_start_matches('/').to_ascii_lowercase();
    id.chars()
        .map(|ch| if matches!(ch, '/' | '@' | '.') { '_' } else { ch })
        .collect()
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
    fn empty_frame_preserves_frame_index() {
        let input = ScriptFrameInput::new(42, 1.0 / 60.0, 10.0, 7);
        let output = ScriptFrameOutput::empty_for(&input);
        assert_eq!(output.frame_index, 42);
        assert!(output.commands.is_empty());
    }

    #[test]
    fn service_info_lists_frame_method() {
        let info = ScriptingServiceInfo::default();
        assert!(info.methods.iter().any(|it| it == SCRIPTING_SERVICE_METHOD_FRAME_JSON_V1));
    }
}
