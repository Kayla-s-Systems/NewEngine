use abi_stable::std_types::RString;
use newengine_plugin_api::Blob;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::Ordering;

use super::super::state::{ctx, with_current_plugin_id, ServiceEntry};
use super::publication::ProviderPublicationSnapshot;
use super::transaction::rollback_provider_transaction;

/// Takes strong references to the currently published services owned by one provider.
/// Replacement/hot-reload uses this before commit so the old ABI objects remain valid
/// until their calls have drained and shutdown has completed.
pub(crate) fn snapshot_services_by_owner(owner_plugin_id: &str) -> Vec<ServiceEntry> {
    let c = ctx();
    let g = match c.services.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    g.values()
        .filter(|entry| entry.owner_plugin_id.as_deref() == Some(owner_plugin_id))
        .cloned()
        .collect()
}

pub(crate) fn quiesce_provider_publication(snapshot: &ProviderPublicationSnapshot) {
    for (_, entry) in &snapshot.services {
        entry.lifecycle.quiesce();
    }
    for entry in &snapshot.event_sinks {
        entry.lifecycle.quiesce();
    }
}

pub(crate) fn shutdown_provider_publication_services(
    owner_plugin_id: &str,
    snapshot: &ProviderPublicationSnapshot,
    reason: &str,
) {
    let entries = snapshot
        .services
        .iter()
        .map(|(_, entry)| entry.clone())
        .collect::<Vec<_>>();
    shutdown_service_entries(owner_plugin_id, &entries, reason);
}

pub(crate) fn wait_for_provider_publication_quiescence(
    snapshot: &ProviderPublicationSnapshot,
    timeout: std::time::Duration,
) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let services_idle = snapshot
            .services
            .iter()
            .all(|(_, entry)| entry.lifecycle.active_calls() == 0);
        let sinks_idle = snapshot
            .event_sinks
            .iter()
            .all(|entry| entry.lifecycle.active_calls() == 0);
        if services_idle && sinks_idle {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::yield_now();
    }
}

fn shutdown_service_entries(owner_plugin_id: &str, entries: &[ServiceEntry], reason: &str) {
    if entries.is_empty() {
        return;
    }

    for entry in entries {
        let service_id = entry.service.id().to_string();
        let svc = entry.service.clone();
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_current_plugin_id(owner_plugin_id, || {
                svc.call(
                    RString::from(newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1),
                    Blob::from(Vec::new()),
                )
            })
        }));
        match result {
            Ok(abi_stable::std_types::RResult::ROk(_)) => {}
            Ok(abi_stable::std_types::RResult::RErr(err)) => {
                let err = err.to_string();
                if !err.contains("unknown method") {
                    newengine_ulog_api::ulog::warn!(
                        "plugins shutdown: service shutdown_v1 failed owner='{}' service='{}' reason='{}' err='{}'",
                        owner_plugin_id,
                        service_id,
                        reason,
                        err
                    );
                }
            }
            Err(_) => {
                newengine_ulog_api::ulog::error!(
                    "plugins shutdown: service shutdown_v1 panicked owner='{}' service='{}' reason='{}'",
                    owner_plugin_id,
                    service_id,
                    reason
                );
            }
        }
    }
}

/// Best-effort explicit service shutdown for all services owned by a plugin.
pub fn shutdown_services_by_owner(owner_plugin_id: &str, reason: &str) {
    let entries = snapshot_services_by_owner(owner_plugin_id);
    shutdown_service_entries(owner_plugin_id, &entries, reason);
}

/// Unregisters all topology owned by the given plugin id as one atomic epoch.
pub fn unregister_by_owner(owner_plugin_id: &str) {
    rollback_provider_transaction(owner_plugin_id);
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
        "topology generation must be stable before unregister"
    );

    let removed_services = services
        .values()
        .filter(|entry| entry.owner_plugin_id.as_deref() == Some(owner_plugin_id))
        .count();
    for entry in services.values() {
        if entry.owner_plugin_id.as_deref() == Some(owner_plugin_id) {
            entry.lifecycle.quiesce();
        }
    }
    services.retain(|_, entry| entry.owner_plugin_id.as_deref() != Some(owner_plugin_id));

    let removed_sinks = event_sinks
        .iter()
        .filter(|entry| entry.owner_plugin_id.as_deref() == Some(owner_plugin_id))
        .count();
    for entry in event_sinks.iter() {
        if entry.owner_plugin_id.as_deref() == Some(owner_plugin_id) {
            entry.lifecycle.quiesce();
        }
    }
    let retained_sinks = event_sinks
        .iter()
        .filter(|entry| entry.owner_plugin_id.as_deref() != Some(owner_plugin_id))
        .cloned()
        .collect::<Vec<_>>();
    *event_sinks = std::sync::Arc::from(retained_sinks);

    descriptors.remove(owner_plugin_id);
    descriptors_v2.remove(owner_plugin_id);
    origins.remove(owner_plugin_id);
    routes.retain(|_, route| route.provider_owner_id != owner_plugin_id);
    let removed_contracts = contract_catalog.remove_plugin_contracts(owner_plugin_id);

    c.services_generation.fetch_add(1, Ordering::Release);
    drop(contract_catalog);
    drop(event_sinks);
    drop(routes);
    drop(origins);
    drop(descriptors_v2);
    drop(descriptors);
    drop(services);

    {
        let mut external = match c.external_runtime_plugins.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        external.remove(owner_plugin_id);
    }

    if removed_services > 0 || removed_sinks > 0 || removed_contracts > 0 {
        newengine_ulog_api::ulog::info!(
            "plugins shutdown: unregister committed owner='{}' services={} event_sinks={} contracts={}",
            owner_plugin_id,
            removed_services,
            removed_sinks,
            removed_contracts
        );
    }
}
