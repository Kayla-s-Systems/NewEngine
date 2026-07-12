use newengine_schema_api::{SchemaBindingManifestV1, ENGINE_SCHEMA_SERVICE_ID};
use serde_json::{json, Value};

use crate::validation::binding_functions;

use super::SchemaRegistryState;

impl SchemaRegistryState {
    pub fn binding_manifest_from_value(&self, request: Value) -> SchemaBindingManifestV1 {
        let target_language = request
            .get("target_language")
            .and_then(Value::as_str)
            .unwrap_or("generic")
            .to_owned();
        let module_id = request
            .get("module_id")
            .and_then(Value::as_str)
            .unwrap_or("engine.schema.bindings")
            .to_owned();
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
        json!({
            "schema": self.source_schema,
            "runtime_contract": newengine_schema_api::SCHEMA_RUNTIME_CONTRACT,
            "policy": self.policy,
            "gateway": newengine_schema_api::ENGINE_SCHEMA_SERVICE_ID,
            "provider_route": crate::PROVIDER_ROUTE,
            "replaceable": true,
            "records": self.records.values().cloned().collect::<Vec<_>>(),
            "diagnostics": self.registry_diagnostics,
        })
    }

    pub fn shutdown(&mut self) -> Value {
        self.shutdown_count = self.shutdown_count.saturating_add(1);
        json!({
            "schema": "newengine.schema.shutdown.v1",
            "accepted": true,
            "idempotent": true,
            "shutdown_count": self.shutdown_count,
        })
    }
}
