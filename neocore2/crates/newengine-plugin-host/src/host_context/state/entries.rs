use newengine_math::collections::prelude::*;
use newengine_plugin_api::{
    EventSinkV1Dyn, PluginDescriptor, PluginInfo, PluginKind, ServiceV1Dyn,
};
use newengine_runtime_contract_catalog::RuntimeContractCatalog;
use std::cell::RefCell;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

static NEXT_HOST_CONTEXT_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) struct ServiceLifecycle {
    accepting_calls: AtomicBool,
    active_calls: AtomicUsize,
}

impl ServiceLifecycle {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            accepting_calls: AtomicBool::new(true),
            active_calls: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub(crate) fn try_acquire(self: &Arc<Self>) -> Option<ServiceCallLease> {
        if !self.accepting_calls.load(Ordering::Acquire) {
            return None;
        }
        self.active_calls.fetch_add(1, Ordering::AcqRel);
        if self.accepting_calls.load(Ordering::Acquire) {
            Some(ServiceCallLease {
                lifecycle: Arc::clone(self),
            })
        } else {
            self.active_calls.fetch_sub(1, Ordering::AcqRel);
            None
        }
    }

    #[inline]
    pub(crate) fn quiesce(&self) {
        self.accepting_calls.store(false, Ordering::Release);
    }

    #[inline]
    pub(crate) fn resume(&self) {
        self.accepting_calls.store(true, Ordering::Release);
    }

    #[inline]
    pub(crate) fn active_calls(&self) -> usize {
        self.active_calls.load(Ordering::Acquire)
    }
}

pub(crate) struct ServiceCallLease {
    lifecycle: Arc<ServiceLifecycle>,
}

impl Drop for ServiceCallLease {
    #[inline]
    fn drop(&mut self) {
        self.lifecycle.active_calls.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Clone)]
pub(crate) struct ServiceEntry {
    pub owner_plugin_id: Option<String>,
    pub service: Arc<ServiceV1Dyn<'static>>,
    pub lifecycle: Arc<ServiceLifecycle>,
}

impl ServiceEntry {
    #[inline]
    pub(crate) fn new(owner_plugin_id: Option<String>, service: ServiceV1Dyn<'static>) -> Self {
        Self {
            owner_plugin_id,
            service: Arc::from(service),
            lifecycle: Arc::new(ServiceLifecycle::new()),
        }
    }
}

#[derive(Clone)]
pub(crate) struct EventSinkEntry {
    pub owner_plugin_id: Option<String>,
    pub sink: Arc<Mutex<EventSinkV1Dyn<'static>>>,
    pub lifecycle: Arc<ServiceLifecycle>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineCapabilitySlotSnapshot {
    pub gateway_id: String,
    pub service_kind: String,
    /// Transitional compatibility bit derived from `requirement_level`.
    pub required: bool,
    pub requirement_level: newengine_service_api::CapabilityRequirementLevel,
    pub contract_id: Option<String>,
    pub min_contract_version: u32,
    pub max_contract_version: Option<u32>,
    pub required_tags: Vec<String>,
    pub preferred_tags: Vec<String>,
    pub conflict_tags: Vec<String>,
    pub fallback_provider_ids: Vec<String>,
    pub min_cardinality: u16,
    pub max_cardinality: u16,
    pub declared_by: String,
    pub state: String,
    pub provider_service_id: Option<String>,
    pub provider_owner_id: Option<String>,
    pub provider_origin: Option<String>,
    pub backend_capability_id: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct EngineCapabilitySlotEntry {
    pub(crate) requirement: newengine_service_api::CompositionRequirement,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DeclaredCompositionPolicy {
    pub(crate) preferred_tags: Vec<String>,
    pub(crate) forbidden_tags: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct EngineGatewayRouteSnapshot {
    pub gateway_id: String,
    pub service_kind: String,
    pub provider_service_id: String,
    pub provider_route_id: Option<String>,
    pub provider_abi: Option<String>,
    pub provider_owner_id: String,
    pub backend_capability_id: String,
    pub backend_priority: i32,
    pub origin: String,
    pub override_mode: String,
    pub active_score: i64,
    pub active: bool,
    pub selection_state: String,
    pub selection_reason: String,
}

#[derive(Clone, Debug)]
pub(crate) struct GatewayProviderRouteEntry {
    pub(crate) gateway_id: String,
    pub(crate) service_kind: String,
    pub(crate) provider_service_id: String,
    pub(crate) provider_route_id: String,
    pub(crate) provider_abi: Option<String>,
    pub(crate) provider_owner_id: String,
    pub(crate) backend_capability_id: String,
    pub(crate) backend_priority: i32,
    pub(crate) system_tags: Vec<String>,
    pub(crate) origin: crate::service_gateway::GatewayProviderOrigin,
}

#[derive(Clone)]
pub(crate) struct GatewayRegistryCache {
    pub(crate) generation: u64,
    pub(crate) registry: Arc<crate::service_gateway::ActiveGatewayRegistry>,
}

/// Service publication transaction used while a provider is being initialized.
/// Staged registrations are invisible to active routing until validation commits
/// the whole set atomically.
#[derive(Default)]
pub(crate) struct ProviderTransactionState {
    pub(crate) owner_plugin_id: String,
    pub(crate) accepts_host_owned: bool,
    pub(crate) staging_error: Option<String>,
    pub(crate) staged_descriptor: Option<PluginDescriptor>,
    pub(crate) staged_descriptor_v2: Option<newengine_plugin_api::PluginDescriptorV2>,
    pub(crate) staged_origin: Option<crate::service_gateway::GatewayProviderOrigin>,
    pub(crate) staged_contracts: Vec<newengine_runtime_contract_catalog::RuntimeContractSpec>,
    pub(crate) staged_services: NeHashMap<String, ServiceEntry>,
    pub(crate) staged_event_sinks: Vec<EventSinkEntry>,
    pub(crate) staged_gateway_routes: NeHashMap<String, GatewayProviderRouteEntry>,
}

thread_local! {
    static CURRENT_PLUGIN_ID: RefCell<Option<String>> = const { RefCell::new(None) };
    static CURRENT_HOST_CALLBACK_OWNER: RefCell<Option<String>> = const { RefCell::new(None) };
    static CURRENT_HOST_CONTEXT: RefCell<Option<Arc<HostContext>>> = const { RefCell::new(None) };
}
