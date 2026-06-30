use super::super::{Engine, PluginFaultTolerance};

use crate::error::{EngineError, EngineResult};
use crate::lifecycle_events::EngineLifecycleEvent;
use crate::plugin_forward_logger::install_forward_logger_once;
use newengine_plugin_host::default_host_api;

use std::time::Instant;

fn bootstrap_preload_deferred() -> bool {
    std::env::var("NEWENGINE_BOOTSTRAP_PLUGIN_PRELOAD")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "off" | "defer" | "deferred" | "safe"
            )
        })
        .unwrap_or(false)
}


fn emit_startup_logs_after_logger_ready() {
    let rid = crate::run_id::run_id().unwrap_or("<unknown>");
    newengine_ulog_api::ulog::info!("startup: Run ID: {}", rid);
    crate::startup::SystemProbe::probe().emit_table("startup");
    if let Some(r) = crate::startup::last_load_report() {
        r.emit_logs();
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

        let res = match self.plugins_dir.as_deref() {
            Some(dir) => self
                .plugins
                .load_bootstrap_from_dir(dir, host.clone(), strict),
            None => self.plugins.load_bootstrap_default(host.clone(), strict),
        };

        install_forward_logger_once(host);

        match res {
            Ok(()) => {
                emit_startup_logs_after_logger_ready();
                Ok(())
            }
            Err(e) => Err(EngineError::Other(format!(
                "plugins: bootstrap load failed: {e}"
            ))),
        }
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

        let load_result = match self.plugins_dir.as_deref() {
            Some(dir) => self.plugins.load_engine_from_dir(dir, host.clone(), strict),
            None => self.plugins.load_engine_default(host.clone(), strict),
        };

        install_forward_logger_once(host);
        if bootstrap_preload_deferred() {
            emit_startup_logs_after_logger_ready();
        }

        if let Err(e) = load_result {
            match self.plugin_fault_tolerance {
                PluginFaultTolerance::Strict => {
                    return Err(EngineError::Other(format!(
                        "plugins: engine load failed (phase={} {}): {e}",
                        phase,
                        Self::elapsed_since(t0)
                    )));
                }
                PluginFaultTolerance::Resilient => {
                    newengine_ulog_api::ulog::warn!(
                        "plugins: non-fatal engine load error (phase={} {}): {}",
                        phase,
                        Self::elapsed_since(t0),
                        e
                    );
                }
            }
        }

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

        let outcome = match self.plugins_dir.as_deref() {
            Some(dir) => {
                self.plugins
                    .load_engine_from_dir_incremental_step(dir, host.clone(), strict)
            }
            None => self
                .plugins
                .load_engine_default_incremental_step(host.clone(), strict),
        }
        .map_err(|e| EngineError::Other(format!("plugins: incremental engine load failed: {e}")))?;

        install_forward_logger_once(host);
        if bootstrap_preload_deferred() {
            emit_startup_logs_after_logger_ready();
        }

        if outcome.finished {
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
                "plugins: done (phase=engine-load-incremental count={})",
                loaded
            );
            self.log_plugins_diagnostics("after incremental engine plugins init");
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

        let load_result = match self.plugins_dir.as_deref() {
            Some(dir) => self
                .plugins
                .load_from_dir_with_policy(dir, host.clone(), strict),
            None => self.plugins.load_default_with_policy(host.clone(), strict),
        };

        install_forward_logger_once(host);

        if let Err(e) = load_result {
            match self.plugin_fault_tolerance {
                PluginFaultTolerance::Strict => {
                    return Err(EngineError::Other(format!(
                        "plugins: load failed (phase={} {}): {e}",
                        phase,
                        Self::elapsed_since(t0)
                    )));
                }
                PluginFaultTolerance::Resilient => {
                    newengine_ulog_api::ulog::warn!(
                        "plugins: non-fatal load error (phase={} {}): {}",
                        phase,
                        Self::elapsed_since(t0),
                        e
                    );
                }
            }
        }

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
