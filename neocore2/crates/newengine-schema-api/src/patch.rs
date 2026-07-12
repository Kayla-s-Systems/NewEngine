use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{service::DEFAULT_SCHEMA_REQUESTER, SchemaDiagnosticV1};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaPatchOperationV1 {
    pub op: String,
    pub path: String,
    pub property_id: String,
    pub value: Value,
    pub old_value: Option<Value>,
}

impl Default for SchemaPatchOperationV1 {
    #[inline]
    fn default() -> Self {
        Self {
            op: "replace".to_owned(),
            path: String::new(),
            property_id: String::new(),
            value: Value::Null,
            old_value: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaPatchDtoV1 {
    pub schema: String,
    pub target_type: String,
    pub target_ref: String,
    pub base_revision: Option<String>,
    pub requester: String,
    pub transaction_id: String,
    pub operations: Vec<SchemaPatchOperationV1>,
}

impl Default for SchemaPatchDtoV1 {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.schema.patch.v1".to_owned(),
            target_type: String::new(),
            target_ref: String::new(),
            base_revision: None,
            requester: DEFAULT_SCHEMA_REQUESTER.to_owned(),
            transaction_id: String::new(),
            operations: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaPatchValidationRequestV1 {
    pub patch: SchemaPatchDtoV1,
    pub mode: String,
}

impl Default for SchemaPatchValidationRequestV1 {
    #[inline]
    fn default() -> Self {
        Self {
            patch: SchemaPatchDtoV1::default(),
            mode: "validate_only".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaPatchValidationResponseV1 {
    pub schema: String,
    pub accepted: bool,
    pub normalized_patch: Option<SchemaPatchDtoV1>,
    pub undo_operations: Vec<SchemaPatchOperationV1>,
    pub diagnostics: Vec<SchemaDiagnosticV1>,
}

impl Default for SchemaPatchValidationResponseV1 {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.schema.validate_patch.response.v1".to_owned(),
            accepted: false,
            normalized_patch: None,
            undo_operations: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaTransactionDtoV1 {
    pub schema: String,
    pub transaction_id: String,
    pub target_type: String,
    pub target_ref: String,
    pub base_revision: Option<String>,
    pub requester: String,
    pub reason: String,
    pub operations: Vec<SchemaPatchOperationV1>,
    pub undo_operations: Vec<SchemaPatchOperationV1>,
}

impl Default for SchemaTransactionDtoV1 {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.schema.transaction.v1".to_owned(),
            transaction_id: String::new(),
            target_type: String::new(),
            target_ref: String::new(),
            base_revision: None,
            requester: DEFAULT_SCHEMA_REQUESTER.to_owned(),
            reason: String::new(),
            operations: Vec::new(),
            undo_operations: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaTransactionResultV1 {
    pub schema: String,
    pub transaction_id: String,
    pub accepted: bool,
    pub committed: bool,
    pub revision: Option<String>,
    pub diagnostics: Vec<SchemaDiagnosticV1>,
}

impl Default for SchemaTransactionResultV1 {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.schema.transaction_result.v1".to_owned(),
            transaction_id: String::new(),
            accepted: false,
            committed: false,
            revision: None,
            diagnostics: Vec::new(),
        }
    }
}
