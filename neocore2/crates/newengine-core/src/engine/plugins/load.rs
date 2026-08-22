use super::super::plugin_discovery::PluginDiscoveryScanTask;
use super::super::{Engine, PluginFaultTolerance};
use super::bootstrap_preload_deferred;

use crate::error::{EngineError, EngineResult};
use crate::lifecycle_events::EngineLifecycleEvent;
use crate::plugin_forward_logger::install_forward_logger_once;
use newengine_plugin_host::default_host_api;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

static STARTUP_LOGS_EMITTED_AFTER_LOGGER_READY: AtomicBool = AtomicBool::new(false);

fn emit_startup_logs_after_logger_ready() {
    if STARTUP_LOGS_EMITTED_AFTER_LOGGER_READY.swap(true, Ordering::AcqRel) {
        return;
    }

    let rid = crate::run_id::run_id().unwrap_or("<unknown>");
    newengine_ulog_api::ulog::info!("startup: Run ID: {}", rid);
    if let Some(r) = crate::startup::last_load_report() {
        r.emit_logs();
    }
}

fn plugin_discovery_pending_outcome(
    loaded_total: usize,
    current_path: Option<std::path::PathBuf>,
) -> newengine_plugin_host::IncrementalLoadOutcome {
    newengine_plugin_host::IncrementalLoadOutcome {
        finished: false,
        loaded_total,
        loaded_this_phase: 0,
        pending_total: 1,
        completed: 0,
        load_errors: 0,
        progress_01: 0.02,
        current_path,
    }
}

impl<E: Send + 'static> Engine<E> {
    pub fn preload_bootstrap_plugins(&mut self) -> EngineResult<()> {
        if bootstrap_preload_deferred() {
            // Crash-safe standalone path: do not dlopen bootstrap DLLs before
            // platform/runtime diagnostics are visible. The engine plugin phase
            // loads bootstrap+engine plugins together.
            return Ok(());
        }

        let strict = matches!(self.plugin_fault_tolerance, PluginFaultTolerance::Strict);
        let host = default_host_api();

        let roots = self.resolved_plugin_discovery_roots()?;
        for root in roots {
            if let Err(error) = self.plugins.load_bootstrap_from_dir_with_origin(
                &root.dir,
                host.clone(),
                strict,
                root.origin,
            ) {
                if strict {
                    install_forward_logger_once(host);
                    return Err(EngineError::Other(format!(
                        "plugins: bootstrap load failed owner='{}' origin='{}' root='{}': {error}",
                        root.owner,
                        root.origin.as_str(),
                        root.dir.display(),
                    )));
                }
                newengine_ulog_api::ulog::warn!(
                    "plugins: non-fatal bootstrap root error owner='{}' origin='{}' root='{}': {}",
                    root.owner,
                    root.origin.as_str(),
                    root.dir.display(),
                    error,
                );
            }
        }

        install_forward_logger_once(host);
        emit_startup_logs_after_logger_ready();
        Ok(())
    }

    pub fn load_engine_plugins_once(&mut self) -> EngineResult<usize> {
        if self.engine_plugins_loaded {
            newengine_ulog_api::ulog::debug!("plugins: engine load skipped (already loaded)");
            return Ok(0);
        }

        let strict = matches!(self.plugin_fault_tolerance, PluginFaultTolerance::Strict);
        let phase = "engine-load";
        let t0 = Instant::now();
        let host = default_host_api();

        let roots = self.resolved_plugin_discovery_roots()?;
        for root in roots {
            if let Err(error) = self.plugins.load_engine_from_dir_with_origin(
                &root.dir,
                host.clone(),
                strict,
                root.origin,
            ) {
                match self.plugin_fault_tolerance {
                    PluginFaultTolerance::Strict => {
                        install_forward_logger_once(host);
                        return Err(EngineError::Other(format!(
                            "plugins: engine load failed (phase={} {} owner='{}' origin='{}' root='{}'): {error}",
                            phase,
                            Self::elapsed_since(t0),
                            root.owner,
                            root.origin.as_str(),
                            root.dir.display(),
                        )));
                    }
                    PluginFaultTolerance::Resilient => {
                        newengine_ulog_api::ulog::warn!(
                            "plugins: non-fatal engine root error (phase={} {} owner='{}' origin='{}' root='{}'): {}",
                            phase,
                            Self::elapsed_since(t0),
                            root.owner,
                            root.origin.as_str(),
                            root.dir.display(),
                            error,
                        );
                    }
                }
            }
        }

        install_forward_logger_once(host);
        if bootstrap_preload_deferred() {
            emit_startup_logs_after_logger_ready();
        }

        self.validate_required_plugins_loaded()?;
        self.engine_plugins_loaded = true;
        self.plugins_loaded = true;
        self.expose_plugins_snapshot();

        let loaded = self.plugins.snapshot().len();
        let event = EngineLifecycleEvent::EnginePluginsReady {
            loaded_count: loaded,
            origin: "load_engine_plugins_once",
        };
        self.mark_readiness_observed(&event);
        self.events.publish(event)?;
        Self::log_phase_ok("plugins", phase, Some(loaded), Self::elapsed_since(t0));
        self.log_plugins_diagnostics("after engine plugins init");

        Ok(loaded)
    }

    pub fn load_engine_plugins_incremental_step(
        &mut self,
    ) -> EngineResult<newengine_plugin_host::IncrementalLoadOutcome> {
        if self.engine_plugins_loaded {
            newengine_ulog_api::ulog::debug!(
                "plugins: incremental engine load skipped (already loaded)"
            );
            let loaded = self.plugins.snapshot().len();
            return Ok(newengine_plugin_host::IncrementalLoadOutcome {
                finished: true,
                loaded_total: loaded,
                loaded_this_phase: 0,
                pending_total: 0,
                completed: 0,
                load_errors: 0,
                progress_01: 1.0,
                current_path: None,
            });
        }

        let strict = matches!(self.plugin_fault_tolerance, PluginFaultTolerance::Strict);
        let host = default_host_api();
        let roots = self.resolved_plugin_discovery_roots()?;
        if roots.is_empty() {
            return Err(EngineError::Other(
                "plugins: no discovery roots resolved for incremental load".to_owned(),
            ));
        }
        if self.plugin_discovery_root_index >= roots.len() {
            self.plugin_discovery_root_index = 0;
        }
        let root_index = self.plugin_discovery_root_index;
        let root = roots[root_index].clone();
        let discovery_dir = root.dir.clone();

        if !self.plugins.has_incremental_load_state()
            && !self.plugins.has_discovery_cache_for_dir(&discovery_dir)
        {
            if self.plugin_discovery_scan.is_none() {
                newengine_ulog_api::ulog::info!(
                    "plugins: discovery scan submitted via engine.threading root={}/{} owner='{}' origin='{}' dir='{}'",
                    root_index + 1,
                    roots.len(),
                    root.owner,
                    root.origin.as_str(),
                    discovery_dir.display(),
                );
                self.plugin_discovery_scan = Some(PluginDiscoveryScanTask::submit(
                    self.thread_pool(),
                    discovery_dir.clone(),
                ));
                let mut pending = plugin_discovery_pending_outcome(
                    self.plugins.snapshot().len(),
                    Some(discovery_dir),
                );
                pending.progress_01 = root_index as f32 / roots.len() as f32;
                return Ok(pending);
            }

            if let Some(task) = self.plugin_discovery_scan.as_ref() {
                if let Some(result) = task.take_result() {
                    let task_dir = task.dir().clone();
                    self.plugin_discovery_scan = None;
                    match result {
                        Ok(graph) => {
                            newengine_ulog_api::ulog::info!(
                                "plugins: discovery scan committed root={}/{} owner='{}' origin='{}' dir='{}'",
                                root_index + 1,
                                roots.len(),
                                root.owner,
                                root.origin.as_str(),
                                task_dir.display(),
                            );
                            self.plugins
                                .begin_engine_incremental_load_from_discovery_graph_with_origin(
                                    graph,
                                    strict,
                                    root.origin,
                                );
                        }
                        Err(error) => {
                            if strict || root.required {
                                return Err(EngineError::Other(format!(
                                    "plugins: async discovery scan failed owner='{}' origin='{}' root='{}': {error}",
                                    root.owner,
                                    root.origin.as_str(),
                                    root.dir.display(),
                                )));
                            }
                            newengine_ulog_api::ulog::warn!(
                                "plugins: optional discovery root scan failed owner='{}' origin='{}' root='{}': {}",
                                root.owner,
                                root.origin.as_str(),
                                root.dir.display(),
                                error,
                            );
                            self.plugin_discovery_root_index += 1;
                            let loaded = self.plugins.snapshot().len();
                            return Ok(newengine_plugin_host::IncrementalLoadOutcome {
                                finished: false,
                                loaded_total: loaded,
                                loaded_this_phase: 0,
                                pending_total: 0,
                                completed: 0,
                                load_errors: 1,
                                progress_01: self.plugin_discovery_root_index as f32
                                    / roots.len() as f32,
                                current_path: None,
                            });
                        }
                    }
                } else {
                    let mut pending = plugin_discovery_pending_outcome(
                        self.plugins.snapshot().len(),
                        Some(task.dir().clone()),
                    );
                    pending.progress_01 = root_index as f32 / roots.len() as f32;
                    return Ok(pending);
                }
            }
        }

        let mut outcome = self
            .plugins
            .load_engine_from_dir_incremental_step_with_origin(
                &discovery_dir,
                host.clone(),
                strict,
                root.origin,
            )
            .map_err(|error| {
                EngineError::Other(format!(
                    "plugins: incremental engine load failed owner='{}' origin='{}' root='{}': {error}",
                    root.owner,
                    root.origin.as_str(),
                    root.dir.display(),
                ))
            })?;

        install_forward_logger_once(host);
        if bootstrap_preload_deferred() {
            emit_startup_logs_after_logger_ready();
        }

        let root_progress = outcome.progress_01.clamp(0.0, 1.0);
        outcome.progress_01 = (root_index as f32 + root_progress) / roots.len().max(1) as f32;

        if outcome.finished {
            self.plugin_discovery_root_index += 1;
            if self.plugin_discovery_root_index < roots.len() {
                outcome.finished = false;
                outcome.progress_01 = self.plugin_discovery_root_index as f32 / roots.len() as f32;
                outcome.current_path = None;
                newengine_ulog_api::ulog::info!(
                    "plugins: discovery root complete root={}/{} owner='{}' origin='{}'; advancing to next root",
                    root_index + 1,
                    roots.len(),
                    root.owner,
                    root.origin.as_str(),
                );
                return Ok(outcome);
            }

            self.validate_required_plugins_loaded()?;
            self.engine_plugins_loaded = true;
            self.plugins_loaded = true;
            self.expose_plugins_snapshot();

            let loaded = self.plugins.snapshot().len();
            let event = EngineLifecycleEvent::EnginePluginsReady {
                loaded_count: loaded,
                origin: "load_engine_plugins_incremental_step",
            };
            self.mark_readiness_observed(&event);
            self.events.publish(event)?;
            newengine_ulog_api::ulog::info!(
                "plugins: done (phase=engine-load-incremental roots={} count={})",
                roots.len(),
                loaded
            );
            self.log_plugins_diagnostics("after incremental engine plugins init");
            outcome.progress_01 = 1.0;
        }

        Ok(outcome)
    }

    /// Loads plugins once (idempotent).
    ///
    /// The engine core never hardcodes plugin categories (assets, input, render, etc.).
    /// Any capability registration and secondary loading is owned by plugins.
    #[inline]
    pub fn load_plugins_once(&mut self) -> EngineResult<()> {
        self.try_load_plugins_once()
    }

    #[inline]
    pub(crate) fn try_load_plugins_once(&mut self) -> EngineResult<()> {
        if self.plugins_loaded {
            newengine_ulog_api::ulog::debug!("plugins: load skipped (already loaded)");
            return Ok(());
        }

        let strict = matches!(self.plugin_fault_tolerance, PluginFaultTolerance::Strict);
        let phase = "load";
        let t0 = Instant::now();
        let host = default_host_api();

        let roots = self.resolved_plugin_discovery_roots()?;
        for root in roots {
            if let Err(error) = self.plugins.load_from_dir_with_policy_and_origin(
                &root.dir,
                host.clone(),
                strict,
                root.origin,
            ) {
                match self.plugin_fault_tolerance {
                    PluginFaultTolerance::Strict => {
                        install_forward_logger_once(host);
                        return Err(EngineError::Other(format!(
                            "plugins: load failed (phase={} {} owner='{}' origin='{}' root='{}'): {error}",
                            phase,
                            Self::elapsed_since(t0),
                            root.owner,
                            root.origin.as_str(),
                            root.dir.display(),
                        )));
                    }
                    PluginFaultTolerance::Resilient => {
                        newengine_ulog_api::ulog::warn!(
                            "plugins: non-fatal root load error (phase={} {} owner='{}' origin='{}' root='{}'): {}",
                            phase,
                            Self::elapsed_since(t0),
                            root.owner,
                            root.origin.as_str(),
                            root.dir.display(),
                            error,
                        );
                    }
                }
            }
        }

        install_forward_logger_once(host);

        self.validate_required_plugins_loaded()?;
        self.plugins_loaded = true;
        self.expose_plugins_snapshot();

        let loaded = self.plugins.snapshot().len();
        let event = EngineLifecycleEvent::EnginePluginsReady {
            loaded_count: loaded,
            origin: "load_plugins_once",
        };
        self.mark_readiness_observed(&event);
        self.events.publish(event)?;
        Self::log_phase_ok("plugins", phase, Some(loaded), Self::elapsed_since(t0));

        Ok(())
    }
}
