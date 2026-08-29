use std::collections::BTreeMap;

use newengine_schema_api::{SchemaDiagnosticV1, SchemaTypeDescriptorV1};
use serde::Serialize;

use crate::config::load_embedded_registry;

mod bindings;
mod info;
mod mutation;
mod query;

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
    pub(crate) shutdown_count: u64,
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
