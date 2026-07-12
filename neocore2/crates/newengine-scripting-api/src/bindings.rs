use newengine_schema_api::SchemaBindingManifestV1;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Request used by scripting providers/tools to generate bindings from the shared schema registry.
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
