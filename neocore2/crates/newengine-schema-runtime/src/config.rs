use std::collections::{BTreeMap, BTreeSet};

use newengine_schema_api::{
    SchemaDiagnosticV1, SchemaPropertyDescriptorV1, SchemaTypeDescriptorV1, SchemaValueKindV1,
};
use serde::Deserialize;
use serde_json::Value;

use crate::validation::default_value_for_kind;

pub(crate) const EMBEDDED_SCHEMA_REGISTRY: &str =
    include_str!("../../../config/schema/schema_registry.v1.json");

#[derive(Clone, Debug, Default)]
pub(crate) struct LoadedRegistry {
    pub(crate) source_schema: String,
    pub(crate) policy: String,
    pub(crate) records: BTreeMap<String, SchemaTypeDescriptorV1>,
    pub(crate) diagnostics: Vec<SchemaDiagnosticV1>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct SchemaRegistryFileV1 {
    #[serde(rename = "$schema")]
    schema: String,
    policy: String,
    records: Vec<SchemaRegistryRecordV1>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct SchemaRegistryRecordV1 {
    type_id: String,
    display_name: String,
    domain: String,
    kind: String,
    owner_gateway: String,
    source_contract: String,
    properties: Vec<SchemaPropertyDescriptorV1>,
    capabilities: Vec<String>,
    tags: Vec<String>,
    consumers: Vec<String>,
    patch_validation: String,
    transaction_dto: String,
    metadata: BTreeMap<String, Value>,
}

pub(crate) fn load_embedded_registry() -> LoadedRegistry {
    load_registry_json(EMBEDDED_SCHEMA_REGISTRY)
}

pub(crate) fn load_registry_json(text: &str) -> LoadedRegistry {
    let parsed = serde_json::from_str::<SchemaRegistryFileV1>(text);
    match parsed {
        Ok(file) => from_registry_file(file),
        Err(err) => LoadedRegistry {
            source_schema: "northstar.engine.schema_registry.v1".to_owned(),
            policy: "embedded registry failed to parse".to_owned(),
            records: BTreeMap::new(),
            diagnostics: vec![SchemaDiagnosticV1::error(
                "SCHEMA_REGISTRY_PARSE_FAILED",
                format!("embedded schema registry is invalid: {err}"),
            )],
        },
    }
}

fn from_registry_file(file: SchemaRegistryFileV1) -> LoadedRegistry {
    let mut diagnostics = Vec::new();
    let mut records = BTreeMap::new();
    for record in file.records {
        let type_id = record.type_id.trim().to_owned();
        if type_id.is_empty() {
            diagnostics.push(SchemaDiagnosticV1::error(
                "SCHEMA_RECORD_EMPTY_TYPE_ID",
                "schema registry record has an empty type_id",
            ));
            continue;
        }
        if records.contains_key(&type_id) {
            diagnostics.push(SchemaDiagnosticV1::error(
                "SCHEMA_RECORD_DUPLICATE_TYPE_ID",
                format!("duplicate schema type_id '{type_id}'"),
            ));
            continue;
        }
        records.insert(type_id, descriptor_from_record(record, &mut diagnostics));
    }
    LoadedRegistry {
        source_schema: if file.schema.trim().is_empty() {
            "northstar.engine.schema_registry.v1".to_owned()
        } else {
            file.schema
        },
        policy: file.policy,
        records,
        diagnostics,
    }
}

fn descriptor_from_record(
    record: SchemaRegistryRecordV1,
    diagnostics: &mut Vec<SchemaDiagnosticV1>,
) -> SchemaTypeDescriptorV1 {
    let type_id = record.type_id.trim().to_owned();
    let mut properties = Vec::with_capacity(record.properties.len());
    let mut seen = BTreeSet::new();
    for mut property in record.properties {
        let property_id = property.property_id.trim().to_owned();
        if property_id.is_empty() {
            diagnostics.push(SchemaDiagnosticV1::error(
                "SCHEMA_PROPERTY_EMPTY_ID",
                format!("type '{type_id}' has a property with empty property_id"),
            ));
            continue;
        }
        if !seen.insert(property_id.clone()) {
            diagnostics.push(SchemaDiagnosticV1::error(
                "SCHEMA_PROPERTY_DUPLICATE_ID",
                format!("type '{type_id}' declares duplicate property '{property_id}'"),
            ));
            continue;
        }
        property.property_id = property_id;
        if property.label.trim().is_empty() {
            property.label = title_case(&property.property_id);
        }
        if property.json_pointer.trim().is_empty() {
            property.json_pointer = format!("/{}", property.property_id);
        }
        property.source_domain = record.domain.clone();
        if property.default_value.is_null() && !property.nullable {
            property.default_value = default_value_for_kind(property.value_kind);
        }
        property.metadata.insert(
            "owner_gateway".to_owned(),
            Value::String(record.owner_gateway.clone()),
        );
        property.metadata.insert(
            "source_contract".to_owned(),
            Value::String(record.source_contract.clone()),
        );
        properties.push(property);
    }

    let mut metadata = record.metadata;
    metadata.insert(
        "owner_gateway".to_owned(),
        Value::String(record.owner_gateway),
    );
    metadata.insert(
        "source_contract".to_owned(),
        Value::String(record.source_contract),
    );
    metadata.insert(
        "patch_validation".to_owned(),
        Value::String(record.patch_validation),
    );
    metadata.insert(
        "transaction_dto".to_owned(),
        Value::String(record.transaction_dto),
    );
    metadata.insert(
        "consumers".to_owned(),
        Value::Array(record.consumers.into_iter().map(Value::String).collect()),
    );

    SchemaTypeDescriptorV1 {
        type_id: type_id.clone(),
        display_name: if record.display_name.trim().is_empty() {
            title_case(&type_id)
        } else {
            record.display_name
        },
        domain: record.domain,
        kind: if record.kind.trim().is_empty() {
            "resource".to_owned()
        } else {
            record.kind
        },
        properties,
        capabilities: record.capabilities,
        tags: record.tags,
        metadata,
        ..SchemaTypeDescriptorV1::default()
    }
}

fn title_case(value: &str) -> String {
    let last = value
        .rsplit('.')
        .next()
        .unwrap_or(value)
        .replace(['_', '-'], " ");
    let mut out = String::new();
    for word in last.split_whitespace() {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
            out.push(' ');
        }
    }
    out.trim().to_owned()
}

#[allow(dead_code)]
fn _kind_for_docs(kind: SchemaValueKindV1) -> &'static str {
    kind.as_str()
}
