use newengine_plugin_api::{EventSinkV1Dyn, PluginDescriptor, PluginInfo, PluginKind, ServiceV1Dyn};
use newengine_math::collections::prelude::*;
use std::cell::RefCell;
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
    pub provider_route_id: Option<String>,
    pub provider_owner_id: String,
    pub backend_capability_id: String,
    pub backend_priority: i32,
    pub origin: String,
    pub override_mode: String,
    pub active_score: i64,
    pub active: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct EngineOwnedGatewayEntry {
    pub(crate) gateway_id: String,
    pub(crate) service_kind: String,
    pub(crate) provider_service_id: String,
    pub(crate) provider_owner_id: String,
    pub(crate) backend_capability_id: String,
    pub(crate) backend_priority: i32,
}

#[derive(Clone)]
pub(crate) struct GatewayRegistryCache {
    pub(crate) generation: u64,
    pub(crate) registry: crate::service_gateway::ActiveGatewayRegistry,
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
    pub(crate) services_generation: AtomicU64,

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
    pub(crate) engine_owned_gateways: Mutex<NeHashMap<String, EngineOwnedGatewayEntry>>,

    /// Cached active gateway registry. Routing is on the service hot path, so
    /// descriptor/fact folding must happen only when the gateway fact generation
    /// changes, not on every `call_service_v1(engine.*)` call.
    pub(crate) gateway_registry_cache: Mutex<Option<GatewayRegistryCache>>,
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
