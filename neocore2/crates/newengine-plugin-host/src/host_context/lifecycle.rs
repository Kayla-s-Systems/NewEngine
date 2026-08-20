use abi_stable::std_types::RString;
use newengine_plugin_api::Blob;
use std::panic::{catch_unwind, AssertUnwindSafe};

use super::state::bump_services_generation;
use super::state::{ctx, with_current_plugin_id, ServiceEntry};

/// Best-effort explicit service shutdown for all services owned by a plugin.
///
/// This runs before service unregister/drop so plugin-owned runtime systems can
/// close native resources while their DLL vtables are still resident.
pub fn shutdown_services_by_owner(owner_plugin_id: &str, reason: &str) {
    let c = ctx();
    let entries: Vec<(String, ServiceEntry)> = {
        let g = match c.services.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.iter()
            .filter(|(_, ent)| ent.owner_plugin_id.as_deref() == Some(owner_plugin_id))
            .map(|(id, ent)| (id.clone(), ent.clone()))
            .collect()
    };

    if entries.is_empty() {
        newengine_ulog_api::ulog::debug!(
            "plugins shutdown: no services owned by plugin id='{}' reason='{}'",
            owner_plugin_id,
            reason
        );
        return;
    }

    newengine_ulog_api::ulog::info!(
        "plugins shutdown: service shutdown begin owner='{}' count={} reason='{}'",
        owner_plugin_id,
        entries.len(),
        reason
    );

    for (service_id, entry) in entries {
        newengine_ulog_api::ulog::info!(
            "plugins shutdown: service shutdown begin owner='{}' service='{}' method='{}'",
            owner_plugin_id,
            service_id,
            newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1
        );

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
            Ok(abi_stable::std_types::RResult::ROk(_)) => {
                newengine_ulog_api::ulog::info!(
                    "plugins shutdown: service shutdown complete owner='{}' service='{}'",
                    owner_plugin_id,
                    service_id
                );
            }
            Ok(abi_stable::std_types::RResult::RErr(err)) => {
                let err = err.to_string();
                if err.contains("unknown method") || err.contains("unknown method:") {
                    newengine_ulog_api::ulog::debug!(
                        "plugins shutdown: service has no shutdown_v1 owner='{}' service='{}' err='{}'",
                        owner_plugin_id,
                        service_id,
                        err
                    );
                } else {
                    newengine_ulog_api::ulog::warn!(
                        "plugins shutdown: service shutdown_v1 failed owner='{}' service='{}' err='{}'",
                        owner_plugin_id,
                        service_id,
                        err
                    );
                }
            }
            Err(_) => {
                newengine_ulog_api::ulog::error!(
                    "plugins shutdown: service shutdown_v1 panicked owner='{}' service='{}'",
                    owner_plugin_id,
                    service_id
                );
            }
        }
    }

    newengine_ulog_api::ulog::info!(
        "plugins shutdown: service shutdown complete owner='{}' reason='{}'",
        owner_plugin_id,
        reason
    );
}

/// Unregisters all services/event sinks owned by the given plugin id.
///
/// Called by the plugin manager when a plugin is unloaded/disabled.
pub fn unregister_by_owner(owner_plugin_id: &str) {
    let c = ctx();

    let removed_service_ids = {
        let mut g = match c.services.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };

        let before = g.len();
        let mut owned = g
            .iter()
            .filter(|(_, ent)| ent.owner_plugin_id.as_deref() == Some(owner_plugin_id))
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        owned.sort();
        g.retain(|_, ent| ent.owner_plugin_id.as_deref() != Some(owner_plugin_id));
        if g.len() != before {
            bump_services_generation();
        }
        owned
    };

    let removed_services = removed_service_ids.len();
    if removed_services > 0 {
        newengine_ulog_api::ulog::info!(
            "plugins shutdown: service unregister owner='{}' count={} services='{}'",
            owner_plugin_id,
            removed_services,
            removed_service_ids.join(",")
        );
    }

    // Event publishing uses an Arc snapshot, so unloading only rebuilds the snapshot
    // once instead of forcing every event publication to clone the full registry.
    let removed_sinks = {
        let mut g = match c.event_sinks.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        let before = g.len();
        let retained = g
            .iter()
            .filter(|ent| ent.owner_plugin_id.as_deref() != Some(owner_plugin_id))
            .cloned()
            .collect::<Vec<_>>();
        let removed = before.saturating_sub(retained.len());
        if removed > 0 {
            *g = std::sync::Arc::from(retained);
        }
        removed
    };

    // Also drop declared descriptor to keep metadata consistent with lifecycle.
    {
        let mut g = match c.plugin_descriptors.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.remove(owner_plugin_id);
    }

    {
        let mut g = match c.plugin_origins.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.remove(owner_plugin_id);
    }

    {
        let mut g = match c.external_runtime_plugins.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.remove(owner_plugin_id);
    }

    {
        let mut g = match c.gateway_provider_routes.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.retain(|_, route| route.provider_owner_id != owner_plugin_id);
    }

    bump_services_generation();

    if removed_services > 0 || removed_sinks > 0 {
        newengine_ulog_api::ulog::info!(
            "plugins shutdown: unregister complete owner='{}' services={} event_sinks={}",
            owner_plugin_id,
            removed_services,
            removed_sinks
        );
    }
}
