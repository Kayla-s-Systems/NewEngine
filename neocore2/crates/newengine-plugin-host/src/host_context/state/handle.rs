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
