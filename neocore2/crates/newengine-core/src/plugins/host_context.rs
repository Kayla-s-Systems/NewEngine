#![forbid(unsafe_op_in_unsafe_fn)]

use crate::plugins::{ServiceEntrySnapshot, ServiceRegistry};
use abi_stable::std_types::RString;
use newengine_assets::AssetStore;
use newengine_plugin_api::{Blob, EventSinkV1Dyn};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

pub struct HostContext {
    pub(crate) services: Mutex<ServiceRegistry>,
    pub(crate) asset_store: Arc<AssetStore>,

    /// Incremented whenever the service registry changes (register/unregister).
    services_generation: AtomicU64,

    /// Sinks registered by plugins via `HostApiV1::subscribe_events_v1`.
    ///
    /// NOTE: Stored as `Arc<Mutex<...>>` to satisfy `EventSinkV1`'s `&mut self` API while still being
    /// callable from different engine threads. If you want per-thread sinks later, split by thread-id.
    pub(crate) event_sinks: Mutex<Vec<Arc<Mutex<EventSinkV1Dyn<'static>>>>>,
}

static HOST_CTX: OnceLock<Arc<HostContext>> = OnceLock::new();

pub fn init_host_context(asset_store: Arc<AssetStore>) {
    let ctx = Arc::new(HostContext {
        services: Mutex::new(ServiceRegistry::new()),
        asset_store,
        services_generation: AtomicU64::new(1),
        event_sinks: Mutex::new(Vec::new()),
    });
    let _ = HOST_CTX.set(ctx);
}

#[inline]
pub(crate) fn ctx() -> Arc<HostContext> {
    HOST_CTX
        .get()
        .expect("HostContext not initialized (call init_host_context first)")
        .clone()
}

#[inline]
pub(crate) fn services_generation() -> u64 {
    ctx().services_generation.load(Ordering::Acquire)
}

#[inline]
pub(crate) fn bump_services_generation() {
    ctx().services_generation.fetch_add(1, Ordering::AcqRel);
}

pub(crate) fn services_snapshot() -> Vec<ServiceEntrySnapshot> {
    let c = ctx();
    let reg = match c.services.lock() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    reg.snapshot()
}

pub(crate) fn asset_store_arc() -> Arc<AssetStore> {
    ctx().asset_store.clone()
}

pub(crate) fn subscribe_events_v1(sink: EventSinkV1Dyn<'static>) -> Result<(), String> {
    let c = ctx();
    let mut g = c
        .event_sinks
        .lock()
        .map_err(|_| "event_sinks mutex poisoned".to_string())?;

    g.push(Arc::new(Mutex::new(sink)));
    Ok(())
}

pub(crate) fn emit_event_v1(topic: &str, payload: Blob) {
    // Clone sink list once to keep lock hold times low.
    let sinks: Vec<Arc<Mutex<EventSinkV1Dyn<'static>>>> = {
        let c = ctx();
        let g = match c.event_sinks.lock() {
            Ok(v) => v,
            Err(_) => return,
        };
        g.iter().cloned().collect()
    };

    if sinks.is_empty() {
        return;
    }

    let topic = RString::from(topic);
    for sink in sinks {
        let mut s = match sink.lock() {
            Ok(v) => v,
            Err(_) => continue,
        };
        s.on_event(topic.clone(), payload.clone());
    }
}
