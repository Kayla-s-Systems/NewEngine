#![forbid(unsafe_op_in_unsafe_fn)]

use crate::path_fmt::canonicalize_if_exists;
use abi_stable::std_types::RString;
use newengine_plugin_api::{
    Blob, CapabilityKind, CapabilityRole, EventSinkV1Dyn, PluginDescriptor, PluginInfo, PluginKind, ServiceV1Dyn,
};

use newengine_math::collections::prelude::*;
use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone)]
pub(crate) struct ServiceEntry {
    pub owner_plugin_id: Option<String>,
    pub service: Arc<ServiceV1Dyn<'static>>,
    pub describe_json: String,
}

#[derive(Clone)]
pub(crate) struct EventSinkEntry {
    pub owner_plugin_id: Option<String>,
    pub sink: Arc<Mutex<EventSinkV1Dyn<'static>>>,
}

#[derive(Clone)]
pub(crate) struct ExternalRuntimePluginEntry {
    pub path: PathBuf,
    pub info: PluginInfo,
    pub descriptor: PluginDescriptor,
    pub state: String,
}

#[derive(Clone, Debug)]
pub struct ExternalRuntimePluginSnapshot {
    pub path: PathBuf,
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: Option<PluginKind>,
    pub capabilities: Vec<newengine_plugin_api::CapabilityDesc>,
    pub state: String,
    pub disabled_reason: Option<String>,
}

#[derive(Clone, Debug)]
pub struct EngineGatewayRouteSnapshot {
    pub gateway_id: String,
    pub service_kind: String,
    pub provider_service_id: String,
    pub provider_owner_id: String,
    pub backend_capability_id: String,
    pub backend_priority: i32,
    pub origin: String,
    pub override_mode: String,
    pub active_score: i64,
    pub active: bool,
}

#[derive(Clone, Debug)]
struct EngineOwnedGatewayEntry {
    gateway_id: String,
    service_kind: newengine_service_api::EngineServiceKind,
    provider_service_id: String,
    provider_owner_id: String,
    backend_capability_id: String,
    backend_priority: i32,
}

#[derive(Clone)]
struct GatewayRegistryCache {
    generation: u64,
    registry: crate::service_gateway::ActiveGatewayRegistry,
}

thread_local! {
    static CURRENT_PLUGIN_ID: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub(crate) fn with_current_plugin_id<R>(plugin_id: &str, f: impl FnOnce() -> R) -> R {
    CURRENT_PLUGIN_ID.with(|c| {
        let prev = c.replace(Some(plugin_id.to_owned()));

        struct Restore<'a> {
            cell: &'a RefCell<Option<String>>,
            prev: Option<String>,
        }

        impl<'a> Drop for Restore<'a> {
            fn drop(&mut self) {
                let _ = self.cell.replace(self.prev.take());
            }
        }

        let _restore = Restore { cell: c, prev };
        f()
    })
}

pub(crate) fn current_plugin_id() -> Option<String> {
    CURRENT_PLUGIN_ID.with(|c| (*c.borrow()).clone())
}

pub(crate) struct HostContext {
    pub(crate) services: Mutex<NeHashMap<String, ServiceEntry>>,
    services_generation: AtomicU64,

    pub(crate) event_sinks: Mutex<Vec<EventSinkEntry>>,

    /// Declared plugin descriptors keyed by plugin id.
    ///
    /// This is host-owned metadata used to validate runtime registrations (services/sinks)
    /// against the plugin's declared capabilities.
    pub(crate) plugin_descriptors: Mutex<NeHashMap<String, PluginDescriptor>>,

    /// Host-assigned provider origin keyed by plugin id. This is intentionally
    /// separate from descriptor JSON because trust tier must be assigned by the
    /// loader/profile layer, never by the plugin itself.
    pub(crate) plugin_origins: Mutex<NeHashMap<String, crate::service_gateway::GatewayProviderOrigin>>,

    /// Host-registered runtime plugins that live outside the normal ABI loader path
    /// (currently platform runtime units only).
    pub(crate) external_runtime_plugins: Mutex<NeHashMap<String, ExternalRuntimePluginEntry>>,

    /// Engine-owned routes for facade ids backed by host/runtime services rather
    /// than plugin descriptors. These entries participate in the same gateway
    /// registry and priority rules as plugin routes.
    engine_owned_gateways: Mutex<NeHashMap<String, EngineOwnedGatewayEntry>>,

    /// Cached active gateway registry. Routing is on the service hot path, so
    /// descriptor/fact folding must happen only when the gateway fact generation
    /// changes, not on every `call_service_v1(engine.*)` call.
    gateway_registry_cache: Mutex<Option<GatewayRegistryCache>>,
}

static HOST_CTX: OnceLock<Arc<HostContext>> = OnceLock::new();

/// Initializes global host context.
///
/// Core must not depend on concrete plugin-owned subsystems (assets/input/render/etc).
/// Plugins register services and event sinks via HostApi into this context.
fn make_default_ctx() -> Arc<HostContext> {
    Arc::new(HostContext {
        services: Mutex::new(NeHashMap::default()),
        services_generation: AtomicU64::new(1),
        event_sinks: Mutex::new(Vec::new()),
        plugin_descriptors: Mutex::new(NeHashMap::default()),
        plugin_origins: Mutex::new(NeHashMap::default()),
        external_runtime_plugins: Mutex::new(NeHashMap::default()),
        engine_owned_gateways: Mutex::new(NeHashMap::default()),
        gateway_registry_cache: Mutex::new(None),
    })
}

/// Initializes global host context.
///
/// Safe to call multiple times; after the first initialization it becomes a no-op.
///
/// Core must not depend on concrete plugin-owned subsystems (assets/input/render/etc).
/// Plugins register services and event sinks via HostApi into this context.
pub fn init_host_context() {
    let _ = HOST_CTX.set(make_default_ctx());
}

/// Returns the global host context.
///
/// This function never panics: if the context wasn't explicitly initialized yet,
/// it will be lazily created.
pub(crate) fn ctx() -> Arc<HostContext> {
    HOST_CTX.get_or_init(make_default_ctx).clone()
}

#[inline]
pub fn services_generation() -> u64 {
    ctx().services_generation.load(Ordering::Acquire)
}

#[inline]
pub fn bump_services_generation() {
    ctx().services_generation.fetch_add(1, Ordering::AcqRel);
}

/// Returns true if a plugin-owned service with the given id is currently registered.
#[inline]
pub fn has_service(service_id: &str) -> bool {
    let direct_registered = {
        let c = ctx();
        let g = match c.services.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.contains_key(service_id)
    };

    if direct_registered {
        return true;
    }

    resolve_service_for_engine_gateway(service_id).is_some()
}

/// Returns a stable, sorted list of registered plugin-owned service ids.
///
/// Intended for diagnostics and crash reports.
pub fn list_services() -> Vec<String> {
    let c = ctx();
    let g = match c.services.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };

    let mut out: Vec<String> = g.keys().cloned().collect();
    drop(g);

    out.extend(active_engine_gateways());
    out.sort();
    out.dedup();
    out
}

/// Returns the `describe()` JSON for the given service id, if present.
#[inline]
pub fn describe_service(service_id: &str) -> Option<String> {
    let routed_id = resolve_service_for_engine_gateway(service_id).unwrap_or_else(|| service_id.to_owned());

    let c = ctx();
    let g = c.services.lock().ok()?;
    Some(g.get(&routed_id)?.describe_json.clone())
}

fn build_gateway_registry_snapshot() -> crate::service_gateway::ActiveGatewayRegistry {
    let c = ctx();

    let services = {
        let services = match c.services.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        services
            .iter()
            .map(|(service_id, entry)| {
                crate::service_gateway::RegisteredServiceFact::new(
                    service_id.clone(),
                    entry.owner_plugin_id.clone(),
                )
            })
            .collect::<Vec<_>>()
    };

    let plugin_origins = {
        let origins = match c.plugin_origins.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        origins.clone()
    };

    let descriptors = {
        let descriptors = match c.plugin_descriptors.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        descriptors
            .iter()
            .map(|(plugin_id, descriptor)| {
                crate::service_gateway::PluginDescriptorFact::new(
                    plugin_id.clone(),
                    descriptor.clone(),
                    plugin_origins
                        .get(plugin_id)
                        .copied()
                        .unwrap_or(crate::service_gateway::GatewayProviderOrigin::GamePlugin),
                )
            })
            .collect::<Vec<_>>()
    };

    let engine_owned_gateways = {
        let gateways = match c.engine_owned_gateways.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        gateways
            .values()
            .map(|entry| {
                crate::service_gateway::EngineOwnedGatewayFact::new(
                    entry.gateway_id.clone(),
                    entry.service_kind,
                    entry.provider_service_id.clone(),
                    entry.provider_owner_id.clone(),
                    entry.backend_capability_id.clone(),
                    entry.backend_priority,
                )
            })
            .collect::<Vec<_>>()
    };

    crate::service_gateway::ActiveGatewayRegistry::from_facts(
        &descriptors,
        &services,
        &engine_owned_gateways,
    )
}

fn gateway_registry_snapshot() -> crate::service_gateway::ActiveGatewayRegistry {
    let c = ctx();

    loop {
        let generation_before = c.services_generation.load(Ordering::Acquire);

        {
            let cache = match c.gateway_registry_cache.lock() {
                Ok(v) => v,
                Err(e) => e.into_inner(),
            };
            if let Some(cache) = cache.as_ref() {
                if cache.generation == generation_before {
                    return cache.registry.clone();
                }
            }
        }

        let registry = build_gateway_registry_snapshot();
        let generation_after = c.services_generation.load(Ordering::Acquire);

        if generation_before != generation_after {
            continue;
        }

        {
            let mut cache = match c.gateway_registry_cache.lock() {
                Ok(v) => v,
                Err(e) => e.into_inner(),
            };
            *cache = Some(GatewayRegistryCache {
                generation: generation_after,
                registry: registry.clone(),
            });
        }

        return registry;
    }
}


fn active_engine_gateways() -> Vec<String> {
    gateway_registry_snapshot().gateway_ids()
}

/// Resolve a host-owned facade service gateway to the active registered provider
/// service declared by plugin metadata.
///
/// The lookup is purely descriptor-driven: if no loaded provider declares the
/// requested gateway id, resolution returns `None`. It does not branch on
/// concrete domains such as assets/render/physics/input.
pub fn resolve_service_for_engine_gateway(gateway_id: &str) -> Option<String> {
    gateway_registry_snapshot().resolve_gateway(gateway_id)
}

pub fn engine_gateway_has_capability(gateway_id: &str, capability_id: &str) -> bool {
    gateway_registry_snapshot().has_gateway_capability(gateway_id, capability_id)
}


pub fn list_engine_gateway_routes() -> Vec<EngineGatewayRouteSnapshot> {
    let registry = gateway_registry_snapshot();
    registry
        .routes()
        .iter()
        .map(|route| {
            let active = match registry.resolve_route(&route.gateway_id) {
                Some(active_route) => {
                    active_route.provider_service_id == route.provider_service_id
                        && active_route.provider_owner_id == route.provider_owner_id
                }
                None => false,
            };
            let override_mode: crate::service_gateway::GatewayOverrideMode = route.override_mode;
            EngineGatewayRouteSnapshot {
                gateway_id: route.gateway_id.clone(),
                service_kind: route.service_kind.as_str().to_owned(),
                provider_service_id: route.provider_service_id.clone(),
                provider_owner_id: route.provider_owner_id.clone(),
                backend_capability_id: route.backend_capability_id.clone(),
                backend_priority: route.backend_priority,
                origin: route.origin.as_str().to_owned(),
                override_mode: override_mode.as_str().to_owned(),
                active_score: route.active_score,
                active,
            }
        })
        .collect()
}

pub fn active_engine_gateway_route(gateway_id: &str) -> Option<EngineGatewayRouteSnapshot> {
    gateway_registry_snapshot()
        .resolve_route(gateway_id)
        .map(|route| {
            let override_mode: crate::service_gateway::GatewayOverrideMode = route.override_mode;
            EngineGatewayRouteSnapshot {
                gateway_id: route.gateway_id.clone(),
                service_kind: route.service_kind.as_str().to_owned(),
                provider_service_id: route.provider_service_id.clone(),
                provider_owner_id: route.provider_owner_id.clone(),
                backend_capability_id: route.backend_capability_id.clone(),
                backend_priority: route.backend_priority,
                origin: route.origin.as_str().to_owned(),
                override_mode: override_mode.as_str().to_owned(),
                active_score: route.active_score,
                active: true,
            }
        })
}

pub fn register_engine_owned_gateway(
    gateway_id: &str,
    service_kind: newengine_service_api::EngineServiceKind,
    provider_service_id: &str,
    backend_capability_id: &str,
    backend_priority: i32,
    provider_owner_id: &str,
) -> Result<(), String> {
    if !newengine_service_api::is_engine_service_gateway_id(gateway_id) {
        return Err(format!("engine-owned gateway id must start with 'engine.': {gateway_id}"));
    }
    if provider_service_id.trim().is_empty() {
        return Err("engine-owned gateway provider_service_id is empty".to_owned());
    }
    if backend_capability_id.trim().is_empty() {
        return Err("engine-owned gateway backend_capability_id is empty".to_owned());
    }

    let c = ctx();
    {
        let services = match c.services.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        match services.get(provider_service_id) {
            Some(entry) if entry.owner_plugin_id.is_none() => {}
            Some(entry) => {
                return Err(format!(
                    "engine-owned gateway '{}' cannot route to plugin-owned service '{}' owner='{}'",
                    gateway_id,
                    provider_service_id,
                    entry.owner_plugin_id.as_deref().unwrap_or("<unknown>")
                ));
            }
            None => {
                return Err(format!(
                    "engine-owned gateway '{}' cannot route to unregistered service '{}'",
                    gateway_id, provider_service_id
                ));
            }
        }
    }

    let key = format!("{}::{}", gateway_id, provider_service_id);
    let mut gateways = match c.engine_owned_gateways.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    gateways.insert(
        key,
        EngineOwnedGatewayEntry {
            gateway_id: gateway_id.to_owned(),
            service_kind,
            provider_service_id: provider_service_id.to_owned(),
            provider_owner_id: if provider_owner_id.trim().is_empty() {
                "engine".to_owned()
            } else {
                provider_owner_id.to_owned()
            },
            backend_capability_id: backend_capability_id.to_owned(),
            backend_priority,
        },
    );

    bump_services_generation();
    log::info!(
        "gateways: registered engine-owned route gateway='{}' service='{}' kind='{}' capability='{}' priority={} owner='{}'",
        gateway_id,
        provider_service_id,
        service_kind.as_str(),
        backend_capability_id,
        backend_priority,
        provider_owner_id
    );
    Ok(())
}

#[inline]
fn parse_backend_priority(json: &str) -> i64 {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("backend_priority").and_then(|x| x.as_i64()))
        .unwrap_or(0)
}

/// Resolve the active registered provider service for a backend capability.
///
/// This is the host-owned service gateway primitive: callers ask the engine for
/// a domain service, while the host selects the concrete provider service from
/// descriptor facts instead of requiring consumers to know provider ids.
pub fn resolve_service_for_backend_capability(capability_id: &str) -> Option<String> {
    let c = ctx();
    let services = match c.services.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let descriptors = match c.plugin_descriptors.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let plugin_origins = match c.plugin_origins.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };

    let mut candidates: Vec<(i64, i64, String, String)> = Vec::new();

    for (service_id, entry) in services.iter() {
        let Some(owner) = entry.owner_plugin_id.as_deref() else {
            continue;
        };
        let Some(descriptor) = descriptors.get(owner) else {
            continue;
        };

        let Some(backend_capability) = descriptor.capabilities.iter().find(|cap| {
            cap.role == CapabilityRole::Provides && cap.id.as_str() == capability_id
        }) else {
            continue;
        };

        let declares_registered_service = descriptor.capabilities.iter().any(|cap| {
            cap.role == CapabilityRole::Provides
                && cap.kind == CapabilityKind::ServiceV1
                && cap.id.as_str() == service_id
        });
        if !declares_registered_service {
            continue;
        }

        let backend_priority = parse_backend_priority(backend_capability.describe_json.as_str());
        let origin = plugin_origins
            .get(owner)
            .copied()
            .unwrap_or(crate::service_gateway::GatewayProviderOrigin::GamePlugin);
        candidates.push((
            origin.origin_bias() + backend_priority,
            backend_priority,
            service_id.clone(),
            owner.to_owned(),
        ));
    }

    candidates.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.cmp(&b.2))
            .then_with(|| a.3.cmp(&b.3))
    });

    candidates.into_iter().map(|(_, _, service_id, _)| service_id).next()
}

pub fn subscribe_event_sink(sink: EventSinkV1Dyn<'static>) -> Result<(), String> {
    let c = ctx();
    let mut g = match c.event_sinks.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };

    g.push(EventSinkEntry {
        owner_plugin_id: current_plugin_id(),
        sink: Arc::new(Mutex::new(sink)),
    });

    Ok(())
}

pub fn publish_event(topic: &str, payload: &[u8]) -> Result<(), String> {
    let c = ctx();

    let sinks: Vec<EventSinkEntry> = {
        let g = match c.event_sinks.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.clone()
    };

    // Avoid per-sink payload construction by cloning a single Vec.
    let payload_vec: Vec<u8> = payload.to_vec();

    let mut bad_owners: Vec<String> = Vec::new();

    for s in sinks {
        let owner = s.owner_plugin_id.clone();

        let mut guard = match s.sink.lock() {
            Ok(v) => v,
            Err(_) => {
                if let Some(pid) = owner {
                    log::error!(
                        "events: sink mutex poisoned; owner='{}' topic='{}' (auto-unregister)",
                        pid,
                        topic
                    );
                    bad_owners.push(pid);
                } else {
                    log::error!("events: sink mutex poisoned; owner=<host> topic='{}'", topic);
                }
                continue;
            }
        };

        let call = || {
            // Blob is consumed by on_event(); clone bytes per sink.
            let _ = guard.on_event(RString::from(topic), Blob::from(payload_vec.clone()));
        };

        let r = match owner.as_deref() {
            Some(pid) => catch_unwind(AssertUnwindSafe(|| with_current_plugin_id(pid, call))),
            None => catch_unwind(AssertUnwindSafe(call)),
        };

        if r.is_err() {
            if let Some(pid) = owner {
                log::error!(
                    "events: sink panicked; owner='{}' topic='{}' (auto-unregister)",
                    pid,
                    topic
                );
                bad_owners.push(pid);
            } else {
                log::error!("events: sink panicked; owner=<host> topic='{}'", topic);
            }
        }
    }

    if !bad_owners.is_empty() {
        bad_owners.sort();
        bad_owners.dedup();
        for pid in bad_owners {
            unregister_by_owner(&pid);
        }
    }

    Ok(())
}

/// Emits an event originating from a plugin (ABI surface: `HostApiV1.emit_event_v1`).
#[inline]
pub fn emit_plugin_event(topic: RString, payload: Blob) -> Result<(), String> {
    publish_event(topic.as_str(), payload.as_slice())
}


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
        log::debug!(
            "plugins shutdown: no services owned by plugin id='{}' reason='{}'",
            owner_plugin_id,
            reason
        );
        return;
    }

    log::info!(
        "plugins shutdown: service shutdown begin owner='{}' count={} reason='{}'",
        owner_plugin_id,
        entries.len(),
        reason
    );

    for (service_id, entry) in entries {
        log::info!(
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
                log::info!(
                    "plugins shutdown: service shutdown complete owner='{}' service='{}'",
                    owner_plugin_id,
                    service_id
                );
            }
            Ok(abi_stable::std_types::RResult::RErr(err)) => {
                let err = err.to_string();
                if err.contains("unknown method") || err.contains("unknown method:") {
                    log::debug!(
                        "plugins shutdown: service has no shutdown_v1 owner='{}' service='{}' err='{}'",
                        owner_plugin_id,
                        service_id,
                        err
                    );
                } else {
                    log::warn!(
                        "plugins shutdown: service shutdown_v1 failed owner='{}' service='{}' err='{}'",
                        owner_plugin_id,
                        service_id,
                        err
                    );
                }
            }
            Err(_) => {
                log::error!(
                    "plugins shutdown: service shutdown_v1 panicked owner='{}' service='{}'",
                    owner_plugin_id,
                    service_id
                );
            }
        }
    }

    log::info!(
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
        log::info!(
            "plugins shutdown: service unregister owner='{}' count={} services='{}'",
            owner_plugin_id,
            removed_services,
            removed_service_ids.join(",")
        );
    }

    let removed_sinks = {
        let mut g = match c.event_sinks.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        let before = g.len();
        g.retain(|ent| ent.owner_plugin_id.as_deref() != Some(owner_plugin_id));
        before.saturating_sub(g.len())
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
        let mut g = match c.engine_owned_gateways.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.retain(|_, route| route.provider_owner_id != owner_plugin_id);
    }

    bump_services_generation();

    if removed_services > 0 || removed_sinks > 0 {
        log::info!(
            "plugins shutdown: unregister complete owner='{}' services={} event_sinks={}",
            owner_plugin_id,
            removed_services,
            removed_sinks
        );
    }
}


#[inline]
fn effective_provider_origin(
    descriptor: &PluginDescriptor,
    default_origin: crate::service_gateway::GatewayProviderOrigin,
) -> crate::service_gateway::GatewayProviderOrigin {
    let id = descriptor.id.as_str().to_ascii_lowercase();
    if id.contains("null") {
        return crate::service_gateway::GatewayProviderOrigin::NullProvider;
    }

    for cap in descriptor.capabilities.iter() {
        if cap.role != CapabilityRole::Provides {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(cap.describe_json.as_str()) else {
            continue;
        };
        let backend_is_null = value
            .get("backend")
            .and_then(|v| v.as_str())
            .map(|v| v.eq_ignore_ascii_case("null"))
            .unwrap_or(false);
        let mode_is_headless = value
            .get("mode")
            .and_then(|v| v.as_str())
            .map(|v| v.eq_ignore_ascii_case("headless"))
            .unwrap_or(false);
        if backend_is_null || mode_is_headless {
            return crate::service_gateway::GatewayProviderOrigin::NullProvider;
        }
    }

    default_origin
}

/// Registers a plugin descriptor (host-owned metadata) for runtime validation.
///
/// Called by the plugin loader *before* `init()` so that service registrations during
/// init can be validated against declared capabilities.
pub(crate) fn register_plugin_descriptor(
    plugin_id: &str,
    d: PluginDescriptor,
    origin: crate::service_gateway::GatewayProviderOrigin,
) -> crate::service_gateway::GatewayProviderOrigin {
    let origin = effective_provider_origin(&d, origin);
    let c = ctx();
    {
        let mut g = match c.plugin_descriptors.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.insert(plugin_id.to_owned(), d);
    }

    {
        let mut g = match c.plugin_origins.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.insert(plugin_id.to_owned(), origin);
    }

    bump_services_generation();
    origin
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct DeclaredCapKey {
    id: String,
    kind: u8,
}

#[inline]
fn declared_cap_key(id: &str, kind: u8) -> DeclaredCapKey {
    DeclaredCapKey {
        id: id.to_owned(),
        kind,
    }
}

fn collect_declared_providers(
    descriptors: impl Iterator<Item=PluginDescriptor>,
) -> newengine_math::collections_prelude::NeHashMap<DeclaredCapKey, u32> {
    let mut out = newengine_math::collections_prelude::NeHashMap::default();
    out.insert(declared_cap_key("host.services.v1", CapabilityKind::ServiceV1 as u8), 1);
    out.insert(declared_cap_key("host.events.v1", CapabilityKind::EventsV1 as u8), 1);

    for d in descriptors {
        for c in d.capabilities.iter() {
            if c.role != CapabilityRole::Provides {
                continue;
            }
            let key = declared_cap_key(c.id.as_str(), c.kind as u8);
            let cur = out.get(&key).copied().unwrap_or(0);
            if c.version > cur {
                out.insert(key, c.version);
            }
        }

        for gateway in crate::service_gateway::descriptor_gateway_capabilities(&d) {
            let _service_kind = gateway.service_kind.as_str();
            if crate::service_gateway::gateway_provider_service_id(&d, &gateway).is_some() {
                let key = declared_cap_key(gateway.gateway_id.as_str(), CapabilityKind::ServiceV1 as u8);
                let cur = out.get(&key).copied().unwrap_or(0);
                if cur < 1 {
                    out.insert(key, 1);
                }
            }
        }
    }

    out
}

fn missing_descriptor_requirements(
    descriptor: &PluginDescriptor,
    providers: &newengine_math::collections_prelude::NeHashMap<DeclaredCapKey, u32>,
) -> Vec<String> {
    let mut out = Vec::new();

    for c in descriptor.capabilities.iter() {
        if c.role != CapabilityRole::Requires {
            continue;
        }

        let key = declared_cap_key(c.id.as_str(), c.kind as u8);
        let pv = providers.get(&key).copied().unwrap_or(0);
        if pv < c.version {
            out.push(format!(
                "{}(kind={} req_v={} avail_v={})",
                c.id,
                c.kind as u8,
                c.version,
                pv
            ));
        }
    }

    out.sort();
    out.dedup();
    out
}

pub fn register_external_runtime_plugin(
    path: PathBuf,
    info: PluginInfo,
    descriptor: PluginDescriptor,
    state: impl Into<String>,
) -> Result<(), String> {
    let plugin_id = info.id.to_string();
    if plugin_id.trim().is_empty() {
        return Err("external runtime plugin id is empty".to_owned());
    }

    let c = ctx();

    let providers = {
        let g = match c.plugin_descriptors.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        let mut descriptors: Vec<PluginDescriptor> = g.values().cloned().collect();
        descriptors.push(descriptor.clone());
        collect_declared_providers(descriptors.into_iter())
    };

    let missing = missing_descriptor_requirements(&descriptor, &providers);
    if !missing.is_empty() {
        return Err(format!(
            "missing required capability(s) for external runtime plugin id='{}': [{}]",
            plugin_id,
            missing.join(", ")
        ));
    }

    let normalized_path = canonicalize_if_exists(&path);
    let origin = effective_provider_origin(
        &descriptor,
        crate::service_gateway::GatewayProviderOrigin::from_plugin_path(&normalized_path),
    );

    {
        let mut descriptors = match c.plugin_descriptors.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        descriptors.insert(plugin_id.clone(), descriptor.clone());
    }

    {
        let mut origins = match c.plugin_origins.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        origins.insert(plugin_id.clone(), origin);
    }

    {
        let mut runtimes = match c.external_runtime_plugins.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        runtimes.insert(
            plugin_id.clone(),
            ExternalRuntimePluginEntry {
                path: normalized_path.clone(),
                info: info.clone(),
                descriptor: descriptor.clone(),
                state: state.into(),
            },
        );
    }

    bump_services_generation();

    log::info!(
        "plugins: external runtime registered id='{}' ver='{}' kind={:?} origin='{}' path='{}'",
        plugin_id,
        info.version,
        descriptor.kind,
        origin.as_str(),
        crate::path_fmt::display_clean(&normalized_path)
    );

    Ok(())
}

pub fn list_external_runtime_plugins() -> Vec<ExternalRuntimePluginSnapshot> {
    let c = ctx();
    let g = match c.external_runtime_plugins.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };

    let mut out: Vec<ExternalRuntimePluginSnapshot> = g
        .values()
        .map(|entry| ExternalRuntimePluginSnapshot {
            path: entry.path.clone(),
            id: entry.info.id.to_string(),
            name: entry.info.name.to_string(),
            version: entry.info.version.to_string(),
            kind: Some(entry.descriptor.kind),
            capabilities: entry.descriptor.capabilities.iter().cloned().collect(),
            state: entry.state.clone(),
            disabled_reason: None,
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

pub fn list_external_runtime_descriptors() -> Vec<PluginDescriptor> {
    let c = ctx();
    let g = match c.external_runtime_plugins.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let mut out: Vec<PluginDescriptor> = g.values().map(|entry| entry.descriptor.clone()).collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Returns:
/// - `Some(true)` if the plugin has a descriptor and declares `Provides(ServiceV1, service_id)`.
/// - `Some(false)` if the plugin has a descriptor but does not declare that capability.
/// - `None` if the plugin has no known descriptor (ABI v1 or loader did not register it).
pub(crate) fn plugin_declares_provided_service(
    plugin_id: &str,
    service_id: &str,
) -> Option<bool> {
    let c = ctx();
    let g = c.plugin_descriptors.lock().ok()?;
    let d = g.get(plugin_id)?;

    for cap in d.capabilities.iter() {
        if cap.role != CapabilityRole::Provides {
            continue;
        }
        if cap.kind != CapabilityKind::ServiceV1 {
            continue;
        }
        if cap.id.as_str() == service_id {
            return Some(true);
        }
    }

    Some(false)
}