pub use newengine_runtime_contract_catalog::{
    RuntimeContractAuthority, RuntimeContractEntry, RuntimeContractSpec,
};

use super::state::ctx;

/// Snapshot of the complete contract universe visible to the current Engine instance:
/// immutable normative engine contracts plus contracts published by loaded plugins.
pub fn list_runtime_contracts() -> Vec<RuntimeContractEntry> {
    let c = ctx();
    let catalog = match c.runtime_contract_catalog.lock() {
        Ok(value) => value,
        Err(poisoned) => poisoned.into_inner(),
    };
    catalog.list()
}

pub fn runtime_contract(key: &str) -> Option<RuntimeContractEntry> {
    let c = ctx();
    let catalog = match c.runtime_contract_catalog.lock() {
        Ok(value) => value,
        Err(poisoned) => poisoned.into_inner(),
    };
    catalog.contract(key).cloned()
}

pub fn runtime_contract_by_advertised_id(id: &str) -> Option<RuntimeContractEntry> {
    let c = ctx();
    let catalog = match c.runtime_contract_catalog.lock() {
        Ok(value) => value,
        Err(poisoned) => poisoned.into_inner(),
    };
    catalog.contract_by_advertised_id(id).cloned()
}
