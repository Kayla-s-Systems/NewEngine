use std::sync::atomic::Ordering;

use super::super::state::{ctx, EventSinkEntry, GatewayProviderRouteEntry, ServiceEntry};

#[derive(Clone)]
pub(crate) struct ProviderPublicationSnapshot {
    pub(super) descriptor: Option<newengine_plugin_api::PluginDescriptor>,
    pub(super) descriptor_v2: Option<newengine_plugin_api::PluginDescriptorV2>,
    pub(super) origin: Option<crate::service_gateway::GatewayProviderOrigin>,
    pub(super) contracts: Vec<newengine_runtime_contract_catalog::RuntimeContractSpec>,
    pub(super) services: Vec<(String, ServiceEntry)>,
    pub(super) event_sinks: Vec<EventSinkEntry>,
    pub(super) gateway_routes: Vec<(String, GatewayProviderRouteEntry)>,
}

/// Captures one provider's currently published topology under the same lock order
/// used by transactional commit. The snapshot is suitable for rollback only; it is
/// intentionally not exposed as a public diagnostics API.
pub(crate) fn snapshot_provider_publication(owner_plugin_id: &str) -> ProviderPublicationSnapshot {
    let c = ctx();
    let services = match c.services.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let descriptors = match c.plugin_descriptors.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let descriptors_v2 = match c.plugin_descriptors_v2.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let origins = match c.plugin_origins.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let routes = match c.gateway_provider_routes.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let event_sinks = match c.event_sinks.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let contract_catalog = match c.runtime_contract_catalog.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };

    ProviderPublicationSnapshot {
        descriptor: descriptors.get(owner_plugin_id).cloned(),
        descriptor_v2: descriptors_v2.get(owner_plugin_id).cloned(),
        origin: origins.get(owner_plugin_id).copied(),
        contracts: contract_catalog.contracts_by_owner(owner_plugin_id),
        services: services
            .iter()
            .filter(|(_, entry)| entry.owner_plugin_id.as_deref() == Some(owner_plugin_id))
            .map(|(id, entry)| (id.clone(), entry.clone()))
            .collect(),
        event_sinks: event_sinks
            .iter()
            .filter(|entry| entry.owner_plugin_id.as_deref() == Some(owner_plugin_id))
            .cloned()
            .collect(),
        gateway_routes: routes
            .iter()
            .filter(|(_, route)| route.provider_owner_id == owner_plugin_id)
            .map(|(key, route)| (key.clone(), route.clone()))
            .collect(),
    }
}

/// Restores a provider publication after a replacement failed after commit. The
/// currently published same-owner services are quiesced first, and the old service
/// leases are reopened only while the topology generation is odd.
pub(crate) fn restore_provider_publication(
    owner_plugin_id: &str,
    snapshot: ProviderPublicationSnapshot,
) {
    let c = ctx();
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

    let generation_before = c.services_generation.fetch_add(1, Ordering::AcqRel);
    debug_assert_eq!(
        generation_before & 1,
        0,
        "topology generation must be stable before rollback"
    );

    for entry in services.values() {
        if entry.owner_plugin_id.as_deref() == Some(owner_plugin_id) {
            entry.lifecycle.quiesce();
        }
    }
    services.retain(|_, entry| entry.owner_plugin_id.as_deref() != Some(owner_plugin_id));
    for (service_id, entry) in snapshot.services {
        entry.lifecycle.resume();
        services.insert(service_id, entry);
    }

    match snapshot.descriptor {
        Some(descriptor) => {
            descriptors.insert(owner_plugin_id.to_owned(), descriptor);
        }
        None => {
            descriptors.remove(owner_plugin_id);
        }
    }
    match snapshot.descriptor_v2 {
        Some(descriptor) => {
            descriptors_v2.insert(owner_plugin_id.to_owned(), descriptor);
        }
        None => {
            descriptors_v2.remove(owner_plugin_id);
        }
    }
    match snapshot.origin {
        Some(origin) => {
            origins.insert(owner_plugin_id.to_owned(), origin);
        }
        None => {
            origins.remove(owner_plugin_id);
        }
    }

    routes.retain(|_, route| route.provider_owner_id != owner_plugin_id);
    for (key, route) in snapshot.gateway_routes {
        routes.insert(key, route);
    }

    for entry in event_sinks.iter() {
        if entry.owner_plugin_id.as_deref() == Some(owner_plugin_id) {
            entry.lifecycle.quiesce();
        }
    }
    let mut next_sinks = event_sinks
        .iter()
        .filter(|entry| entry.owner_plugin_id.as_deref() != Some(owner_plugin_id))
        .cloned()
        .collect::<Vec<_>>();
    for entry in snapshot.event_sinks {
        entry.lifecycle.resume();
        next_sinks.push(entry);
    }
    *event_sinks = std::sync::Arc::from(next_sinks);

    contract_catalog.replace_plugin_contracts_after_validation(owner_plugin_id, snapshot.contracts);

    c.services_generation.fetch_add(1, Ordering::Release);
}
