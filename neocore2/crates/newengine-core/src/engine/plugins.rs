use super::Engine;

use crate::error::{EngineError, EngineResult};
use crate::plugins::{default_host_api, PluginControlCommand, PluginControlQueue, PluginsSnapshot};

use std::time::Instant;

impl<E: Send + 'static> Engine<E> {
    /// Loads plugins once (idempotent).
    ///
    /// The engine core never hardcodes plugin categories (assets, input, render, etc.).
    /// Any capability registration and secondary loading (e.g. importers) is owned by plugins.
    #[inline]
    pub fn load_plugins_once(&mut self) -> EngineResult<()> {
        self.try_load_plugins_once()
    }

    pub(super) fn try_load_plugins_once(&mut self) -> EngineResult<()> {
        if self.plugins_loaded {
            log::debug!("plugins: load skipped (already loaded)");
            return Ok(());
        }

        let phase = "load";
        let t0 = Instant::now();

        let host = default_host_api();
        let load_result = match self.plugins_dir.as_deref() {
            Some(dir) => self.plugins.load_from_dir(dir, host),
            None => self.plugins.load_default(host),
        };

        if let Err(e) = load_result {
            log::warn!(
                "plugins: non-fatal load error (phase={} {}): {}",
                phase,
                Self::elapsed_since(t0),
                e
            );
        }

        // Mark as loaded even if some plugins failed to load (non-fatal path).
        self.plugins_loaded = true;

        let loaded = self.plugins.snapshot().len();
        // Emit diagnostics after `load_*` so an optional logging plugin can install
        // the global `log` backend during its `init()`.
        Self::log_phase_ok("plugins", phase, Some(loaded), Self::elapsed_since(t0));

        Ok(())
    }

    pub(super) fn log_plugins_diagnostics(&self, tag: &'static str) {
        let list = self.plugins.snapshot();
        let n = list.len();
        log::info!("plugins: diagnostics tag='{}' loaded={}", tag, n);

        // Keep INFO concise and stable.
        for (i, p) in list.iter().enumerate() {
            log::info!(
                "plugins: diag [{:02}/{:02}] id='{}' ver='{}' state='{}'",
                i.saturating_add(1),
                n.max(1),
                p.id,
                p.version,
                p.state
            );
        }

        if log::log_enabled!(log::Level::Debug) {
            for p in list.iter() {
                log::debug!(
                    "plugins: diag.debug id='{}' ver='{}' kind={:?} caps={} path='{}'",
                    p.id,
                    p.version,
                    p.kind,
                    p.capabilities.len(),
                    p.path.display()
                );
            }
        }
    }

    pub(super) fn process_plugin_control(&mut self) {
        let Some(queue) = self.resources.get_mut::<PluginControlQueue>() else {
            return;
        };

        let mut did_any = false;
        let mut last_action: Option<String> = None;
        let mut last_error: Option<String> = None;

        for cmd in queue.drain() {
            did_any = true;

            // Host API is cheap, but do not re-create it multiple times per command.
            // Create per command to avoid any lifetime/aliasing surprises and keep semantics clean.
            let host = default_host_api();

            match cmd {
                PluginControlCommand::Rescan => {
                    let phase = "rescan";
                    let t0 = Instant::now();

                    let dir = self.plugins_dir.clone();
                    let res = match dir.as_deref() {
                        Some(d) => self.plugins.load_from_dir(d, host),
                        None => self.plugins.load_default(host),
                    };

                    match res {
                        Ok(()) => {
                            let loaded = self.plugins.snapshot().len();
                            last_action = Some("plugins: rescan".to_string());
                            Self::log_phase_ok("plugins", phase, Some(loaded), Self::elapsed_since(t0));
                        }
                        Err(e) => {
                            last_error = Some(format!(
                                "plugins: rescan failed ({}): {e}",
                                Self::elapsed_since(t0)
                            ));
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
                            Self::log_phase_ok("plugins", phase, Some(loaded), Self::elapsed_since(t0));
                        }
                        Err(e) => {
                            last_error = Some(format!(
                                "plugins: load failed path='{}' ({}): {e}",
                                path.display(),
                                Self::elapsed_since(t0)
                            ));
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
                            Self::log_phase_ok("plugins", phase, Some(loaded), Self::elapsed_since(t0));
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
    }

    pub(super) fn expose_plugins_snapshot(&mut self) {
        self.resources.insert(PluginsSnapshot {
            plugins: self.plugins.snapshot(),
        });
    }

    pub(super) fn plugins_start_all(&mut self) -> EngineResult<()> {
        if let Err(e) = self.plugins.start_all() {
            return Err(EngineError::Other(format!("plugins: start failed: {e}")));
        }
        Ok(())
    }

    pub(super) fn plugins_shutdown(&mut self) {
        self.plugins.shutdown();
    }
}