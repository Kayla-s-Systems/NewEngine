#![forbid(unsafe_op_in_unsafe_fn)]

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

    /// Host-registered runtime plugins that live outside the normal ABI loader path
    /// (for example platform runtime and render backend runtime).
    pub(crate) external_runtime_plugins: Mutex<NeHashMap<String, ExternalRuntimePluginEntry>>,
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
        external_runtime_plugins: Mutex::new(NeHashMap::default()),
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
    let c = ctx();
    let g = match c.services.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    g.contains_key(service_id)
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
    out.sort();
    out
}

/// Returns the `describe()` JSON for the given service id, if present.
#[inline]
pub fn describe_service(service_id: &str) -> Option<String> {
    let c = ctx();
    let g = c.services.lock().ok()?;
    Some(g.get(service_id)?.describe_json.clone())
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

/// Unregisters all services/event sinks owned by the given plugin id.
///
/// Called by the plugin manager when a plugin is unloaded/disabled.
pub fn unregister_by_owner(owner_plugin_id: &str) {
    let c = ctx();

    {
        let mut g = match c.services.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };

        let before = g.len();
        g.retain(|_, ent| ent.owner_plugin_id.as_deref() != Some(owner_plugin_id));
        if g.len() != before {
            bump_services_generation();
        }
    }

    {
        let mut g = match c.event_sinks.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.retain(|ent| ent.owner_plugin_id.as_deref() != Some(owner_plugin_id));
    }

    // Also drop declared descriptor to keep metadata consistent with lifecycle.
    {
        let mut g = match c.plugin_descriptors.lock() {
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
}

/// Registers a plugin descriptor (host-owned metadata) for runtime validation.
///
/// Called by the plugin loader *before* `init()` so that service registrations during
/// init can be validated against declared capabilities.
pub(crate) fn register_plugin_descriptor(plugin_id: &str, d: PluginDescriptor) {
    let c = ctx();
    let mut g = match c.plugin_descriptors.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    g.insert(plugin_id.to_owned(), d);
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
) -> std::collections::HashMap<DeclaredCapKey, u32> {
    let mut out = std::collections::HashMap::new();
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
    }

    out
}

fn missing_descriptor_requirements(
    descriptor: &PluginDescriptor,
    providers: &std::collections::HashMap<DeclaredCapKey, u32>,
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

    {
        let mut descriptors = match c.plugin_descriptors.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        descriptors.insert(plugin_id.clone(), descriptor.clone());
    }

    {
        let mut runtimes = match c.external_runtime_plugins.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        runtimes.insert(
            plugin_id.clone(),
            ExternalRuntimePluginEntry {
                path: path.clone(),
                info: info.clone(),
                descriptor: descriptor.clone(),
                state: state.into(),
            },
        );
    }

    log::info!(
        "plugins: external runtime registered id='{}' ver='{}' kind={:?} path='{}'",
        plugin_id,
        info.version,
        descriptor.kind,
        path.display()
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