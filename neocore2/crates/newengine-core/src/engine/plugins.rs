#![forbid(unsafe_op_in_unsafe_fn)]

use super::{Engine, PluginFaultTolerance};

use crate::error::{EngineError, EngineResult};
use crate::log_fmt::{ellipsize, emit_boxed_kv, emit_prefixed_table};
use crate::path_fmt::display_clean;
use crate::plugins::{
    default_host_api, PluginControlCommand, PluginControlQueue, PluginsSnapshot,
};

use std::time::Instant;

impl<E: Send + 'static> Engine<E> {
    pub fn preload_bootstrap_plugins(&mut self) -> EngineResult<()> {
        let strict = matches!(self.plugin_fault_tolerance, PluginFaultTolerance::Strict);
        let host = default_host_api();

        let res = match self.plugins_dir.as_deref() {
            Some(dir) => self.plugins.load_bootstrap_from_dir(dir, host, strict),
            None => self.plugins.load_bootstrap_default(host, strict),
        };

        match res {
            Ok(()) => Ok(()),
            Err(e) => Err(EngineError::Other(format!(
                "plugins: bootstrap load failed: {e}"
            ))),
        }
    }

    pub fn load_engine_plugins_once(&mut self) -> EngineResult<usize> {
        if self.engine_plugins_loaded {
            log::debug!("plugins: engine load skipped (already loaded)");
            return Ok(0);
        }

        let strict = matches!(self.plugin_fault_tolerance, PluginFaultTolerance::Strict);
        let phase = "engine-load";
        let t0 = Instant::now();
        let host = default_host_api();

        let load_result = match self.plugins_dir.as_deref() {
            Some(dir) => self.plugins.load_engine_from_dir(dir, host, strict),
            None => self.plugins.load_engine_default(host, strict),
        };

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
                    log::warn!(
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

        let loaded = self.plugins.snapshot().len();
        Self::log_phase_ok("plugins", phase, Some(loaded), Self::elapsed_since(t0));
        self.log_plugins_diagnostics("after engine plugins init");

        Ok(loaded)
    }

    #[inline]
    pub fn emit_plugins_diagnostics(&self, tag: &'static str) {
        self.log_plugins_diagnostics(tag);
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
            log::debug!("plugins: load skipped (already loaded)");
            return Ok(());
        }

        let strict = matches!(self.plugin_fault_tolerance, PluginFaultTolerance::Strict);
        let phase = "load";
        let t0 = Instant::now();
        let host = default_host_api();

        let load_result = match self.plugins_dir.as_deref() {
            Some(dir) => self.plugins.load_from_dir_with_policy(dir, host, strict),
            None => self.plugins.load_default_with_policy(host, strict),
        };

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
                    log::warn!(
                        "plugins: non-fatal load error (phase={} {}): {}",
                        phase,
                        Self::elapsed_since(t0),
                        e
                    );
                }
            }
        }

        self.plugins_loaded = true;

        let loaded = self.plugins.snapshot().len();
        Self::log_phase_ok("plugins", phase, Some(loaded), Self::elapsed_since(t0));

        Ok(())
    }

    #[inline]
    pub(crate) fn log_plugins_diagnostics(&self, tag: &'static str) {
        let list = self.plugins.snapshot();
        let n = list.len();

        emit_boxed_kv(
            &format!("Plugins :: Diagnostics [{}]", tag),
            &[("loaded", n.to_string())],
        );

        if list.is_empty() {
            return;
        }

        let rows: Vec<Vec<String>> = list
            .iter()
            .map(|p| {
                vec![
                    ellipsize(&p.id, 32),
                    ellipsize(&p.version, 12),
                    ellipsize(&p.state, 16),
                    format!("{:?}", p.kind),
                    p.capabilities.len().to_string(),
                ]
            })
            .collect();

        emit_prefixed_table(
            "",
            &format!("Plugins :: Registered [{}]", tag),
            &["id", "ver", "state", "kind", "caps"],
            &rows,
        );

        if log::log_enabled!(log::Level::Debug) {
            for p in &list {
                log::debug!(
                    "plugins: path id='{}' caps={} path='{}'",
                    p.id,
                    p.capabilities.len(),
                    display_clean(&p.path)
                );
            }
        }
    }

    pub(crate) fn process_plugin_control(&mut self) -> EngineResult<()> {
        let Some(queue) = self.resources.get_mut::<PluginControlQueue>() else {
            return Ok(());
        };

        let strict = matches!(self.plugin_fault_tolerance, PluginFaultTolerance::Strict);

        let mut last_action: Option<String> = None;
        let mut last_error: Option<String> = None;
        let mut did_any = false;

        for cmd in queue.drain() {
            did_any = true;

            let host = default_host_api();

            match cmd {
                PluginControlCommand::Rescan => {
                    let phase = "rescan";
                    let t0 = Instant::now();

                    self.plugins.invalidate_discovery_cache();

                    let res = match self.plugins_dir.as_deref() {
                        Some(dir) => self.plugins.load_from_dir_with_policy(dir, host, strict),
                        None => self.plugins.load_default_with_policy(host, strict),
                    };

                    match res {
                        Ok(()) => {
                            last_action = Some("plugins: rescan".to_owned());
                            let loaded = self.plugins.snapshot().len();
                            Self::log_phase_ok(
                                "plugins",
                                phase,
                                Some(loaded),
                                Self::elapsed_since(t0),
                            );
                        }
                        Err(e) => {
                            last_error = Some(format!(
                                "plugins: rescan failed ({}): {e}",
                                Self::elapsed_since(t0)
                            ));

                            if strict {
                                return Err(EngineError::Other(
                                    last_error.clone().unwrap_or_else(|| e.to_string()),
                                ));
                            }

                            log::warn!(
                                "plugins: non-fatal rescan error (phase={} {}): {}",
                                phase,
                                Self::elapsed_since(t0),
                                e
                            );
                        }
                    }
                }

                PluginControlCommand::LoadPath(path) => {
                    let phase = "load_path";
                    let t0 = Instant::now();

                    match self.plugins.load_path(&path, host) {
                        Ok(()) => {
                            last_action = Some(format!("plugins: load '{}'", path.display()));
                            let loaded = self.plugins.snapshot().len();
                            Self::log_phase_ok(
                                "plugins",
                                phase,
                                Some(loaded),
                                Self::elapsed_since(t0),
                            );
                        }
                        Err(e) => {
                            last_error = Some(format!(
                                "plugins: load failed path='{}' ({}): {e}",
                                path.display(),
                                Self::elapsed_since(t0)
                            ));

                            if strict {
                                return Err(EngineError::Other(
                                    last_error.clone().unwrap_or_else(|| e.to_string()),
                                ));
                            }
                        }
                    }
                }

                PluginControlCommand::ReloadId(id) | PluginControlCommand::EnableId(id) => {
                    let phase = "reload";
                    let t0 = Instant::now();

                    match self.plugins.reload_by_id(&id, host) {
                        Ok(true) => {
                            self.plugins.start_by_id(&id);
                            last_action = Some(format!("plugins: reloaded id='{}'", id));
                            let loaded = self.plugins.snapshot().len();
                            Self::log_phase_ok(
                                "plugins",
                                phase,
                                Some(loaded),
                                Self::elapsed_since(t0),
                            );
                        }
                        Ok(false) => {
                            last_error = Some(format!("plugins: unknown id='{}'", id));
                        }
                        Err(e) => {
                            last_error = Some(format!(
                                "plugins: reload failed id='{}' ({}): {e}",
                                id,
                                Self::elapsed_since(t0)
                            ));

                            if strict {
                                return Err(EngineError::Other(
                                    last_error.clone().unwrap_or_else(|| e.to_string()),
                                ));
                            }
                        }
                    }
                }

                PluginControlCommand::StartId(id) => {
                    if self.plugins.start_by_id(&id) {
                        last_action = Some(format!("plugins: start id='{}'", id));
                    } else {
                        last_error = Some(format!("plugins: unknown id='{}'", id));
                    }
                }

                PluginControlCommand::StopId(id) => {
                    if self.plugins.stop_by_id(&id) {
                        last_action = Some(format!("plugins: stop id='{}'", id));
                    } else {
                        last_error = Some(format!("plugins: unknown id='{}'", id));
                    }
                }

                PluginControlCommand::DisableId(id) => {
                    if self
                        .plugins
                        .disable_by_id(&id, "manually disabled via control plane")
                    {
                        last_action = Some(format!("plugins: disable id='{}'", id));
                    } else {
                        last_error = Some(format!("plugins: unknown id='{}'", id));
                    }
                }
            }
        }

        if did_any {
            queue.result.last_action = last_action;
            queue.result.last_error = last_error;
        }

        Ok(())
    }

    #[inline]
    pub(crate) fn expose_plugins_snapshot(&mut self) {
        self.resources.insert(PluginsSnapshot {
            plugins: self.plugins.snapshot(),
        });
    }

    #[inline]
    pub(crate) fn plugins_start_all(&mut self) -> EngineResult<()> {
        self.plugins
            .start_all()
            .map_err(|e| EngineError::Other(format!("plugins: start failed: {e}")))
    }

    #[inline]
    pub(crate) fn plugins_shutdown(&mut self) {
        self.plugins.shutdown();
    }
}