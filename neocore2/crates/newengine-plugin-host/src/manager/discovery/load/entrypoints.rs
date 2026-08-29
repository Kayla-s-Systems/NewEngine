use super::*;

impl PluginManager {
    #[inline]
    pub fn load_default(&mut self, host: HostApiV1) -> Result<(), PluginLoadError> {
        self.load_default_with_policy(host, false)
    }

    #[inline]
    pub fn load_default_with_policy(
        &mut self,
        host: HostApiV1,
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        let dir = default_plugins_dir()?;
        self.load_from_dir_with_policy(&dir, host, strict)
    }

    #[inline]
    pub fn load_bootstrap_default(
        &mut self,
        host: HostApiV1,
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        let dir = default_plugins_dir()?;
        self.load_from_dir_with_policy_and_filter(
            &dir,
            host,
            strict,
            LoadPhaseFilter::BootstrapOnly,
            PluginLoadOrigin::Auto,
        )
    }

    #[inline]
    pub fn load_bootstrap_from_dir(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        self.load_bootstrap_from_dir_with_origin(dir, host, strict, PluginLoadOrigin::Auto)
    }

    pub fn load_bootstrap_from_dir_with_origin(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
        load_origin: PluginLoadOrigin,
    ) -> Result<(), PluginLoadError> {
        self.load_from_dir_with_policy_and_filter(
            dir,
            host,
            strict,
            LoadPhaseFilter::BootstrapOnly,
            load_origin,
        )
    }

    #[inline]
    pub fn load_engine_default(
        &mut self,
        host: HostApiV1,
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        let dir = default_plugins_dir()?;
        self.load_from_dir_with_policy_and_filter(
            &dir,
            host,
            strict,
            LoadPhaseFilter::BootstrapAndEngine,
            PluginLoadOrigin::Auto,
        )
    }

    #[inline]
    pub fn load_engine_from_dir(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        self.load_engine_from_dir_with_origin(dir, host, strict, PluginLoadOrigin::Auto)
    }

    pub fn load_engine_from_dir_with_origin(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
        load_origin: PluginLoadOrigin,
    ) -> Result<(), PluginLoadError> {
        self.load_from_dir_with_policy_and_filter(
            dir,
            host,
            strict,
            LoadPhaseFilter::BootstrapAndEngine,
            load_origin,
        )
    }

    #[inline]
    pub fn load_from_dir(&mut self, dir: &Path, host: HostApiV1) -> Result<(), PluginLoadError> {
        self.load_from_dir_with_policy(dir, host, false)
    }

    pub fn load_from_dir_with_policy(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        self.load_from_dir_with_policy_and_origin(dir, host, strict, PluginLoadOrigin::Auto)
    }

    pub fn load_from_dir_with_policy_and_origin(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
        load_origin: PluginLoadOrigin,
    ) -> Result<(), PluginLoadError> {
        self.load_from_dir_with_policy_and_filter(
            dir,
            host,
            strict,
            LoadPhaseFilter::All,
            load_origin,
        )
    }

    /// Scans the complete plugin inventory and freezes the single authoritative
    /// provider-selection plan before any provider DLL is initialized.
    pub fn freeze_composition_plan_for_roots(
        &mut self,
        roots: &[(PathBuf, PluginLoadOrigin, bool)],
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        if self.frozen_composition_plan.is_some() {
            return Ok(());
        }

        let mut inventories = Vec::with_capacity(roots.len());
        for (dir, origin, required) in roots {
            match scan_plugin_discovery_graph(dir) {
                Ok(graph) => inventories.push((graph, *origin)),
                Err(error) if strict || *required => return Err(error),
                Err(error) => {
                    newengine_ulog_api::ulog::warn!(
                        "plugins: optional discovery root omitted from composition inventory path={} err={}",
                        display_clean(dir),
                        error,
                    );
                }
            }
        }

        let planning = crate::host_context::with_host_context(&self.host, || {
            crate::host_context::composition_planning_snapshot()
        });
        let frozen = build_frozen_composition_plan(&inventories, &planning);
        crate::host_context::with_host_context(&self.host, || {
            self.host.freeze_composition_plan(frozen.plan.clone())
        })
        .map_err(|message| PluginLoadError {
            path: roots
                .first()
                .map(|(path, _, _)| path.clone())
                .unwrap_or_default(),
            message,
        })?;

        newengine_ulog_api::ulog::info!(
            "plugins: authoritative composition plan frozen roots={} gateways={}",
            inventories.len(),
            frozen.plan.gateway_ids().len(),
        );
        self.frozen_composition_plan = Some(frozen);
        Ok(())
    }
    #[inline]
    pub fn invalidate_discovery_cache(&mut self) {
        self.discovery_cache = None;
    }

    #[inline]
    pub fn has_incremental_load_state(&self) -> bool {
        self.incremental_load.is_some()
    }

    #[inline]
    pub fn has_discovery_cache_for_dir(&self, dir: &Path) -> bool {
        self.discovery_cache
            .as_ref()
            .is_some_and(|graph| graph.dir == canonicalize_if_exists(dir))
    }

    pub fn begin_engine_incremental_load_from_discovery_graph(
        &mut self,
        graph: DiscoveryGraph,
        strict: bool,
    ) {
        self.begin_engine_incremental_load_from_discovery_graph_with_origin(
            graph,
            strict,
            PluginLoadOrigin::Auto,
        );
    }

    pub fn begin_engine_incremental_load_from_discovery_graph_with_origin(
        &mut self,
        graph: DiscoveryGraph,
        strict: bool,
        load_origin: PluginLoadOrigin,
    ) {
        self.begin_incremental_load_from_graph(
            graph,
            true,
            LoadPhaseFilter::BootstrapAndEngine,
            strict,
            load_origin,
        );
    }
}
