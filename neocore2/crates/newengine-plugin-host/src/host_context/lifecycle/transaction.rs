use std::sync::atomic::Ordering;

use super::super::state::{
    ctx, current_plugin_id, EventSinkEntry, GatewayProviderRouteEntry, ProviderTransactionState,
    ServiceEntry,
};

/// A host/module composition transaction. Dropping an uncommitted transaction rolls
/// back all staged registrations, so an early-returning `Module::init()` cannot leak
/// partial routing state.
pub struct ProviderRegistrationTransaction {
    owner_id: String,
    finalized: bool,
}

impl ProviderRegistrationTransaction {
    pub fn begin_host(owner_id: impl Into<String>) -> Result<Self, String> {
        let owner_id = owner_id.into();
        begin_provider_transaction_mode(&owner_id, true)?;
        Ok(Self {
            owner_id,
            finalized: false,
        })
    }

    #[inline]
    pub fn validate(&self) -> Result<(), String> {
        validate_provider_transaction(&self.owner_id)
    }

    pub fn commit(mut self) -> Result<usize, String> {
        let committed = commit_provider_transaction(&self.owner_id)?;
        self.finalized = true;
        Ok(committed)
    }

    pub fn rollback(mut self) {
        rollback_provider_transaction(&self.owner_id);
        self.finalized = true;
    }
}

impl Drop for ProviderRegistrationTransaction {
    fn drop(&mut self) {
        if !self.finalized {
            rollback_provider_transaction(&self.owner_id);
        }
    }
}

fn begin_provider_transaction_mode(owner_id: &str, accepts_host_owned: bool) -> Result<(), String> {
    let owner_id = owner_id.trim();
    if owner_id.is_empty() {
        return Err("provider transaction owner is empty".to_owned());
    }
    let c = ctx();
    let mut tx = c
        .provider_transaction
        .lock()
        .map_err(|_| "provider transaction mutex poisoned".to_owned())?;
    if let Some(active) = tx.as_ref() {
        return Err(format!(
            "provider transaction already active owner='{}'",
            active.owner_plugin_id
        ));
    }
    *tx = Some(ProviderTransactionState {
        owner_plugin_id: owner_id.to_owned(),
        accepts_host_owned,
        ..ProviderTransactionState::default()
    });
    Ok(())
}

/// Starts an isolated dynamic-plugin publication transaction.
pub(crate) fn begin_provider_transaction(owner_plugin_id: &str) -> Result<(), String> {
    begin_provider_transaction_mode(owner_plugin_id, false)
}

/// Stages descriptor/origin metadata so `Module::init()` can validate its own
/// registrations without publishing the provider into the active topology.
pub(crate) fn stage_plugin_descriptor_registration(
    owner_plugin_id: &str,
    descriptor: newengine_plugin_api::PluginDescriptor,
    descriptor_v2: Option<newengine_plugin_api::PluginDescriptorV2>,
    origin: crate::service_gateway::GatewayProviderOrigin,
) -> Result<bool, String> {
    let c = ctx();
    let mut guard = c
        .provider_transaction
        .lock()
        .map_err(|_| "provider transaction mutex poisoned".to_owned())?;
    let Some(tx) = guard.as_mut() else {
        return Ok(false);
    };
    if tx.owner_plugin_id != owner_plugin_id {
        return Ok(false);
    }
    if descriptor.id.as_str() != owner_plugin_id {
        let error = format!(
            "provider transaction descriptor id mismatch owner='{}' descriptor='{}'",
            owner_plugin_id, descriptor.id
        );
        tx.staging_error = Some(error.clone());
        return Err(error);
    }
    let contracts =
        match newengine_runtime_contract_catalog::contracts_from_plugin_descriptor(&descriptor) {
            Ok(contracts) => contracts,
            Err(error) => {
                let error = format!(
                    "runtime contract declaration invalid owner='{}': {}",
                    owner_plugin_id, error
                );
                tx.staging_error = Some(error.clone());
                return Err(error);
            }
        };
    tx.staged_contracts = contracts;
    tx.staged_descriptor_v2 = Some(
        descriptor_v2
            .unwrap_or_else(|| newengine_plugin_api::PluginDescriptorV2::from_legacy(&descriptor)),
    );
    tx.staged_descriptor = Some(descriptor);
    tx.staged_origin = Some(origin);
    Ok(true)
}

/// Returns whether the staged descriptor declares one service. `None` means no
/// matching transaction/descriptor exists and the published descriptor should be used.
pub(crate) fn staged_plugin_declares_service(
    owner_plugin_id: &str,
    service_id: &str,
) -> Option<bool> {
    let c = ctx();
    let guard = c.provider_transaction.lock().ok()?;
    let tx = guard.as_ref()?;
    if tx.owner_plugin_id != owner_plugin_id {
        return None;
    }
    let descriptor = tx.staged_descriptor.as_ref()?;
    Some(descriptor.capabilities.iter().any(|cap| {
        cap.role == newengine_plugin_api::CapabilityRole::Provides
            && cap.kind == newengine_plugin_api::CapabilityKind::ServiceV1
            && cap.id.as_str() == service_id
    }))
}

/// Stages a service if the current provider is inside an init/reload transaction.
/// Returns `Ok(false)` when no matching transaction is active.
pub(crate) fn stage_service_registration(
    service_id: String,
    entry: ServiceEntry,
) -> Result<bool, String> {
    let c = ctx();
    let mut guard = c
        .provider_transaction
        .lock()
        .map_err(|_| "provider transaction mutex poisoned".to_owned())?;
    let Some(tx) = guard.as_mut() else {
        return Ok(false);
    };
    let plugin_owned_match = entry.owner_plugin_id.as_deref() == Some(tx.owner_plugin_id.as_str());
    let host_owned_match =
        tx.accepts_host_owned && entry.owner_plugin_id.is_none() && current_plugin_id().is_none();
    if !plugin_owned_match && !host_owned_match {
        return Ok(false);
    }
    if tx.staged_services.contains_key(&service_id) {
        return Err(format!(
            "service staged twice in provider transaction: {service_id}"
        ));
    }
    tx.staged_services.insert(service_id, entry);
    Ok(true)
}

/// Event subscriptions created by a provider during init are invisible until commit.
pub(crate) fn stage_event_sink_registration(entry: EventSinkEntry) -> Result<bool, String> {
    let c = ctx();
    let mut guard = c
        .provider_transaction
        .lock()
        .map_err(|_| "provider transaction mutex poisoned".to_owned())?;
    let Some(tx) = guard.as_mut() else {
        return Ok(false);
    };
    let plugin_owned_match = entry.owner_plugin_id.as_deref() == Some(tx.owner_plugin_id.as_str());
    let host_owned_match =
        tx.accepts_host_owned && entry.owner_plugin_id.is_none() && current_plugin_id().is_none();
    if !plugin_owned_match && !host_owned_match {
        return Ok(false);
    }
    tx.staged_event_sinks.push(entry);
    Ok(true)
}

/// Explicit provider routes published from a plugin callback are staged together
/// with its service. Host-owned runtime routes outside plugin callbacks remain direct.
pub(crate) fn stage_gateway_route_registration(
    key: String,
    mut entry: GatewayProviderRouteEntry,
) -> Result<bool, String> {
    let current_owner = current_plugin_id();
    let c = ctx();
    let mut guard = c
        .provider_transaction
        .lock()
        .map_err(|_| "provider transaction mutex poisoned".to_owned())?;
    let Some(tx) = guard.as_mut() else {
        return Ok(false);
    };
    let plugin_owned_match = current_owner.as_deref() == Some(tx.owner_plugin_id.as_str());
    let host_owned_match = tx.accepts_host_owned && current_owner.is_none();
    if !plugin_owned_match && !host_owned_match {
        return Ok(false);
    }
    if tx.staged_gateway_routes.contains_key(&key) {
        return Err(format!(
            "gateway route staged twice in provider transaction: {key}"
        ));
    }
    if let Some(origin) = tx.staged_origin {
        entry.origin = origin;
    }
    tx.staged_gateway_routes.insert(key, entry);
    Ok(true)
}

pub(super) fn validate_staged_route_contracts(
    tx: &ProviderTransactionState,
    contract_catalog: &newengine_runtime_contract_catalog::RuntimeContractCatalog,
) -> Result<(), String> {
    for route in tx.staged_gateway_routes.values() {
        let Some(provider_abi) = route.provider_abi.as_deref() else {
            continue;
        };
        let provider_abi = provider_abi.trim();
        if provider_abi.is_empty() {
            continue;
        }

        let staged = tx
            .staged_contracts
            .iter()
            .find(|spec| spec.advertised_id.as_deref() == Some(provider_abi));
        let published = contract_catalog
            .contract_by_advertised_id(provider_abi)
            .map(|entry| &entry.spec);
        let Some(contract) = staged.or(published) else {
            return Err(format!(
                "transactional route '{}' advertises unknown provider ABI '{}' owner='{}'; publish the ABI through Runtime Contract Catalog or use a normative Engine contract",
                route.provider_route_id, provider_abi, tx.owner_plugin_id
            ));
        };
        if contract.kind != newengine_runtime_contract_catalog::ContractKind::Abi {
            return Err(format!(
                "transactional route '{}' provider ABI '{}' resolves to contract '{}' kind='{}', expected kind='abi'",
                route.provider_route_id,
                provider_abi,
                contract.key,
                contract.kind.as_str()
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_provider_transaction(owner_plugin_id: &str) -> Result<(), String> {
    let c = ctx();
    let guard = c
        .provider_transaction
        .lock()
        .map_err(|_| "provider transaction mutex poisoned".to_owned())?;
    let tx = guard
        .as_ref()
        .ok_or_else(|| "no provider transaction is active".to_owned())?;
    if tx.owner_plugin_id != owner_plugin_id {
        return Err(format!(
            "provider transaction owner mismatch active='{}' requested='{}'",
            tx.owner_plugin_id, owner_plugin_id
        ));
    }
    if let Some(error) = tx.staging_error.as_ref() {
        return Err(error.clone());
    }

    if let Some(descriptor) = tx.staged_descriptor.as_ref() {
        if descriptor.id.as_str() != owner_plugin_id {
            return Err(format!(
                "provider transaction descriptor id mismatch owner='{}' descriptor='{}'",
                owner_plugin_id, descriptor.id
            ));
        }
        for service_id in tx.staged_services.keys() {
            let declared = descriptor.capabilities.iter().any(|cap| {
                cap.role == newengine_plugin_api::CapabilityRole::Provides
                    && cap.kind == newengine_plugin_api::CapabilityKind::ServiceV1
                    && cap.id.as_str() == service_id
            });
            if !declared {
                return Err(format!(
                    "transactional service is not declared by descriptor owner='{}' service='{}'",
                    owner_plugin_id, service_id
                ));
            }
        }
    }

    let services = c
        .services
        .lock()
        .map_err(|_| "services mutex poisoned".to_owned())?;
    for service_id in tx.staged_services.keys() {
        if let Some(existing) = services.get(service_id) {
            let replaceable_plugin_service = !tx.accepts_host_owned
                && existing.owner_plugin_id.as_deref() == Some(owner_plugin_id);
            if !replaceable_plugin_service {
                return Err(format!(
                    "transactional service collision service='{}' existing_owner='{}' contender='{}'",
                    service_id,
                    existing.owner_plugin_id.as_deref().unwrap_or("<host>"),
                    owner_plugin_id
                ));
            }
        }
    }
    for route in tx.staged_gateway_routes.values() {
        if !tx.accepts_host_owned && route.provider_owner_id != owner_plugin_id {
            return Err(format!(
                "transactional route owner mismatch route='{}' route_owner='{}' transaction_owner='{}'",
                route.provider_route_id, route.provider_owner_id, owner_plugin_id
            ));
        }
        let staged_service = tx.staged_services.contains_key(&route.provider_service_id);
        let existing_compatible = services
            .get(&route.provider_service_id)
            .is_some_and(|entry| {
                if tx.accepts_host_owned {
                    entry.owner_plugin_id.is_none()
                        || entry.owner_plugin_id.as_deref()
                            == Some(route.provider_owner_id.as_str())
                } else {
                    entry.owner_plugin_id.as_deref() == Some(owner_plugin_id)
                }
            });
        if !staged_service && !existing_compatible {
            return Err(format!(
                "transactional route '{}' references incompatible service '{}' transaction_owner='{}'",
                route.provider_route_id, route.provider_service_id, owner_plugin_id
            ));
        }
    }
    let contract_catalog = c
        .runtime_contract_catalog
        .lock()
        .map_err(|_| "runtime contract catalog mutex poisoned".to_owned())?;
    if !tx.accepts_host_owned {
        contract_catalog.validate_plugin_publication(owner_plugin_id, &tx.staged_contracts)?;
    }
    validate_staged_route_contracts(tx, &contract_catalog)?;
    Ok(())
}

/// Publishes descriptor, services, event sinks, and explicit gateway routes as one
/// topology epoch. Odd generations mean "commit in progress"; readers retry until
/// they observe the same even generation before and after their snapshot.
pub(crate) fn commit_provider_transaction(owner_plugin_id: &str) -> Result<usize, String> {
    validate_provider_transaction(owner_plugin_id)?;
    let c = ctx();
    let staged = {
        let mut guard = c
            .provider_transaction
            .lock()
            .map_err(|_| "provider transaction mutex poisoned".to_owned())?;
        let tx = guard
            .take()
            .ok_or_else(|| "provider transaction disappeared before commit".to_owned())?;
        if tx.owner_plugin_id != owner_plugin_id {
            return Err(format!(
                "provider transaction owner changed before commit active='{}' requested='{}'",
                tx.owner_plugin_id, owner_plugin_id
            ));
        }
        tx
    };

    if staged.staged_descriptor.is_none()
        && staged.staged_descriptor_v2.is_none()
        && staged.staged_origin.is_none()
        && staged.staged_contracts.is_empty()
        && staged.staged_services.is_empty()
        && staged.staged_event_sinks.is_empty()
        && staged.staged_gateway_routes.is_empty()
    {
        return Ok(0);
    }

    // Lock order follows the existing service -> descriptor -> origin ordering used
    // by capability resolution. No plugin/user callbacks execute while these guards exist.
    let mut services = match c.services.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let mut descriptors = match c.plugin_descriptors.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let mut descriptors_v2 = match c.plugin_descriptors_v2.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let mut origins = match c.plugin_origins.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let mut routes = match c.gateway_provider_routes.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let mut event_sinks = match c.event_sinks.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let mut contract_catalog = match c.runtime_contract_catalog.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    if !staged.accepts_host_owned {
        contract_catalog.validate_plugin_publication(owner_plugin_id, &staged.staged_contracts)?;
    }
    validate_staged_route_contracts(&staged, &contract_catalog)?;

    let generation_before = c.services_generation.fetch_add(1, Ordering::AcqRel);
    debug_assert_eq!(
        generation_before & 1,
        0,
        "topology generation must be stable before commit"
    );

    let accepts_host_owned = staged.accepts_host_owned;
    if !accepts_host_owned {
        contract_catalog
            .replace_plugin_contracts_after_validation(owner_plugin_id, staged.staged_contracts);
    }
    if !accepts_host_owned {
        for entry in services.values() {
            if entry.owner_plugin_id.as_deref() == Some(owner_plugin_id) {
                entry.lifecycle.quiesce();
            }
        }
        services.retain(|_, entry| entry.owner_plugin_id.as_deref() != Some(owner_plugin_id));
    }
    let committed_services = staged.staged_services.len();
    for (service_id, entry) in staged.staged_services {
        services.insert(service_id, entry);
    }

    if let Some(descriptor) = staged.staged_descriptor {
        descriptors.insert(owner_plugin_id.to_owned(), descriptor);
    }
    if let Some(descriptor) = staged.staged_descriptor_v2 {
        descriptors_v2.insert(owner_plugin_id.to_owned(), descriptor);
    }
    if let Some(origin) = staged.staged_origin {
        origins.insert(owner_plugin_id.to_owned(), origin);
    }

    if !accepts_host_owned {
        routes.retain(|_, route| route.provider_owner_id != owner_plugin_id);
    }
    for (key, route) in staged.staged_gateway_routes {
        routes.insert(key, route);
    }

    let mut next_sinks = if accepts_host_owned {
        event_sinks.iter().cloned().collect::<Vec<_>>()
    } else {
        for entry in event_sinks.iter() {
            if entry.owner_plugin_id.as_deref() == Some(owner_plugin_id) {
                entry.lifecycle.quiesce();
            }
        }
        event_sinks
            .iter()
            .filter(|entry| entry.owner_plugin_id.as_deref() != Some(owner_plugin_id))
            .cloned()
            .collect::<Vec<_>>()
    };
    next_sinks.extend(staged.staged_event_sinks);
    *event_sinks = std::sync::Arc::from(next_sinks);

    // Release publication only after every topology component is coherent.
    c.services_generation.fetch_add(1, Ordering::Release);
    Ok(committed_services)
}

pub(crate) fn rollback_provider_transaction(owner_plugin_id: &str) {
    let c = ctx();
    let mut tx = match c.provider_transaction.lock() {
        Ok(value) => value,
        Err(poisoned) => poisoned.into_inner(),
    };
    if tx
        .as_ref()
        .is_some_and(|active| active.owner_plugin_id == owner_plugin_id)
    {
        *tx = None;
    }
}
