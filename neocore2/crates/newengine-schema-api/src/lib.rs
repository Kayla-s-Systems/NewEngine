#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Engine-facing schema/property registry gateway id.
///
/// Consumers call this facade. The selected provider/registry owns concrete
/// type metadata, property shapes and patch validation policy. Editor UI,
/// asset tools, component editors and scripting binders must not duplicate
/// per-format/per-component property branches.
pub const ENGINE_SCHEMA_SERVICE_ID: &str = "engine.schema";

/// Generic provider service id for schema registry implementations.
pub const SCHEMA_SERVICE_ID: &str = "schema.api";

/// Backend capability id declared by schema registry providers.
pub const SCHEMA_BACKEND_CAPABILITY_ID: &str = "schema.registry";

/// Stable runtime contract string for schema registry DTOs.
pub const SCHEMA_RUNTIME_CONTRACT: &str = "newengine.schema.registry.v1";

pub mod schema_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const DESCRIBE_TYPE_V1: &str = "schema.describe_type_v1";
    pub const DESCRIBE_PROPERTIES_V1: &str = "schema.describe_properties_v1";
    pub const VALIDATE_PATCH_V1: &str = "schema.validate_patch_v1";
    pub const DEFAULT_VALUE_V1: &str = "schema.default_value_v1";
    pub const BINDING_MANIFEST_V1: &str = "schema.binding_manifest_v1";
    pub const TRANSACTION_PLAN_V1: &str = "schema.transaction_plan_v1";
}

pub const SCHEMA_SERVICE_METHODS: &[&str] = &[
    schema_method::INFO_JSON,
    schema_method::INVOKE_JSON,
    schema_method::SHUTDOWN_V1,
    schema_method::DESCRIBE_TYPE_V1,
    schema_method::DESCRIBE_PROPERTIES_V1,
    schema_method::VALIDATE_PATCH_V1,
    schema_method::DEFAULT_VALUE_V1,
    schema_method::BINDING_MANIFEST_V1,
    schema_method::TRANSACTION_PLAN_V1,
];

pub const SCHEMA_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "schema",
        ENGINE_SCHEMA_SERVICE_ID,
        SCHEMA_SERVICE_ID,
        SCHEMA_BACKEND_CAPABILITY_ID,
    );

pub const SCHEMA_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_SCHEMA_SERVICE_ID,
        SCHEMA_RUNTIME_CONTRACT,
        SCHEMA_SERVICE_METHODS,
    );

pub const SCHEMA_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        SCHEMA_RUNTIME_CONTRACT_SPEC,
        Some(SCHEMA_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_SCHEMA_REGISTRY"),
    );

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaValueKindV1 {
    Null,
    Bool,
    Int,
    Float,
    String,
    StringList,
    Enum,
    Vec2,
    Vec3,
    Vec4,
    Color,
    AssetRef,
    EntityRef,
    Object,
    Array,
    Json,
}

impl Default for SchemaValueKindV1 {
    #[inline]
    fn default() -> Self {
        Self::String
    }
}

impl SchemaValueKindV1 {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool => "bool",
            Self::Int => "int",
            Self::Float => "float",
            Self::String => "string",
            Self::StringList => "string_list",
            Self::Enum => "enum",
            Self::Vec2 => "vec2",
            Self::Vec3 => "vec3",
            Self::Vec4 => "vec4",
            Self::Color => "color",
            Self::AssetRef => "asset_ref",
            Self::EntityRef => "entity_ref",
            Self::Object => "object",
            Self::Array => "array",
            Self::Json => "json",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaDiagnosticV1 {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

impl Default for SchemaDiagnosticV1 {
    #[inline]
    fn default() -> Self {
        Self {
            severity: "info".to_owned(),
            code: String::new(),
            message: String::new(),
            path: None,
        }
    }
}

impl SchemaDiagnosticV1 {
    #[inline]
    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "info".to_owned(),
            code: code.into(),
            message: message.into(),
            path: None,
        }
    }

    #[inline]
    pub fn warn(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "warning".to_owned(),
            code: code.into(),
            message: message.into(),
            path: None,
        }
    }

    #[inline]
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "error".to_owned(),
            code: code.into(),
            message: message.into(),
            path: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaPropertyDescriptorV1 {
    pub property_id: String,
    pub label: String,
    pub value_kind: SchemaValueKindV1,
    pub value: Value,
    pub default_value: Value,
    pub editable: bool,
    pub required: bool,
    pub readonly: bool,
    pub nullable: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub enum_values: Vec<String>,
    pub tags: Vec<String>,
    pub help: String,
    pub json_pointer: String,
    pub source_domain: String,
    pub metadata: BTreeMap<String, Value>,
}

impl Default for SchemaPropertyDescriptorV1 {
    #[inline]
    fn default() -> Self {
        Self {
            property_id: String::new(),
            label: String::new(),
            value_kind: SchemaValueKindV1::String,
            value: Value::Null,
            default_value: Value::Null,
            editable: false,
            required: false,
            readonly: false,
            nullable: true,
            min: None,
            max: None,
            enum_values: Vec::new(),
            tags: Vec::new(),
            help: String::new(),
            json_pointer: String::new(),
            source_domain: String::new(),
            metadata: BTreeMap::new(),
        }
    }
}

impl SchemaPropertyDescriptorV1 {
    #[inline]
    pub fn readonly(
        property_id: impl Into<String>,
        label: impl Into<String>,
        value_kind: SchemaValueKindV1,
        value: Value,
    ) -> Self {
        Self {
            property_id: property_id.into(),
            label: label.into(),
            value_kind,
            value,
            readonly: true,
            editable: false,
            ..Self::default()
        }
    }

    #[inline]
    pub fn editable(
        property_id: impl Into<String>,
        label: impl Into<String>,
        value_kind: SchemaValueKindV1,
        value: Value,
    ) -> Self {
        Self {
            property_id: property_id.into(),
            label: label.into(),
            value_kind,
            value: value.clone(),
            default_value: Value::Null,
            editable: true,
            readonly: false,
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaTypeDescriptorV1 {
    pub schema: String,
    pub type_id: String,
    pub display_name: String,
    pub domain: String,
    pub kind: String,
    pub version: u32,
    pub resource_ref: Option<String>,
    pub properties: Vec<SchemaPropertyDescriptorV1>,
    pub capabilities: Vec<String>,
    pub tags: Vec<String>,
    pub diagnostics: Vec<SchemaDiagnosticV1>,
    pub metadata: BTreeMap<String, Value>,
}

impl Default for SchemaTypeDescriptorV1 {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.schema.type_descriptor.v1".to_owned(),
            type_id: String::new(),
            display_name: String::new(),
            domain: String::new(),
            kind: "resource".to_owned(),
            version: 1,
            resource_ref: None,
            properties: Vec::new(),
            capabilities: Vec::new(),
            tags: Vec::new(),
            diagnostics: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaDescribeTypeRequestV1 {
    pub type_id: String,
    pub resource_ref: Option<String>,
    pub requester: String,
    pub include_properties: bool,
}

impl Default for SchemaDescribeTypeRequestV1 {
    #[inline]
    fn default() -> Self {
        Self {
            type_id: String::new(),
            resource_ref: None,
            requester: "engine.editor".to_owned(),
            include_properties: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaDescribeTypeResponseV1 {
    pub schema: String,
    pub accepted: bool,
    pub descriptor: Option<SchemaTypeDescriptorV1>,
    pub diagnostics: Vec<SchemaDiagnosticV1>,
}

impl Default for SchemaDescribeTypeResponseV1 {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.schema.describe_type.response.v1".to_owned(),
            accepted: false,
            descriptor: None,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaDescribePropertiesRequestV1 {
    pub type_id: String,
    pub resource_ref: Option<String>,
    pub requester: String,
}

impl Default for SchemaDescribePropertiesRequestV1 {
    #[inline]
    fn default() -> Self {
        Self {
            type_id: String::new(),
            resource_ref: None,
            requester: "engine.editor".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaDescribePropertiesResponseV1 {
    pub schema: String,
    pub accepted: bool,
    pub type_id: String,
    pub properties: Vec<SchemaPropertyDescriptorV1>,
    pub diagnostics: Vec<SchemaDiagnosticV1>,
}

impl Default for SchemaDescribePropertiesResponseV1 {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.schema.describe_properties.response.v1".to_owned(),
            accepted: false,
            type_id: String::new(),
            properties: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

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
            requester: "engine.editor".to_owned(),
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
pub struct SchemaDefaultValueRequestV1 {
    pub type_id: String,
    pub property_id: String,
    pub requester: String,
}

impl Default for SchemaDefaultValueRequestV1 {
    #[inline]
    fn default() -> Self {
        Self {
            type_id: String::new(),
            property_id: String::new(),
            requester: "engine.editor".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SchemaDefaultValueResponseV1 {
    pub schema: String,
    pub accepted: bool,
    pub type_id: String,
    pub property_id: String,
    pub value: Value,
    pub diagnostics: Vec<SchemaDiagnosticV1>,
}

impl Default for SchemaDefaultValueResponseV1 {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.schema.default_value.response.v1".to_owned(),
            accepted: false,
            type_id: String::new(),
            property_id: String::new(),
            value: Value::Null,
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
            requester: "engine.editor".to_owned(),
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
