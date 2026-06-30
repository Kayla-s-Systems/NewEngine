// Split from lib.rs to keep the UI API DTO surface navigable.
// This file is included flat from lib.rs to preserve the existing public API.

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiBindingMode {
    OneWay,
    TwoWay,
    Event,
}

impl Default for UiBindingMode {
    #[inline]
    fn default() -> Self { Self::OneWay }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiUpdatePolicy {
    Frame,
    Event,
    Dirty,
    OnChange,
    Manual,
}

impl Default for UiUpdatePolicy {
    #[inline]
    fn default() -> Self { Self::OnChange }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiStateSource {
    pub id: String,
    pub source: String,
    pub contract: String,
    pub update_policy: UiUpdatePolicy,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiBindingEdge {
    pub element_id: String,
    pub property: String,
    pub source_id: String,
    pub path: String,
    pub mode: UiBindingMode,
    #[serde(default)]
    pub fallback: Option<String>,
    #[serde(default)]
    pub transform: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiActionEdge {
    pub element_id: String,
    pub trigger: String,
    pub action_id: String,
    pub target_gateway: String,
    pub command: String,
    #[serde(default)]
    pub payload_schema: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiBindingPlan {
    pub document_ref: String,
    pub surface_id: String,
    pub state_sources: Vec<UiStateSource>,
    pub bindings: Vec<UiBindingEdge>,
    pub actions: Vec<UiActionEdge>,
}

/// Runtime source kind for UI documents.
///
/// UI may come from compiled `.neui` assets, a runtime stream, or a generated
/// document, but all paths must end in the same compiled DTO/mount contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDocumentSourceKind {
    Asset,
    Stream,
    Generated,
}

impl Default for UiDocumentSourceKind {
    #[inline]
    fn default() -> Self { Self::Asset }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDocumentSource {
    pub kind: UiDocumentSourceKind,
    pub document_ref: String,
    pub style_ref: Option<String>,
    pub stream_id: Option<String>,
    pub generator_id: Option<String>,
}

impl Default for UiDocumentSource {
    #[inline]
    fn default() -> Self {
        Self {
            kind: UiDocumentSourceKind::Asset,
            document_ref: String::new(),
            style_ref: None,
            stream_id: None,
            generator_id: None,
        }
    }
}


/// Source location carried by compiled `.neui` diagnostics. Authoring/import
/// tools should fill line/column from the original asset; generated documents
/// may leave them as zero.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiSourceSpan {
    pub source_ref: String,
    pub line: u32,
    pub column: u32,
}
impl UiSourceSpan {
    #[inline]
    pub fn display(&self, fallback_ref: &str) -> String {
        let source = if self.source_ref.trim().is_empty() { fallback_ref } else { self.source_ref.as_str() };
        format!("{}:{}:{}", source, self.line, self.column)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiSchemaSeverity {
    Info,
    Warning,
    Error,
}

impl Default for UiSchemaSeverity {
    #[inline]
    fn default() -> Self { Self::Error }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSchemaDiagnostic {
    pub severity: UiSchemaSeverity,
    pub code: String,
    pub message: String,
    pub span: UiSourceSpan,
    pub node_id: String,
    pub component_id: String,
    pub expected_prop: Option<String>,
}

impl Default for UiSchemaDiagnostic {
    fn default() -> Self {
        Self {
            severity: UiSchemaSeverity::Error,
            code: String::new(),
            message: String::new(),
            span: UiSourceSpan::default(),
            node_id: String::new(),
            component_id: String::new(),
            expected_prop: None,
        }
    }
}

impl UiSchemaDiagnostic {
    #[inline]
    pub fn error(code: impl Into<String>, message: impl Into<String>, span: UiSourceSpan) -> Self {
        Self { severity: UiSchemaSeverity::Error, code: code.into(), message: message.into(), span, ..Self::default() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSchemaValidationReport {
    pub ok: bool,
    pub diagnostics: Vec<UiSchemaDiagnostic>,
}

impl Default for UiSchemaValidationReport {
    fn default() -> Self { Self { ok: true, diagnostics: Vec::new() } }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiComponentLibraryRef {
    pub library_ref: String,
    pub entries: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiThemeLibraryRef {
    pub theme_ref: String,
    pub entries: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiComponentTemplate {
    pub id: String,
    pub source_ref: String,
    pub required_props: Vec<String>,
    pub root: UiNodeRequest,
}

impl Default for UiComponentTemplate {
    fn default() -> Self {
        Self { id: String::new(), source_ref: String::new(), required_props: Vec::new(), root: UiNodeRequest::new("component.root", UiRuntimeNodeKind::Panel) }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiDependencyResolveReport {
    pub resolved: Vec<String>,
    pub missing: Vec<String>,
    pub diagnostics: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiCompiledDocument {
    pub version: u32,
    pub source: UiDocumentSource,
    pub document_ref: String,
    pub surface_id: String,
    pub root_id: String,
    pub theme_ref: Option<String>,
    pub style_ref: Option<String>,
    pub dependencies: Vec<String>,
    pub style_dependencies: Vec<String>,
    pub component_libraries: Vec<UiComponentLibraryRef>,
    pub theme_libraries: Vec<UiThemeLibraryRef>,
    pub component_templates: Vec<UiComponentTemplate>,
    pub root: Option<UiNodeRequest>,
    pub binding_plan: UiBindingPlan,
    pub validation: UiSchemaValidationReport,
    pub dependency_report: UiDependencyResolveReport,
}

impl Default for UiCompiledDocument {
    fn default() -> Self {
        Self {
            version: 1,
            source: UiDocumentSource::default(),
            document_ref: String::new(),
            surface_id: String::new(),
            root_id: String::new(),
            theme_ref: None,
            style_ref: None,
            dependencies: Vec::new(),
            style_dependencies: Vec::new(),
            component_libraries: Vec::new(),
            theme_libraries: Vec::new(),
            component_templates: Vec::new(),
            root: None,
            binding_plan: UiBindingPlan::default(),
            validation: UiSchemaValidationReport::default(),
            dependency_report: UiDependencyResolveReport::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiStateChange {
    pub source_id: String,
    pub path: String,
    pub value: serde_json::Value,
}

impl Default for UiStateChange {
    fn default() -> Self {
        Self { source_id: String::new(), path: String::new(), value: serde_json::Value::Null }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiStatePatch {
    pub frame_index: u64,
    pub surface_id: String,
    pub changes: Vec<UiStateChange>,
}
impl UiStatePatch {
    #[inline]
    pub fn new(frame_index: u64, surface_id: impl Into<String>) -> Self {
        Self { frame_index, surface_id: surface_id.into(), changes: Vec::new() }
    }

    #[inline]
    pub fn with_change(mut self, source_id: impl Into<String>, path: impl Into<String>, value: serde_json::Value) -> Self {
        self.changes.push(UiStateChange { source_id: source_id.into(), path: path.into(), value });
        self
    }
}
