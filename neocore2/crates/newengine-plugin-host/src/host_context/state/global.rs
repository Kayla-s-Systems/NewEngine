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
    })
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

/// Creates and activates a fresh hermetic host context with an empty environment snapshot.
/// Launchers that intentionally import process environment must capture it at their bootstrap
/// boundary and call `create_host_context_with_environment_snapshot` explicitly.
pub fn create_host_context() -> HostContextHandle {
    create_host_context_with_environment_snapshot(std::iter::empty::<(OsString, OsString)>())
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
            *slot.borrow_mut() = Some(make_ctx(NeHashMap::default()));
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
            // Unbound runtime threads receive a hermetic empty context. Process environment
            // can enter only through an explicit launcher-owned bootstrap snapshot.
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
