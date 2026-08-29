use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::SchemaDiagnosticV1;

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
    fn with_access(
        property_id: impl Into<String>,
        label: impl Into<String>,
        value_kind: SchemaValueKindV1,
        value: Value,
        editable: bool,
    ) -> Self {
        Self {
            property_id: property_id.into(),
            label: label.into(),
            value_kind,
            value,
            editable,
            readonly: !editable,
            ..Self::default()
        }
    }

    #[inline]
    pub fn readonly(
        property_id: impl Into<String>,
        label: impl Into<String>,
        value_kind: SchemaValueKindV1,
        value: Value,
    ) -> Self {
        Self::with_access(property_id, label, value_kind, value, false)
    }

    #[inline]
    pub fn editable(
        property_id: impl Into<String>,
        label: impl Into<String>,
        value_kind: SchemaValueKindV1,
        value: Value,
    ) -> Self {
        Self::with_access(property_id, label, value_kind, value, true)
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
