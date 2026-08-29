#![forbid(unsafe_op_in_unsafe_fn)]

//! Browser-facing NEF8/ListFile contracts.
//!
//! This module is deliberately DTO-only. It describes what a provider explicitly
//! declares for an Asset Browser surface; it does not grant the browser permission
//! to infer semantics from extensions, names or hashes.

pub const ASSETS_LISTFILE_NEF8_CAPABILITY_ID: &str = "assets.listfile.nef8";
pub const ASSETS_BROWSER_LISTFILE_EXPLAIN_CAPABILITY_ID: &str = "assets.browser.listfile.explain";
pub const ASSETS_BROWSER_PREVIEW_CAPABILITY_ID: &str = "assets.browser.preview";
pub const ASSETS_BROWSER_EDITOR_SCHEMA_CAPABILITY_ID: &str = "assets.browser.editor.schema";
pub const ASSETS_BROWSER_MUTATION_CAPABILITY_ID: &str = "assets.browser.mutation";

pub const ASSET_BROWSER_EXPLAIN_LISTFILE_OUTPUT: &str = "asset.browser_explain_listfile_v1";
pub const ASSET_BROWSER_PREVIEW_OUTPUT: &str = "asset.browser_preview_v1";
pub const ASSET_BROWSER_EDITOR_SCHEMA_OUTPUT: &str = "asset.browser_editor_schema_v1";
pub const ASSET_BROWSER_APPLY_EDIT_OUTPUT: &str = "asset.browser_apply_edit_v1";

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BrowserDiagnosticV1 {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub entry_ref: Option<String>,
}

impl Default for BrowserDiagnosticV1 {
    fn default() -> Self {
        Self { severity: "info".to_owned(), code: String::new(), message: String::new(), entry_ref: None }
    }
}

impl BrowserDiagnosticV1 {
    #[inline]
    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { severity: "info".to_owned(), code: code.into(), message: message.into(), entry_ref: None }
    }

    #[inline]
    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { severity: "warning".to_owned(), code: code.into(), message: message.into(), entry_ref: None }
    }

    #[inline]
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { severity: "error".to_owned(), code: code.into(), message: message.into(), entry_ref: None }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct EntryModelDescriptorV1 {
    pub schema_id: String,
    pub entry_kind_id: String,
    pub display_name_field: Option<String>,
    pub stable_id_field: Option<String>,
    pub fields: Vec<String>,
}

impl Default for EntryModelDescriptorV1 {
    fn default() -> Self {
        Self { schema_id: String::new(), entry_kind_id: String::new(), display_name_field: None, stable_id_field: None, fields: Vec::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BrowserRouteDescriptorV1 {
    pub gateway: String,
    pub method: String,
    pub contract_id: String,
    pub provider_id: String,
}

impl Default for BrowserRouteDescriptorV1 {
    fn default() -> Self {
        Self { gateway: String::new(), method: String::new(), contract_id: String::new(), provider_id: String::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PreviewContractV1 {
    pub route: BrowserRouteDescriptorV1,
    pub profiles: Vec<String>,
    pub output_kind: String,
}

impl Default for PreviewContractV1 {
    fn default() -> Self { Self { route: BrowserRouteDescriptorV1::default(), profiles: Vec::new(), output_kind: String::new() } }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct EditorContractV1 {
    pub route: BrowserRouteDescriptorV1,
    pub schema_id: String,
    pub readonly: bool,
}

impl Default for EditorContractV1 {
    fn default() -> Self { Self { route: BrowserRouteDescriptorV1::default(), schema_id: String::new(), readonly: true } }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MutationContractV1 {
    pub route: BrowserRouteDescriptorV1,
    pub patch_schema_id: String,
    pub requires_package_writer: bool,
}

impl Default for MutationContractV1 {
    fn default() -> Self {
        Self { route: BrowserRouteDescriptorV1::default(), patch_schema_id: String::new(), requires_package_writer: true }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ListFileBrowserExplanationV1 {
    pub file_ref: String,
    pub content_kind: String,
    pub provider_id: String,
    pub entry_model: Option<EntryModelDescriptorV1>,
    pub preview_contract: Option<PreviewContractV1>,
    pub editor_contract: Option<EditorContractV1>,
    pub mutation_contract: Option<MutationContractV1>,
    pub diagnostics: Vec<BrowserDiagnosticV1>,
}

impl Default for ListFileBrowserExplanationV1 {
    fn default() -> Self {
        Self {
            file_ref: String::new(),
            content_kind: String::new(),
            provider_id: String::new(),
            entry_model: None,
            preview_contract: None,
            editor_contract: None,
            mutation_contract: None,
            diagnostics: Vec::new(),
        }
    }
}

impl ListFileBrowserExplanationV1 {
    #[inline]
    pub fn has_preview(&self) -> bool { self.preview_contract.is_some() }

    #[inline]
    pub fn has_editor(&self) -> bool { self.editor_contract.is_some() }

    #[inline]
    pub fn can_mutate(&self) -> bool { self.editor_contract.is_some() && self.mutation_contract.is_some() }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ListFileBrowserExplanationResponseV1 {
    pub file_ref: String,
    pub explanation: Option<ListFileBrowserExplanationV1>,
    pub diagnostics: Vec<BrowserDiagnosticV1>,
}

impl Default for ListFileBrowserExplanationResponseV1 {
    fn default() -> Self { Self { file_ref: String::new(), explanation: None, diagnostics: Vec::new() } }
}

impl ListFileBrowserExplanationResponseV1 {
    #[inline]
    pub fn unknown_semantics(file_ref: impl Into<String>) -> Self {
        Self {
            file_ref: file_ref.into(),
            explanation: None,
            diagnostics: vec![BrowserDiagnosticV1::info(
                "browser.explanation.missing",
                "known ListFile container; provider did not declare browser semantics",
            )],
        }
    }
}
