use std::collections::BTreeMap;

use newengine_schema_api::{
    SchemaBindingManifestV1, SchemaDefaultValueRequestV1, SchemaDefaultValueResponseV1,
    SchemaDescribePropertiesRequestV1, SchemaDescribePropertiesResponseV1,
    SchemaDescribeTypeRequestV1, SchemaDescribeTypeResponseV1, SchemaDiagnosticV1,
    SchemaPatchValidationRequestV1, SchemaPatchValidationResponseV1, SchemaTransactionDtoV1,
    SchemaTransactionResultV1, SchemaTypeDescriptorV1, ENGINE_SCHEMA_SERVICE_ID,
};
use serde::Serialize;
use serde_json::{json, Value};

use crate::config::load_embedded_registry;
use crate::validation::{binding_functions, deterministic_transaction_id, normalize_operation};

#[derive(Clone, Debug, Serialize)]
pub struct SchemaRuntimeServiceInfoV1 {
    pub schema: &'static str,
    pub id: &'static str,
    pub gateway: &'static str,
    pub provider_route: &'static str,
    pub backend_capability: &'static str,
    pub owner: &'static str,
    pub provider_kind: &'static str,
    pub replaceable: bool,
    pub methods: Vec<String>,
    pub type_count: usize,
    pub domains: Vec<String>,
    pub diagnostics: Vec<SchemaDiagnosticV1>,
}

#[derive(Clone, Debug)]
pub struct SchemaRegistryState {
    pub(crate) source_schema: String,
    pub(crate) policy: String,
    pub(crate) records: BTreeMap<String, SchemaTypeDescriptorV1>,
    pub(crate) registry_diagnostics: Vec<SchemaDiagnosticV1>,
    shutdown_count: u64,
}

impl Default for SchemaRegistryState {
    fn default() -> Self {
        let loaded = load_embedded_registry();
        Self {
            source_schema: loaded.source_schema,
            policy: loaded.policy,
            records: loaded.records,
            registry_diagnostics: loaded.diagnostics,
            shutdown_count: 0,
        }
    }
}

impl SchemaRegistryState {
    pub fn info(&self) -> SchemaRuntimeServiceInfoV1 {
        let mut domains = self.records.values().map(|record| record.domain.clone()).collect::<Vec<_>>();
        domains.sort();
        domains.dedup();
        SchemaRuntimeServiceInfoV1 {
            schema: newengine_schema_api::SCHEMA_RUNTIME_CONTRACT,
            id: newengine_schema_api::SCHEMA_SERVICE_ID,
            gateway: newengine_schema_api::ENGINE_SCHEMA_SERVICE_ID,
            provider_route: crate::PROVIDER_ROUTE,
            backend_capability: newengine_schema_api::SCHEMA_BACKEND_CAPABILITY_ID,
            owner: crate::OWNER,
            provider_kind: "core-owned-baseline-replaceable-provider",
            replaceable: true,
            methods: newengine_schema_api::SCHEMA_SERVICE_METHODS.iter().map(|method| (*method).to_owned()).collect(),
            type_count: self.records.len(),
            domains,
            diagnostics: self.registry_diagnostics.clone(),
        }
    }

    pub fn describe_type(&self, request: SchemaDescribeTypeRequestV1) -> SchemaDescribeTypeResponseV1 {
        let mut response = SchemaDescribeTypeResponseV1::default();
        let type_id = request.type_id.trim();
        if type_id.is_empty() {
            response.diagnostics.push(SchemaDiagnosticV1::error("SCHEMA_TYPE_ID_REQUIRED", "schema.describe_type_v1 requires type_id"));
            return response;
        }
        let Some(descriptor) = self.records.get(type_id) else {
            response.diagnostics.push(SchemaDiagnosticV1::error("SCHEMA_TYPE_NOT_FOUND", format!("schema type '{type_id}' is not registered")));
            return response;
        };
        let mut descriptor = descriptor.clone();
        descriptor.resource_ref = request.resource_ref;
        if !request.include_properties { descriptor.properties.clear(); }
        response.accepted = true;
        response.descriptor = Some(descriptor);
        response
    }

    pub fn describe_properties(&self, request: SchemaDescribePropertiesRequestV1) -> SchemaDescribePropertiesResponseV1 {
        let mut response = SchemaDescribePropertiesResponseV1::default();
        let type_id = request.type_id.trim();
        response.type_id = type_id.to_owned();
        if type_id.is_empty() {
            response.diagnostics.push(SchemaDiagnosticV1::error("SCHEMA_TYPE_ID_REQUIRED", "schema.describe_properties_v1 requires type_id"));
            return response;
        }
        let Some(descriptor) = self.records.get(type_id) else {
            response.diagnostics.push(SchemaDiagnosticV1::error("SCHEMA_TYPE_NOT_FOUND", format!("schema type '{type_id}' is not registered")));
            return response;
        };
        response.accepted = true;
        response.properties = descriptor.properties.clone();
        response
    }

    pub fn default_value(&self, request: SchemaDefaultValueRequestV1) -> SchemaDefaultValueResponseV1 {
        let mut response = SchemaDefaultValueResponseV1::default();
        let type_id = request.type_id.trim();
        let property_id = request.property_id.trim();
        response.type_id = type_id.to_owned();
        response.property_id = property_id.to_owned();
        match self.property(type_id, property_id) {
            Ok(property) => { response.accepted = true; response.value = property.default_value.clone(); }
            Err(diagnostic) => response.diagnostics.push(diagnostic),
        }
        response
    }

    pub fn validate_patch(&self, request: SchemaPatchValidationRequestV1) -> SchemaPatchValidationResponseV1 {
        let mut response = SchemaPatchValidationResponseV1::default();
        let mut patch = request.patch;
        let target_type = patch.target_type.trim().to_owned();
        let Some(descriptor) = self.records.get(&target_type) else {
            response.diagnostics.push(SchemaDiagnosticV1::error("SCHEMA_PATCH_TARGET_TYPE_NOT_FOUND", format!("patch target_type '{target_type}' is not registered")));
            return response;
        };
        if patch.transaction_id.trim().is_empty() { patch.transaction_id = deterministic_transaction_id(&patch); }
        if patch.requester.trim().is_empty() { patch.requester = "engine.schema".to_owned(); }
        if patch.operations.is_empty() {
            response.diagnostics.push(SchemaDiagnosticV1::error("SCHEMA_PATCH_EMPTY", "schema patch must contain at least one operation"));
            return response;
        }
        let mut normalized = Vec::with_capacity(patch.operations.len());
        let mut undo = Vec::with_capacity(patch.operations.len());
        let mut diagnostics = Vec::new();
        for operation in patch.operations.iter() {
            match normalize_operation(descriptor, operation) {
                Ok((op, undo_op)) => { normalized.push(op); undo.push(undo_op); }
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
        if diagnostics.iter().any(|d| d.severity == "error") { response.diagnostics = diagnostics; return response; }
        patch.operations = normalized;
        response.accepted = true;
        response.undo_operations = undo;
        response.normalized_patch = Some(patch);
        response.diagnostics = diagnostics;
        response
    }

    pub fn transaction_plan(&self, transaction: SchemaTransactionDtoV1) -> SchemaTransactionResultV1 {
        let mut response = SchemaTransactionResultV1::default();
        response.transaction_id = transaction.transaction_id.clone();
        if !self.records.contains_key(transaction.target_type.trim()) {
            response.diagnostics.push(SchemaDiagnosticV1::error("SCHEMA_TRANSACTION_TARGET_TYPE_NOT_FOUND", format!("transaction target_type '{}' is not registered", transaction.target_type)));
            return response;
        }
        if transaction.operations.is_empty() {
            response.diagnostics.push(SchemaDiagnosticV1::error("SCHEMA_TRANSACTION_EMPTY", "schema transaction must contain at least one operation"));
            return response;
        }
        response.accepted = true;
        response.committed = false;
        response.revision = transaction.base_revision;
        response.diagnostics.push(SchemaDiagnosticV1::info("SCHEMA_TRANSACTION_PLAN_READY", "transaction DTO is valid for editor undo/redo planning; applying the patch remains owned by the target domain"));
        response
    }

    pub fn binding_manifest_from_value(&self, request: Value) -> SchemaBindingManifestV1 {
        let target_language = request.get("target_language").and_then(Value::as_str).unwrap_or("generic").to_owned();
        let module_id = request.get("module_id").and_then(Value::as_str).unwrap_or("engine.schema.bindings").to_owned();
        let mut type_descriptors = self.records.values().cloned().collect::<Vec<_>>();
        type_descriptors.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        SchemaBindingManifestV1 {
            target_language,
            module_id,
            gateway: ENGINE_SCHEMA_SERVICE_ID.to_owned(),
            type_descriptors,
            functions: binding_functions(),
            diagnostics: self.registry_diagnostics.clone(),
            ..SchemaBindingManifestV1::default()
        }
    }

    pub fn dump_registry(&self) -> Value {
        json!({"schema": self.source_schema, "runtime_contract": newengine_schema_api::SCHEMA_RUNTIME_CONTRACT, "policy": self.policy, "gateway": newengine_schema_api::ENGINE_SCHEMA_SERVICE_ID, "provider_route": crate::PROVIDER_ROUTE, "replaceable": true, "records": self.records.values().cloned().collect::<Vec<_>>(), "diagnostics": self.registry_diagnostics.clone()})
    }

    pub fn shutdown(&mut self) -> Value {
        self.shutdown_count = self.shutdown_count.saturating_add(1);
        json!({"schema": "newengine.schema.shutdown.v1", "accepted": true, "idempotent": true, "shutdown_count": self.shutdown_count})
    }

    fn property(&self, type_id: &str, property_id: &str) -> Result<&newengine_schema_api::SchemaPropertyDescriptorV1, SchemaDiagnosticV1> {
        let Some(descriptor) = self.records.get(type_id.trim()) else {
            return Err(SchemaDiagnosticV1::error("SCHEMA_TYPE_NOT_FOUND", format!("schema type '{}' is not registered", type_id.trim())));
        };
        descriptor.properties.iter().find(|property| property.property_id == property_id.trim()).ok_or_else(|| {
            SchemaDiagnosticV1::error("SCHEMA_PROPERTY_NOT_FOUND", format!("property '{}' is not registered for type '{}'", property_id.trim(), type_id.trim()))
        })
    }
}
