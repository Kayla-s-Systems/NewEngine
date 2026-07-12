use serde::{Deserialize, Serialize};

use crate::{SchemaDiagnosticV1, SchemaTypeDescriptorV1, ENGINE_SCHEMA_SERVICE_ID};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaBindingFunctionV1 {
    pub name: String,
    pub method: String,
    pub request_type: String,
    pub response_type: String,
    pub gateway: String,
}

impl Default for SchemaBindingFunctionV1 {
    #[inline]
    fn default() -> Self {
        Self {
            name: String::new(),
            method: String::new(),
            request_type: String::new(),
            response_type: String::new(),
            gateway: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaBindingManifestV1 {
    pub schema: String,
    pub target_language: String,
    pub module_id: String,
    pub gateway: String,
    pub type_descriptors: Vec<SchemaTypeDescriptorV1>,
    pub functions: Vec<SchemaBindingFunctionV1>,
    pub diagnostics: Vec<SchemaDiagnosticV1>,
}

impl Default for SchemaBindingManifestV1 {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.schema.binding_manifest.v1".to_owned(),
            target_language: String::new(),
            module_id: String::new(),
            gateway: ENGINE_SCHEMA_SERVICE_ID.to_owned(),
            type_descriptors: Vec::new(),
            functions: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}
