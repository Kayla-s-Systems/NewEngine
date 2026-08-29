use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const SCRIPTING_COMPLETION_REQUEST_SCHEMA_V1: &str =
    "newengine.scripting.completion.request.v1";
pub const SCRIPTING_COMPLETION_RESPONSE_SCHEMA_V1: &str =
    "newengine.scripting.completion.response.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingCompletionRequest {
    pub schema: String,
    pub module_ref: String,
    pub language_id: String,
    pub source_text: String,
    /// Monotonic editor-owned document revision. Providers echo it in the response so callers
    /// can discard stale asynchronous completion results without inspecting provider state.
    pub document_revision: u64,
    /// UTF-8 byte offset into `source_text`.
    pub cursor_byte_offset: usize,
    pub trigger_character: Option<String>,
    pub max_items: usize,
}

impl Default for ScriptingCompletionRequest {
    fn default() -> Self {
        Self {
            schema: SCRIPTING_COMPLETION_REQUEST_SCHEMA_V1.to_owned(),
            module_ref: String::new(),
            language_id: String::new(),
            source_text: String::new(),
            document_revision: 0,
            cursor_byte_offset: 0,
            trigger_character: None,
            max_items: 128,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingCompletionItem {
    pub label: String,
    pub insert_text: String,
    /// Provider-neutral kind string: keyword/function/class/interface/type/variable/property/etc.
    pub kind: String,
    pub detail: String,
    pub documentation: String,
    pub sort_text: String,
    pub filter_text: String,
    pub replacement_start_byte: usize,
    pub replacement_end_byte: usize,
    pub provider_data: BTreeMap<String, String>,
}

impl Default for ScriptingCompletionItem {
    fn default() -> Self {
        Self {
            label: String::new(),
            insert_text: String::new(),
            kind: "text".to_owned(),
            detail: String::new(),
            documentation: String::new(),
            sort_text: String::new(),
            filter_text: String::new(),
            replacement_start_byte: 0,
            replacement_end_byte: 0,
            provider_data: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingCompletionResponse {
    pub schema: String,
    pub provider: String,
    pub language_id: String,
    pub document_revision: u64,
    pub is_incomplete: bool,
    pub replacement_start_byte: usize,
    pub replacement_end_byte: usize,
    pub items: Vec<ScriptingCompletionItem>,
    pub diagnostics: Vec<String>,
}

impl Default for ScriptingCompletionResponse {
    fn default() -> Self {
        Self {
            schema: SCRIPTING_COMPLETION_RESPONSE_SCHEMA_V1.to_owned(),
            provider: String::new(),
            language_id: String::new(),
            document_revision: 0,
            is_incomplete: false,
            replacement_start_byte: 0,
            replacement_end_byte: 0,
            items: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

pub const SCRIPTING_SIGNATURE_HELP_REQUEST_SCHEMA_V1: &str =
    "newengine.scripting.signature_help.request.v1";
pub const SCRIPTING_SIGNATURE_HELP_RESPONSE_SCHEMA_V1: &str =
    "newengine.scripting.signature_help.response.v1";
pub const SCRIPTING_TOOLING_CATALOG_SCHEMA_V1: &str = "newengine.scripting.tooling_catalog.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScriptingToolingFunction {
    pub namespace: String,
    pub name: String,
    pub parameters: Vec<String>,
    pub return_type: String,
    pub detail: String,
    pub gateway: String,
    pub method: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingToolingCatalog {
    pub schema: String,
    pub revision: u64,
    pub root_namespace: String,
    pub functions: Vec<ScriptingToolingFunction>,
    pub diagnostics: Vec<String>,
}

impl Default for ScriptingToolingCatalog {
    fn default() -> Self {
        Self {
            schema: SCRIPTING_TOOLING_CATALOG_SCHEMA_V1.to_owned(),
            revision: 0,
            root_namespace: "NorthStar".to_owned(),
            functions: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingSignatureHelpRequest {
    pub schema: String,
    pub module_ref: String,
    pub language_id: String,
    pub source_text: String,
    pub document_revision: u64,
    pub cursor_byte_offset: usize,
}

impl Default for ScriptingSignatureHelpRequest {
    fn default() -> Self {
        Self {
            schema: SCRIPTING_SIGNATURE_HELP_REQUEST_SCHEMA_V1.to_owned(),
            module_ref: String::new(),
            language_id: String::new(),
            source_text: String::new(),
            document_revision: 0,
            cursor_byte_offset: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScriptingSignatureInformation {
    pub label: String,
    pub parameters: Vec<String>,
    pub return_type: String,
    pub documentation: String,
    pub provider_data: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingSignatureHelpResponse {
    pub schema: String,
    pub provider: String,
    pub language_id: String,
    pub document_revision: u64,
    pub active_signature: usize,
    pub active_parameter: usize,
    pub signatures: Vec<ScriptingSignatureInformation>,
    pub diagnostics: Vec<String>,
}

impl Default for ScriptingSignatureHelpResponse {
    fn default() -> Self {
        Self {
            schema: SCRIPTING_SIGNATURE_HELP_RESPONSE_SCHEMA_V1.to_owned(),
            provider: String::new(),
            language_id: String::new(),
            document_revision: 0,
            active_signature: 0,
            active_parameter: 0,
            signatures: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}
