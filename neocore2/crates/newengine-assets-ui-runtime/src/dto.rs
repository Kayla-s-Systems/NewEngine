use super::*;

#[derive(Clone, Debug, Serialize)]
pub struct AssetsUiServiceInfo {
    pub id: &'static str,

    pub gateway: &'static str,

    pub provider: &'static str,

    pub contract: &'static str,

    pub byte_owner: &'static str,

    pub semantic_owner: &'static str,

    pub runtime_owner: &'static str,

    pub methods: &'static [&'static str],

    pub policy: &'static str,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetsUiRefRequest {
    pub document_ref: String,

    pub ui_ref: String,

    pub logical_path: String,

    pub entry: String,
}

impl Default for AssetsUiRefRequest {
    #[inline]
    fn default() -> Self {
        Self {
            document_ref: String::new(),

            ui_ref: String::new(),

            logical_path: String::new(),

            entry: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetsUiCompileRequest {
    pub document_ref: String,

    pub ui_ref: String,

    pub logical_path: String,

    pub entry: String,

    pub style_ref: Option<String>,

    pub source_kind: UiDocumentSourceKind,

    pub stream_id: Option<String>,

    pub generator_id: Option<String>,

    pub mount_runtime: bool,
}

impl Default for AssetsUiCompileRequest {
    #[inline]
    fn default() -> Self {
        Self {
            document_ref: String::new(),

            ui_ref: String::new(),

            logical_path: String::new(),

            entry: String::new(),

            style_ref: None,

            source_kind: UiDocumentSourceKind::Asset,

            stream_id: None,

            generator_id: None,

            mount_runtime: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetsUiCompileResponse {
    pub ok: bool,

    pub schema: String,

    pub document_ref: String,

    pub logical_path: String,

    pub vfs_path: String,

    pub entry: String,

    pub surface_id: String,

    pub xmlcentral: String,

    pub compiled_document: UiCompiledDocument,

    pub navigation_document: Option<UiNodeNavigationDocument>,

    pub source_kind: UiDocumentSourceKind,

    pub style_ref: Option<String>,

    pub dependencies: Vec<String>,

    pub style_dependencies: Vec<String>,

    pub warnings: Vec<String>,
}

impl Default for AssetsUiCompileResponse {
    #[inline]
    fn default() -> Self {
        Self {
            ok: false,

            schema: "newengine.assets.ui.compile_document.response.v1".to_owned(),

            document_ref: String::new(),

            logical_path: String::new(),

            vfs_path: String::new(),

            entry: String::new(),

            surface_id: String::new(),

            xmlcentral: String::new(),

            compiled_document: UiCompiledDocument::default(),

            navigation_document: None,

            source_kind: UiDocumentSourceKind::Asset,

            style_ref: None,

            dependencies: Vec::new(),

            style_dependencies: Vec::new(),

            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(default)]
pub struct AssetsUiDiagnosticResponse {
    pub ok: bool,

    pub schema: String,

    pub document_ref: String,

    pub logical_path: String,

    pub entry: String,

    pub entry_id: String,

    pub source_span: UiSourceSpan,

    pub message: String,

    pub warnings: Vec<String>,
}

impl Default for AssetsUiDiagnosticResponse {
    #[inline]
    fn default() -> Self {
        Self {
            ok: false,

            schema: "newengine.assets.ui.diagnostic.v1".to_owned(),

            document_ref: String::new(),

            logical_path: String::new(),

            entry: String::new(),

            entry_id: String::new(),

            source_span: UiSourceSpan::default(),

            message: String::new(),

            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetsUiDialectInspectRequest {
    pub dialect_ref: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetsUiInvalidateRequest {
    pub document_ref: String,
    pub dialect_ref: String,
    pub all: bool,
}
