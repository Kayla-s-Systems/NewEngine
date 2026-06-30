use super::super::{Engine, PluginFaultTolerance};

use crate::error::{EngineError, EngineResult};
use crate::plugin_forward_logger::install_forward_logger_once;
use newengine_plugin_host::{default_host_api, PluginControlCommand, PluginControlQueue};

use std::time::Instant;

impl<E: Send + 'static> Engine<E> {
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
                    Some(dir) => {
                        self.plugins
                            .load_from_dir_with_policy(dir, host.clone(), strict)
                    }
                    None => self.plugins.load_default_with_policy(host.clone(), strict),
                };

                install_forward_logger_once(host);

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

                        newengine_ulog_api::ulog::warn!(
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

                match self.plugins.load_path(&path, host.clone()) {
                    Ok(()) => {
                        install_forward_logger_once(host);
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

                match self.plugins.reload_by_id(&id, host.clone()) {
                    Ok(true) => {
                        install_forward_logger_once(host);
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
}
