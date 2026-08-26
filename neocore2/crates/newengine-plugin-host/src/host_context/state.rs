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

/// Runs a host module callback in a topology-read-only scope. `Module::init()` is
/// intentionally not wrapped: it owns a ProviderRegistrationTransaction instead.
pub fn with_host_module_callback<R>(owner_id: &str, f: impl FnOnce() -> R) -> R {
    CURRENT_HOST_CALLBACK_OWNER.with(|cell| {
        let previous = cell.replace(Some(owner_id.to_owned()));
        struct Restore<'a> {
            cell: &'a RefCell<Option<String>>,
            previous: Option<String>,
        }
        impl<'a> Drop for Restore<'a> {
            fn drop(&mut self) {
                let _ = self.cell.replace(self.previous.take());
            }
        }
        let _restore = Restore { cell, previous };
        f()
    })
}

#[inline]
pub(crate) fn reject_topology_mutation_from_host_callback(operation: &str) -> Result<(), String> {
    CURRENT_HOST_CALLBACK_OWNER.with(|cell| {
        if let Some(owner) = cell.borrow().as_deref() {
            Err(format!(
                "topology mutation is forbidden during host module callback: owner='{}' operation='{}'; publish providers only through Module::init() transaction or host control plane",
                owner, operation
            ))
        } else {
            Ok(())
        }
    })
}

pub(crate) struct HostContext {
    pub(crate) instance_id: u64,
    pub(crate) services: Mutex<NeHashMap<String, ServiceEntry>>,
    pub(crate) services_generation: AtomicU64,
    pub(crate) event_sinks: Mutex<Arc<[EventSinkEntry]>>,
    pub(crate) plugin_descriptors: Mutex<NeHashMap<String, PluginDescriptor>>,
    pub(crate) plugin_descriptors_v2:
        Mutex<NeHashMap<String, newengine_plugin_api::PluginDescriptorV2>>,
    pub(crate) plugin_origins:
        Mutex<NeHashMap<String, crate::service_gateway::GatewayProviderOrigin>>,
    pub(crate) runtime_contract_catalog: Mutex<RuntimeContractCatalog>,
    pub(crate) external_runtime_plugins: Mutex<NeHashMap<String, ExternalRuntimePluginEntry>>,
    pub(crate) gateway_provider_routes: Mutex<NeHashMap<String, GatewayProviderRouteEntry>>,
    pub(crate) capability_slots: Mutex<NeHashMap<String, EngineCapabilitySlotEntry>>,
    pub(crate) composition_policy: Mutex<DeclaredCompositionPolicy>,
    pub(crate) gateway_selection_policies:
        Mutex<NeHashMap<String, crate::host_context::gateway::EngineGatewaySelectionPolicy>>,
    pub(crate) gateway_registry_cache: Mutex<Option<GatewayRegistryCache>>,
    pub(crate) frozen_composition_plan: RwLock<Option<Arc<newengine_service_api::CompositionPlan>>>,
    pub(crate) provider_transaction: Mutex<Option<ProviderTransactionState>>,
    pub(crate) plugin_config_store:
        Mutex<Option<Arc<crate::plugin_config_service::PluginConfigStore>>>,
    pub(crate) host_job_seq: AtomicU64,
    pub(crate) active_host_jobs: Mutex<NeHashMap<String, crate::diagnostics::PluginHostJobRecord>>,
    pub(crate) plugin_root_observers: RwLock<crate::root_observers::PluginRootObserverState>,
    pub(crate) invalid_gateway_route_warnings: Mutex<NeHashSet<String>>,
    pub(crate) gateway_resolution_diagnostics: Mutex<NeHashMap<String, String>>,
    pub(crate) warned_retired_capabilities: AtomicBool,
    /// Per-Engine snapshot of process/bootstrap environment. Runtime policy must
    /// read this snapshot instead of observing later process-global mutations.
    pub(crate) environment: RwLock<Arc<NeHashMap<OsString, OsString>>>,
    pub(crate) run_id: std::sync::OnceLock<String>,
}

/// Instance-owned host state handle. It is cheap to clone and is intended to live
/// inside `Engine`; no process-global HostContext exists anymore.
#[derive(Clone)]
pub struct HostContextHandle {
    inner: Arc<HostContext>,
}

impl HostContextHandle {
    #[inline]
    pub(crate) fn arc(&self) -> Arc<HostContext> {
        Arc::clone(&self.inner)
    }

    /// Stable identity for the lifetime of this host universe. This is used only
    /// to partition per-thread caches; it is not a process-global registry key.
    #[inline]
    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.inner) as usize
    }

    /// Opaque, process-local Engine instance identity for observability/correlation.
    /// Unlike `identity()`, this never exposes a memory address.
    #[inline]
    pub fn instance_id(&self) -> u64 {
        self.inner.instance_id
    }

    /// Replaces this Engine's environment snapshot explicitly. This is the
    /// preferred path for Editor/PIE/preview instances because it never mutates
    /// or depends on process-global environment after construction.
    pub fn replace_environment_snapshot(
        &self,
        variables: impl IntoIterator<Item = (OsString, OsString)>,
    ) {
        let snapshot: NeHashMap<OsString, OsString> = variables.into_iter().collect();
        match self.inner.environment.write() {
            Ok(mut slot) => *slot = Arc::new(snapshot),
            Err(poisoned) => *poisoned.into_inner() = Arc::new(snapshot),
        }
    }

    /// Compatibility ingress for standalone launchers. Snapshot once at a
    /// bootstrap boundary; runtime policy then reads instance-owned state.
    pub fn refresh_environment_from_process(&self) {
        self.replace_environment_snapshot(std::env::vars_os());
    }

    /// Applies one launch-time environment override to this Engine instance.
    /// This mutates only the instance snapshot; it never touches process env.
    pub fn set_environment_var(&self, name: impl Into<OsString>, value: impl Into<OsString>) {
        let name = name.into();
        let value = value.into();
        match self.inner.environment.write() {
            Ok(mut slot) => {
                Arc::make_mut(&mut *slot).insert(name, value);
            }
            Err(poisoned) => {
                let mut slot = poisoned.into_inner();
                Arc::make_mut(&mut *slot).insert(name, value);
            }
        }
    }

    /// Removes one launch-time environment override from this Engine instance.
    /// This mutates only the instance snapshot; it never touches process env.
    pub fn remove_environment_var(&self, name: &str) {
        match self.inner.environment.write() {
            Ok(mut slot) => {
                Arc::make_mut(&mut *slot).remove(OsStr::new(name));
            }
            Err(poisoned) => {
                let mut slot = poisoned.into_inner();
                Arc::make_mut(&mut *slot).remove(OsStr::new(name));
            }
        }
    }

    #[inline]
    pub fn environment_var_os(&self, name: &str) -> Option<OsString> {
        let environment = match self.inner.environment.read() {
            Ok(value) => value,
            Err(poisoned) => poisoned.into_inner(),
        };
        environment.get(OsStr::new(name)).cloned()
    }

    #[inline]
    pub fn environment_var(&self, name: &str) -> Option<String> {
        self.environment_var_os(name)
            .and_then(|value| value.into_string().ok())
    }

    /// Installs the one authoritative provider-selection plan for this Engine.
    /// Re-installing the exact same plan is idempotent; replacing it with a
    /// different plan is rejected so bootstrap cannot silently re-compose after
    /// provider loading has started.
    pub(crate) fn freeze_composition_plan(
        &self,
        plan: newengine_service_api::CompositionPlan,
    ) -> Result<(), String> {
        let mut slot = match self.inner.frozen_composition_plan.write() {
            Ok(value) => value,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(existing) = slot.as_ref() {
            if existing.as_ref() == &plan {
                return Ok(());
            }
            return Err(
                "authoritative composition plan is already frozen for this host context".to_owned(),
            );
        }
        *slot = Some(Arc::new(plan));
        drop(slot);
        self.inner
            .services_generation
            .fetch_add(2, Ordering::AcqRel);
        Ok(())
    }

    /// Returns the complete contract universe owned by this Engine instance.
    pub fn runtime_contracts(
        &self,
    ) -> Vec<newengine_runtime_contract_catalog::RuntimeContractEntry> {
        let catalog = match self.inner.runtime_contract_catalog.lock() {
            Ok(value) => value,
            Err(poisoned) => poisoned.into_inner(),
        };
        catalog.list()
    }

    pub fn runtime_contract(
        &self,
        key: &str,
    ) -> Option<newengine_runtime_contract_catalog::RuntimeContractEntry> {
        let catalog = match self.inner.runtime_contract_catalog.lock() {
            Ok(value) => value,
            Err(poisoned) => poisoned.into_inner(),
        };
        catalog.contract(key).cloned()
    }

    pub fn runtime_contract_by_advertised_id(
        &self,
        id: &str,
    ) -> Option<newengine_runtime_contract_catalog::RuntimeContractEntry> {
        let catalog = match self.inner.runtime_contract_catalog.lock() {
            Ok(value) => value,
            Err(poisoned) => poisoned.into_inner(),
        };
        catalog.contract_by_advertised_id(id).cloned()
    }

    /// Captures the stable composition observability surface for this exact Engine instance.
    pub fn composition_snapshot_v1(&self) -> newengine_service_api::CompositionSnapshotV1 {
        with_host_context(
            self,
            crate::host_context::gateway::engine_composition_snapshot_v1,
        )
    }

    /// JSON form of [`Self::composition_snapshot_v1`], suitable for profiler,
    /// console, editor inspector and crash-report attachment surfaces.
    pub fn composition_snapshot_v1_json(&self) -> Result<String, String> {
        self.composition_snapshot_v1()
            .to_json()
            .map_err(|error| format!("composition.snapshot_v1 serialization failed: {error}"))
    }
}

fn make_ctx(environment: NeHashMap<OsString, OsString>) -> Arc<HostContext> {
    Arc::new(HostContext {
        instance_id: NEXT_HOST_CONTEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed),
        services: Mutex::new(NeHashMap::default()),
        services_generation: AtomicU64::new(2),
        event_sinks: Mutex::new(Arc::from(Vec::<EventSinkEntry>::new())),
        plugin_descriptors: Mutex::new(NeHashMap::default()),
        plugin_descriptors_v2: Mutex::new(NeHashMap::default()),
        plugin_origins: Mutex::new(NeHashMap::default()),
        runtime_contract_catalog: Mutex::new(RuntimeContractCatalog::default()),
        external_runtime_plugins: Mutex::new(NeHashMap::default()),
        gateway_provider_routes: Mutex::new(NeHashMap::default()),
        capability_slots: Mutex::new(NeHashMap::default()),
        composition_policy: Mutex::new(DeclaredCompositionPolicy::default()),
        gateway_selection_policies: Mutex::new(NeHashMap::default()),
        gateway_registry_cache: Mutex::new(None),
        frozen_composition_plan: RwLock::new(None),
        provider_transaction: Mutex::new(None),
        plugin_config_store: Mutex::new(None),
        host_job_seq: AtomicU64::new(1),
        active_host_jobs: Mutex::new(NeHashMap::default()),
        plugin_root_observers: RwLock::new(
            crate::root_observers::PluginRootObserverState::default(),
        ),
        invalid_gateway_route_warnings: Mutex::new(NeHashSet::default()),
        gateway_resolution_diagnostics: Mutex::new(NeHashMap::default()),
        warned_retired_capabilities: AtomicBool::new(false),
        environment: RwLock::new(Arc::new(environment)),
        run_id: std::sync::OnceLock::new(),
    })
}

fn make_default_ctx() -> Arc<HostContext> {
    make_ctx(std::env::vars_os().collect())
}

/// Creates and activates a fresh host context from an explicit environment snapshot.
/// This is the preferred launcher path for multi-instance runtimes.
pub fn create_host_context_with_environment_snapshot(
    variables: impl IntoIterator<Item = (OsString, OsString)>,
) -> HostContextHandle {
    let handle = HostContextHandle {
        inner: make_ctx(variables.into_iter().collect()),
    };
    activate_host_context(&handle);
    handle
}

/// Compatibility constructor for callers that intentionally use the current process environment
/// as their one-time bootstrap snapshot.
pub fn create_host_context() -> HostContextHandle {
    create_host_context_with_environment_snapshot(std::env::vars_os())
}

/// Activates an Engine-owned context on the current execution thread.
#[inline]
pub fn activate_host_context(handle: &HostContextHandle) {
    CURRENT_HOST_CONTEXT.with(|slot| {
        *slot.borrow_mut() = Some(handle.arc());
    });
}

/// Runs a bounded operation against an explicit Engine-owned host context.
pub fn with_host_context<R>(handle: &HostContextHandle, f: impl FnOnce() -> R) -> R {
    CURRENT_HOST_CONTEXT.with(|slot| {
        let previous = slot.replace(Some(handle.arc()));
        struct Restore<'a> {
            slot: &'a RefCell<Option<Arc<HostContext>>>,
            previous: Option<Arc<HostContext>>,
        }
        impl<'a> Drop for Restore<'a> {
            fn drop(&mut self) {
                let _ = self.slot.replace(self.previous.take());
            }
        }
        let _restore = Restore { slot, previous };
        f()
    })
}

/// Compatibility bootstrap for code paths that have not yet received an explicit
/// Engine handle. The fallback is thread-local, never process-global.
pub fn init_host_context() {
    CURRENT_HOST_CONTEXT.with(|slot| {
        if slot.borrow().is_none() {
            *slot.borrow_mut() = Some(make_default_ctx());
        }
    });
}

/// Returns a handle to the currently scoped host context. Compatibility callers
/// should prefer passing an explicit `HostContextHandle` from their Engine.
pub fn current_host_context() -> HostContextHandle {
    HostContextHandle { inner: ctx() }
}

#[inline]
pub(crate) fn current_host_context_identity() -> usize {
    let current = ctx();
    Arc::as_ptr(&current) as usize
}

#[inline]
pub(crate) fn environment_var(name: &str) -> Option<String> {
    current_host_context().environment_var(name)
}

#[inline]
pub(crate) fn environment_var_os(name: &str) -> Option<OsString> {
    current_host_context().environment_var_os(name)
}

pub(crate) fn environment_snapshot_utf8() -> NeHashMap<String, String> {
    let context = ctx();
    let environment = match context.environment.read() {
        Ok(value) => value,
        Err(poisoned) => poisoned.into_inner(),
    };
    environment
        .iter()
        .filter_map(|(key, value)| Some((key.to_str()?.to_owned(), value.to_str()?.to_owned())))
        .collect()
}

pub(crate) fn ctx() -> Arc<HostContext> {
    CURRENT_HOST_CONTEXT.with(|slot| {
        if slot.borrow().is_none() {
            // Never re-snapshot process environment from an arbitrary runtime thread.
            // Callers that intentionally bootstrap from process state must do so explicitly
            // through `init_host_context` / `create_host_context`.
            *slot.borrow_mut() = Some(make_ctx(NeHashMap::default()));
        }
        slot.borrow()
            .as_ref()
            .expect("host context installed")
            .clone()
    })
}

#[inline]
pub fn services_generation() -> u64 {
    ctx().services_generation.load(Ordering::Acquire)
}

#[inline]
pub fn bump_services_generation() {
    ctx().services_generation.fetch_add(2, Ordering::AcqRel);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_context_handles_have_distinct_instance_identity() {
        let a = create_host_context();
        let b = create_host_context();
        assert_ne!(a.identity(), b.identity());
        assert_ne!(a.instance_id(), b.instance_id());
    }

    #[test]
    fn runtime_contract_catalog_is_instance_scoped() {
        let a = create_host_context();
        let b = create_host_context();
        let contract = newengine_runtime_contract_catalog::RuntimeContractSpec {
            key: "test.instance.contract".to_owned(),
            kind: newengine_runtime_contract_catalog::ContractKind::Protocol,
            version: newengine_runtime_contract_catalog::ContractVersion::major(1),
            compatibility: newengine_runtime_contract_catalog::ContractCompatibility::SameMajor,
            owner: "test.instance.owner".to_owned(),
            advertised_id: Some("test.instance.contract.v1".to_owned()),
        };
        {
            let mut catalog = a.inner.runtime_contract_catalog.lock().unwrap();
            catalog
                .replace_plugin_contracts("test.instance.owner", vec![contract])
                .unwrap();
        }
        assert!(a.runtime_contract("test.instance.contract").is_some());
        assert!(b.runtime_contract("test.instance.contract").is_none());
        assert!(a.runtime_contract("render.provider.abi").is_some());
        assert!(b.runtime_contract("render.provider.abi").is_some());
    }

    #[test]
    fn environment_snapshots_are_instance_scoped() {
        let a = create_host_context();
        a.replace_environment_snapshot([(
            OsString::from("NEWENGINE_TEST_INSTANCE_ENV"),
            OsString::from("alpha"),
        )]);
        let b = create_host_context();
        b.replace_environment_snapshot([(
            OsString::from("NEWENGINE_TEST_INSTANCE_ENV"),
            OsString::from("beta"),
        )]);

        assert_eq!(
            a.environment_var("NEWENGINE_TEST_INSTANCE_ENV").as_deref(),
            Some("alpha")
        );
        assert_eq!(
            b.environment_var("NEWENGINE_TEST_INSTANCE_ENV").as_deref(),
            Some("beta")
        );

        activate_host_context(&a);
        assert_eq!(
            environment_var("NEWENGINE_TEST_INSTANCE_ENV").as_deref(),
            Some("alpha")
        );
        activate_host_context(&b);
        assert_eq!(
            environment_var("NEWENGINE_TEST_INSTANCE_ENV").as_deref(),
            Some("beta")
        );
    }

    #[test]
    fn explicit_environment_snapshots_and_overrides_are_instance_scoped() {
        let a = create_host_context_with_environment_snapshot([(
            OsString::from("NEWENGINE_TEST_EXPLICIT_ENV"),
            OsString::from("alpha"),
        )]);
        let b = create_host_context_with_environment_snapshot([(
            OsString::from("NEWENGINE_TEST_EXPLICIT_ENV"),
            OsString::from("beta"),
        )]);

        a.set_environment_var("NEWENGINE_TEST_EXPLICIT_ENV", "gamma");
        assert_eq!(
            a.environment_var("NEWENGINE_TEST_EXPLICIT_ENV").as_deref(),
            Some("gamma")
        );
        assert_eq!(
            b.environment_var("NEWENGINE_TEST_EXPLICIT_ENV").as_deref(),
            Some("beta")
        );

        a.remove_environment_var("NEWENGINE_TEST_EXPLICIT_ENV");
        assert!(a.environment_var("NEWENGINE_TEST_EXPLICIT_ENV").is_none());
        assert_eq!(
            b.environment_var("NEWENGINE_TEST_EXPLICIT_ENV").as_deref(),
            Some("beta")
        );
    }

    #[test]
    fn implicit_unbound_thread_context_has_no_process_environment_snapshot() {
        let snapshot = std::thread::spawn(environment_snapshot_utf8)
            .join()
            .expect("unbound HostContext probe thread panicked");
        assert!(snapshot.is_empty());
    }

    #[test]
    fn scoped_host_context_restores_previous_instance() {
        let outer = create_host_context();
        let inner = create_host_context();
        activate_host_context(&outer);
        assert_eq!(current_host_context().identity(), outer.identity());

        with_host_context(&inner, || {
            assert_eq!(current_host_context().identity(), inner.identity());
        });

        assert_eq!(current_host_context().identity(), outer.identity());
    }
}
