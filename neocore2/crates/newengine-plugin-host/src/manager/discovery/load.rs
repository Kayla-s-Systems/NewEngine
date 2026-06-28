#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;

use newengine_plugin_api::HostApiV1;

use super::graph::{DiscoveryGraph, LoadPhaseFilter};
use super::logging::{emit_discovery_logs, emit_selection_table};
use super::scan::scan_plugins_dir;
use super::selection::build_load_selection;
use crate::log_fmt::emit_boxed_kv;
use crate::manager::types::PluginLoadError;
use crate::path_fmt::{canonicalize_if_exists, display_clean};
use crate::paths::{default_plugins_dir, resolve_plugins_dir};
use crate::PluginManager;

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
        )
    }

    #[inline]
    pub fn load_bootstrap_from_dir(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        self.load_from_dir_with_policy_and_filter(dir, host, strict, LoadPhaseFilter::BootstrapOnly)
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
        )
    }

    #[inline]
    pub fn load_engine_from_dir(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        self.load_from_dir_with_policy_and_filter(
            dir,
            host,
            strict,
            LoadPhaseFilter::BootstrapAndEngine,
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
        self.load_from_dir_with_policy_and_filter(dir, host, strict, LoadPhaseFilter::All)
    }

    #[inline]
    pub fn invalidate_discovery_cache(&mut self) {
        self.discovery_cache = None;
    }

    fn load_from_dir_with_policy_and_filter(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
        filter: LoadPhaseFilter,
    ) -> Result<(), PluginLoadError> {
        let (graph, graph_is_new) = self.ensure_discovery_graph(dir)?;
        let selection = build_load_selection(&graph, filter, &self.loaded_ids);

        let loaded_ids_before = self.loaded_ids.clone();

        emit_boxed_kv(
            &format!("PluginDiscovery :: Phase Selection [{}]", filter.label()),
            &[
                ("dir", display_clean(&graph.dir)),
                (
                    "bootstrap_queue",
                    selection.bootstrap_candidates.len().to_string(),
                ),
                (
                    "engine_queue",
                    selection.engine_candidates.len().to_string(),
                ),
                ("platform_runtime", graph.platform_runtime_count.to_string()),
                ("unknown_dynlibs", graph.unknown_dynlibs.len().to_string()),
            ],
        );

        let mut load_errors: Vec<PluginLoadError> = Vec::new();

        for path in selection
            .bootstrap_candidates
            .iter()
            .chain(selection.engine_candidates.iter())
        {
            if let Err(e) = self.load_one(path, host.clone()) {
                newengine_ulog_api::ulog::warn!(
                    "plugins: failed to load '{}': {}",
                    display_clean(path),
                    e
                );
                load_errors.push(e);
            }
        }

        if graph_is_new {
            emit_discovery_logs(&graph);
        }

        emit_selection_table(&graph, &selection, filter);

        let loaded_total = self.loaded.len();
        let loaded_this_phase = loaded_total.saturating_sub(loaded_ids_before.len());

        emit_boxed_kv(
            &format!("PluginDiscovery :: Load Result [{}]", filter.label()),
            &[
                ("loaded_total", loaded_total.to_string()),
                ("loaded_this_phase", loaded_this_phase.to_string()),
                ("load_errors", load_errors.len().to_string()),
                ("scan_errors", graph.scan_errors.len().to_string()),
            ],
        );

        self.validate_required_capabilities();

        if newengine_ulog_api::ulog::debug_enabled() {
            for p in &self.loaded {
                newengine_ulog_api::ulog::debug!(
                    "plugins: loaded '{}' ver='{}' path='{}'",
                    p.info.id,
                    p.info.version,
                    display_clean(&p.path)
                );
            }
        }

        if strict && (!load_errors.is_empty() || !graph.scan_errors.is_empty()) {
            let mut msg = String::new();
            use std::fmt::Write as _;

            if !graph.scan_errors.is_empty() {
                let _ = writeln!(
                    msg,
                    "one or more dynamic libraries failed signature scan (count={}):",
                    graph.scan_errors.len()
                );
                for e in &graph.scan_errors {
                    let _ = writeln!(msg, "- {}", e);
                }
            }

            if !load_errors.is_empty() {
                let _ = writeln!(
                    msg,
                    "one or more plugins failed to load (count={}):",
                    load_errors.len()
                );
                for e in &load_errors {
                    let _ = writeln!(
                        msg,
                        "- path='{}' err='{}'",
                        display_clean(&e.path),
                        e.message
                    );
                }
            }

            return Err(PluginLoadError {
                path: graph.dir.clone(),
                message: msg,
            });
        }

        Ok(())
    }

    #[inline]
    pub fn load_path(&mut self, path: &Path, host: HostApiV1) -> Result<(), PluginLoadError> {
        self.load_one(path, host)
    }

    fn ensure_discovery_graph(
        &mut self,
        dir: &Path,
    ) -> Result<(DiscoveryGraph, bool), PluginLoadError> {
        let dir = resolve_plugins_dir(dir)?;

        if let Err(e) = std::fs::create_dir_all(&dir) {
            return Err(PluginLoadError {
                path: dir.clone(),
                message: format!("create_dir_all failed: {e}"),
            });
        }

        let dir = canonicalize_if_exists(&dir);

        if let Some(graph) = &self.discovery_cache {
            if graph.dir == dir {
                newengine_ulog_api::ulog::debug!(
                    "plugins: discovery cache hit dir='{}' entries={} dynlibs={}",
                    display_clean(&graph.dir),
                    graph.entries_total,
                    graph.items.len(),
                );
                return Ok((graph.clone(), false));
            }
        }

        let graph = scan_plugins_dir(&dir)?;
        self.discovery_cache = Some(graph.clone());
        Ok((graph, true))
    }
}
