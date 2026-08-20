#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};

use newengine_plugin_api::HostApiV1;

use super::graph::{DiscoveryGraph, LoadPhaseFilter};
use super::logging::{emit_discovery_logs, emit_selection_table};
use super::scan::scan_plugins_dir;
use super::selection::build_load_selection;
use crate::manager::types::{PluginLoadError, PluginLoadOrigin};
use crate::paths::{default_plugins_dir, resolve_plugins_dir};
use crate::PluginManager;
use newengine_ulog_api::formatting::emit_boxed_kv;
use newengine_ulog_api::path_format::{canonicalize_if_exists, display_clean};

pub fn resolve_plugin_discovery_dir(dir: Option<&Path>) -> Result<PathBuf, PluginLoadError> {
    let dir = match dir {
        Some(dir) => resolve_plugins_dir(dir)?,
        None => default_plugins_dir()?,
    };

    if let Err(e) = std::fs::create_dir_all(&dir) {
        return Err(PluginLoadError {
            path: dir.clone(),
            message: format!("create_dir_all failed: {e}"),
        });
    }

    Ok(canonicalize_if_exists(&dir))
}

pub fn scan_plugin_discovery_graph(dir: &Path) -> Result<DiscoveryGraph, PluginLoadError> {
    let dir = resolve_plugin_discovery_dir(Some(dir))?;
    scan_plugins_dir(&dir)
}

#[derive(Debug, Clone)]
pub struct IncrementalLoadOutcome {
    pub finished: bool,
    pub loaded_total: usize,
    pub loaded_this_phase: usize,
    pub pending_total: usize,
    pub completed: usize,
    pub load_errors: usize,
    pub progress_01: f32,
    pub current_path: Option<PathBuf>,
}

impl IncrementalLoadOutcome {
    #[inline]
    fn running(
        loaded_total: usize,
        loaded_this_phase: usize,
        pending_total: usize,
        completed: usize,
        load_errors: usize,
        current_path: Option<PathBuf>,
    ) -> Self {
        let progress_01 = if pending_total == 0 {
            1.0
        } else {
            (completed as f32 / pending_total as f32).clamp(0.0, 1.0)
        };
        Self {
            finished: false,
            loaded_total,
            loaded_this_phase,
            pending_total,
            completed,
            load_errors,
            progress_01,
            current_path,
        }
    }

    #[inline]
    fn finished(
        loaded_total: usize,
        loaded_this_phase: usize,
        pending_total: usize,
        load_errors: usize,
    ) -> Self {
        Self {
            finished: true,
            loaded_total,
            loaded_this_phase,
            pending_total,
            completed: pending_total,
            load_errors,
            progress_01: 1.0,
            current_path: None,
        }
    }
}

pub(crate) struct IncrementalLoadState {
    graph: DiscoveryGraph,
    graph_is_new: bool,
    selection: super::selection::LoadSelection,
    filter: LoadPhaseFilter,
    pending: Vec<PathBuf>,
    next_index: usize,
    loaded_ids_before_len: usize,
    load_errors: Vec<PluginLoadError>,
    strict: bool,
    load_origin: PluginLoadOrigin,
}

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

    fn begin_incremental_load_from_graph(
        &mut self,
        graph: DiscoveryGraph,
        graph_is_new: bool,
        filter: LoadPhaseFilter,
        strict: bool,
        load_origin: PluginLoadOrigin,
    ) {
        let selection = build_load_selection(&graph, filter, &self.loaded_ids);
        let pending = selection
            .bootstrap_candidates
            .iter()
            .chain(selection.engine_candidates.iter())
            .cloned()
            .collect::<Vec<_>>();

        emit_boxed_kv(
            &format!(
                "PluginDiscovery :: Incremental Phase Selection [{}]",
                filter.label()
            ),
            &[
                ("dir", display_clean(&graph.dir)),
                ("pending", pending.len().to_string()),
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

        self.discovery_cache = Some(graph.clone());
        self.incremental_load = Some(IncrementalLoadState {
            graph,
            graph_is_new,
            selection,
            filter,
            pending,
            next_index: 0,
            loaded_ids_before_len: self.loaded_ids.len(),
            load_errors: Vec::new(),
            strict,
            load_origin,
        });
    }

    fn load_from_dir_with_policy_and_filter(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
        filter: LoadPhaseFilter,
        load_origin: PluginLoadOrigin,
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
            if let Err(e) = self.load_one_with_origin(path, host.clone(), load_origin) {
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

    /// Loads one explicitly selected dynamic plugin with a host-owned trust/origin.
    /// This lets project manifests select a game DLL before runtime-profile launch
    /// without scanning unrelated sibling libraries.
    #[inline]
    pub fn load_path_with_origin(
        &mut self,
        path: &Path,
        host: HostApiV1,
        load_origin: PluginLoadOrigin,
    ) -> Result<(), PluginLoadError> {
        self.load_one_with_origin(path, host, load_origin)
    }

    /// Loads exactly one descriptor-selected plugin id from the default runtime
    /// plugin directory without initializing unrelated providers.
    pub fn load_plugin_id_default_with_origin(
        &mut self,
        plugin_id: &str,
        host: HostApiV1,
        load_origin: PluginLoadOrigin,
    ) -> Result<bool, PluginLoadError> {
        let dir = default_plugins_dir()?;
        self.load_plugin_id_from_dir_with_origin(&dir, plugin_id, host, load_origin)
    }

    /// Loads exactly one descriptor-selected plugin id from a discovery directory.
    /// Unlike `load_from_dir*`, this does not initialize unrelated renderer/physics/UI
    /// providers just because they share the same pluginsRuntime directory.
    pub fn load_plugin_id_from_dir_with_origin(
        &mut self,
        dir: &Path,
        plugin_id: &str,
        host: HostApiV1,
        load_origin: PluginLoadOrigin,
    ) -> Result<bool, PluginLoadError> {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return Ok(false);
        }
        if self.loaded_ids.contains(plugin_id) {
            return Ok(true);
        }

        let (graph, _) = self.ensure_discovery_graph(dir)?;
        let selection = build_load_selection(&graph, LoadPhaseFilter::All, &self.loaded_ids);
        let selected_paths = selection
            .bootstrap_candidates
            .iter()
            .chain(selection.engine_candidates.iter());

        let path = graph.items.iter().find_map(|item| {
            let matches_id = matches!(
                &item.kind,
                super::graph::ScannedDynlibKind::Plugin { id, .. } if id == plugin_id
            );
            if matches_id
                && selected_paths
                    .clone()
                    .any(|selected| selected == &item.path)
            {
                Some(item.path.clone())
            } else {
                None
            }
        });

        let Some(path) = path else {
            return Ok(false);
        };
        self.load_one_with_origin(&path, host, load_origin)?;
        Ok(self.loaded_ids.contains(plugin_id))
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

impl PluginManager {
    #[inline]
    pub fn load_engine_default_incremental_step(
        &mut self,
        host: HostApiV1,
        strict: bool,
    ) -> Result<IncrementalLoadOutcome, PluginLoadError> {
        let dir = default_plugins_dir()?;
        self.load_engine_from_dir_incremental_step(&dir, host, strict)
    }

    #[inline]
    pub fn load_engine_from_dir_incremental_step(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
    ) -> Result<IncrementalLoadOutcome, PluginLoadError> {
        self.load_engine_from_dir_incremental_step_with_origin(
            dir,
            host,
            strict,
            PluginLoadOrigin::Auto,
        )
    }

    pub fn load_engine_from_dir_incremental_step_with_origin(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
        load_origin: PluginLoadOrigin,
    ) -> Result<IncrementalLoadOutcome, PluginLoadError> {
        self.load_from_dir_incremental_step(
            dir,
            host,
            strict,
            LoadPhaseFilter::BootstrapAndEngine,
            load_origin,
        )
    }

    fn load_from_dir_incremental_step(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
        filter: LoadPhaseFilter,
        load_origin: PluginLoadOrigin,
    ) -> Result<IncrementalLoadOutcome, PluginLoadError> {
        if self.incremental_load.is_none() {
            let (graph, graph_is_new) = self.ensure_discovery_graph(dir)?;
            self.begin_incremental_load_from_graph(
                graph,
                graph_is_new,
                filter,
                strict,
                load_origin,
            );
        }

        let Some(mut state) = self.incremental_load.take() else {
            unreachable!("incremental load state must exist after initialization");
        };

        let pending_total = state.pending.len();
        let mut current_path = None;

        if state.next_index < pending_total {
            let path = state.pending[state.next_index].clone();
            current_path = Some(path.clone());
            if let Err(e) = self.load_one_with_origin(&path, host.clone(), state.load_origin) {
                newengine_ulog_api::ulog::warn!(
                    "plugins: failed to load '{}': {}",
                    display_clean(&path),
                    e
                );
                state.load_errors.push(e);
            }
            state.next_index = state.next_index.saturating_add(1);
        }

        let loaded_total = self.loaded.len();
        let loaded_this_phase = loaded_total.saturating_sub(state.loaded_ids_before_len);
        let completed = state.next_index.min(pending_total);
        let load_errors = state.load_errors.len();

        if completed < pending_total {
            let outcome = IncrementalLoadOutcome::running(
                loaded_total,
                loaded_this_phase,
                pending_total,
                completed,
                load_errors,
                current_path,
            );
            self.incremental_load = Some(state);
            return Ok(outcome);
        }

        if state.graph_is_new {
            emit_discovery_logs(&state.graph);
        }

        emit_selection_table(&state.graph, &state.selection, state.filter);

        emit_boxed_kv(
            &format!(
                "PluginDiscovery :: Incremental Load Result [{}]",
                state.filter.label()
            ),
            &[
                ("loaded_total", loaded_total.to_string()),
                ("loaded_this_phase", loaded_this_phase.to_string()),
                ("load_errors", load_errors.to_string()),
                ("scan_errors", state.graph.scan_errors.len().to_string()),
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

        if state.strict && (!state.load_errors.is_empty() || !state.graph.scan_errors.is_empty()) {
            let mut msg = String::new();
            use std::fmt::Write as _;

            if !state.graph.scan_errors.is_empty() {
                let _ = writeln!(
                    msg,
                    "one or more dynamic libraries failed signature scan (count={}):",
                    state.graph.scan_errors.len()
                );
                for e in &state.graph.scan_errors {
                    let _ = writeln!(msg, "- {}", e);
                }
            }

            if !state.load_errors.is_empty() {
                let _ = writeln!(
                    msg,
                    "one or more plugins failed to load (count={}):",
                    state.load_errors.len()
                );
                for e in &state.load_errors {
                    let _ = writeln!(
                        msg,
                        "- path='{}' err='{}'",
                        display_clean(&e.path),
                        e.message
                    );
                }
            }

            return Err(PluginLoadError {
                path: state.graph.dir.clone(),
                message: msg,
            });
        }

        Ok(IncrementalLoadOutcome::finished(
            loaded_total,
            loaded_this_phase,
            pending_total,
            load_errors,
        ))
    }
}
