use super::{SchemaRegistryState, SchemaRuntimeServiceInfoV1};

impl SchemaRegistryState {
    pub fn info(&self) -> SchemaRuntimeServiceInfoV1 {
        let mut domains = self
            .records
            .values()
            .map(|record| record.domain.clone())
            .collect::<Vec<_>>();
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
            methods: newengine_schema_api::SCHEMA_SERVICE_METHODS
                .iter()
                .map(|method| (*method).to_owned())
                .collect(),
            type_count: self.records.len(),
            domains,
            diagnostics: self.registry_diagnostics.clone(),
        }
    }
}
