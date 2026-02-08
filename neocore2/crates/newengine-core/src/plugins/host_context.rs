#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RString;
use newengine_plugin_api::{Blob, EventSinkV1Dyn, ServiceV1Dyn};

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone)]
pub struct ServiceEntry {
    pub owner_plugin_id: Option<String>,
    pub service: Arc<ServiceV1Dyn<'static>>,
    pub describe_json: String,
}

#[derive(Clone)]
pub struct EventSinkEntry {
    pub owner_plugin_id: Option<String>,
    pub sink: Arc<Mutex<EventSinkV1Dyn<'static>>>,
}

thread_local! {
    static CURRENT_PLUGIN_ID: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub(crate) fn with_current_plugin_id<R>(plugin_id: &str, f: impl FnOnce() -> R) -> R {
    CURRENT_PLUGIN_ID.with(|c| {
        let prev = c.replace(Some(plugin_id.to_owned()));
        let out = f();
        c.replace(prev);
        out
    })
}

pub(crate) fn current_plugin_id() -> Option<String> {
    CURRENT_PLUGIN_ID.with(|c| c.borrow().clone())
}

pub struct HostContext {
    pub services: Mutex<HashMap<String, ServiceEntry>>,
    services_generation: AtomicU64,

    pub(crate) event_sinks: Mutex<Vec<EventSinkEntry>>,
}

static HOST_CTX: OnceLock<Arc<HostContext>> = OnceLock::new();

/// Initializes global host context.
///
/// Core must not depend on concrete plugin-owned subsystems (assets/input/render/etc).
/// Plugins register services and event sinks via HostApi into this context.
pub fn init_host_context() {
    let ctx = Arc::new(HostContext {
        services: Mutex::new(HashMap::new()),
        services_generation: AtomicU64::new(1),
        event_sinks: Mutex::new(Vec::new()),
    });
    let _ = HOST_CTX.set(ctx);
}

pub fn ctx() -> Arc<HostContext> {
    HOST_CTX.get().expect("HostContext not initialized").clone()
}

#[inline]
pub fn services_generation() -> u64 {
    ctx().services_generation.load(Ordering::Acquire)
}

#[inline]
pub fn bump_services_generation() {
    ctx().services_generation.fetch_add(1, Ordering::AcqRel);
}

pub fn subscribe_event_sink(sink: EventSinkV1Dyn<'static>) -> Result<(), String> {
    let c = ctx();
    let mut g = c
        .event_sinks
        .lock()
        .map_err(|_| "event_sinks mutex poisoned".to_string())?;

    g.push(EventSinkEntry {
        owner_plugin_id: current_plugin_id(),
        sink: Arc::new(Mutex::new(sink)),
    });

    Ok(())
}

pub fn publish_event(topic: &str, payload: &[u8]) -> Result<(), String> {
    let c = ctx();

    let sinks: Vec<EventSinkEntry> = {
        let g = c
            .event_sinks
            .lock()
            .map_err(|_| "event_sinks mutex poisoned".to_string())?;
        g.clone()
    };

    for s in sinks {
        let mut guard = s
            .sink
            .lock()
            .map_err(|_| "event sink mutex poisoned".to_string())?;
        let _ = guard.on_event(RString::from(topic), Blob::from(payload.to_vec()));
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
            Err(_) => return,
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
            Err(_) => return,
        };
        g.retain(|ent| ent.owner_plugin_id.as_deref() != Some(owner_plugin_id));
    }
}