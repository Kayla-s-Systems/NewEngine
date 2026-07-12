use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    service::DEFAULT_SCHEMA_REQUESTER, SchemaDiagnosticV1, SchemaPropertyDescriptorV1,
    SchemaTypeDescriptorV1,
};

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
            requester: DEFAULT_SCHEMA_REQUESTER.to_owned(),
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
            requester: DEFAULT_SCHEMA_REQUESTER.to_owned(),
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
            requester: DEFAULT_SCHEMA_REQUESTER.to_owned(),
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
