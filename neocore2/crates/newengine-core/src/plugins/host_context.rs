#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RString;
#[cfg(feature = "runtime")]
use newengine_assets::AssetStore;
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
    CURRENT_PLUGIN_ID.with(|slot| {
        let prev = slot.replace(Some(plugin_id.to_string()));
        let out = f();
        *slot.borrow_mut() = prev;
        out
    })
}

#[inline]
pub(crate) fn current_plugin_id() -> Option<String> {
    CURRENT_PLUGIN_ID.with(|slot| slot.borrow().clone())
}

pub struct HostContext {
    pub services: Mutex<HashMap<String, ServiceEntry>>,
    #[cfg(feature = "runtime")]
    pub(crate) asset_store: Arc<AssetStore>,
    services_generation: AtomicU64,

    pub(crate) event_sinks: Mutex<Vec<EventSinkEntry>>,
}

static HOST_CTX: OnceLock<Arc<HostContext>> = OnceLock::new();

#[cfg(feature = "runtime")]
pub fn init_host_context(asset_store: Arc<AssetStore>) {
    let ctx = Arc::new(HostContext {
        services: Mutex::new(HashMap::new()),
        asset_store,
        services_generation: AtomicU64::new(1),
        event_sinks: Mutex::new(Vec::new()),
    });
    let _ = HOST_CTX.set(ctx);
}

#[cfg(not(feature = "runtime"))]
pub fn init_host_context() {
    let ctx = Arc::new(HostContext {
        services: Mutex::new(HashMap::new()),
        services_generation: AtomicU64::new(1),
        event_sinks: Mutex::new(Vec::new()),
    });
    let _ = HOST_CTX.set(ctx);
}

#[inline]
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

pub fn emit_plugin_event(topic: RString, payload: Blob) -> Result<(), String> {
    let c = ctx();
    let sinks = {
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
        guard.on_event(topic.clone(), payload.clone());
    }

    Ok(())
}

pub fn unregister_by_owner(plugin_id: &str) {
    let c = ctx();

    {
        let mut g = match c.services.lock() {
            Ok(v) => v,
            Err(_) => return,
        };
        let before = g.len();
        g.retain(|_, e| e.owner_plugin_id.as_deref() != Some(plugin_id));
        if g.len() != before {
            bump_services_generation();
        }
    }

    {
        let mut g = match c.event_sinks.lock() {
            Ok(v) => v,
            Err(_) => return,
        };
        g.retain(|e| e.owner_plugin_id.as_deref() != Some(plugin_id));
    }
}