#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RString;
use newengine_assets::AssetStore;
use newengine_plugin_api::{Blob, EventSinkV1Dyn, ServiceV1Dyn};

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

pub struct HostContext {
    pub services: Mutex<HashMap<String, Arc<ServiceV1Dyn<'static>>>>,
    pub(crate) asset_store: Arc<AssetStore>,
    services_generation: AtomicU64,

    pub(crate) event_sinks: Mutex<Vec<Arc<Mutex<EventSinkV1Dyn<'static>>>>>,
}

static HOST_CTX: OnceLock<Arc<HostContext>> = OnceLock::new();

pub fn init_host_context(asset_store: Arc<AssetStore>) {
    let ctx = Arc::new(HostContext {
        services: Mutex::new(HashMap::new()),
        asset_store,
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
    let mut g = c.event_sinks.lock().map_err(|_| "event_sinks mutex poisoned".to_string())?;
    g.push(Arc::new(Mutex::new(sink)));
    Ok(())
}

pub fn emit_plugin_event(topic: RString, payload: Blob) -> Result<(), String> {
    let c = ctx();
    let sinks = {
        let g = c.event_sinks.lock().map_err(|_| "event_sinks mutex poisoned".to_string())?;
        g.clone()
    };

    for s in sinks {
        let mut guard = s.lock().map_err(|_| "event sink mutex poisoned".to_string())?;
        guard.on_event(topic.clone(), payload.clone());
    }

    Ok(())
}