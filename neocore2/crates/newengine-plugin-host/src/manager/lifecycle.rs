use newengine_plugin_api::HostApiV1;
use std::time::Instant;

use crate::host_context::{
    shutdown_services_by_owner, unregister_by_owner, with_current_plugin_id,
};

use super::types::{rresult_unit_to_string, PluginState};
use super::{PluginLoadError, PluginManager};

fn runtime_dll_unload_enabled() -> bool {
    matches!(
        std::env::var("NEWENGINE_UNLOAD_RUNTIME_DLLS")
            .ok()
            .as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn plugin_module_shutdown_enabled() -> bool {
    !matches!(
        std::env::var("NEWENGINE_DISABLE_PLUGIN_MODULE_SHUTDOWN")
            .ok()
            .as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

impl PluginManager {
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &newengine_plugin_api::PluginModuleDyn<'static>> {
        self.loaded.iter().map(|p| p.module.module_ref())
    }

    fn emit_provider_shutdown_started(&self, id: &str, reason: &str) {
        let host = crate::host_api::default_host_api();
        crate::ulog_event::emit_ulog_event(
            &host,
            "engine.provider.shutdown.started",
            "INFO",
            "Provider shutdown started",
            serde_json::json!({ "provider_id": id, "reason": reason }),
        );
    }

    fn emit_provider_shutdown_completed(&self, id: &str, reason: &str, elapsed_ms: f64) {
        let host = crate::host_api::default_host_api();
        crate::ulog_event::emit_ulog_event(
            &host,
            "engine.provider.shutdown.completed",
            "INFO",
            "Provider shutdown completed",
            serde_json::json!({ "provider_id": id, "reason": reason, "elapsed_ms": elapsed_ms }),
        );
    }

    fn emit_provider_shutdown_failed(&self, id: &str, reason: &str, elapsed_ms: f64, error: &str) {
        let host = crate::host_api::default_host_api();
        crate::ulog_event::emit_ulog_event(
            &host,
            "engine.provider.shutdown.failed",
            "ERROR",
            "Provider shutdown failed",
            serde_json::json!({ "provider_id": id, "reason": reason, "elapsed_ms": elapsed_ms, "error": error }),
        );
    }

    pub fn start_all(&mut self) -> Result<(), String> {
        for i in 0..self.loaded.len() {
            if self.loaded[i].state != PluginState::Registered {
                continue;
            }
            self.start_plugin_inline(i);
        }
        Ok(())
    }

    fn start_plugin_inline(&mut self, idx: usize) {
        if idx >= self.loaded.len() {
            return;
        }
        if self.loaded[idx].state == PluginState::Disabled {
            return;
        }

        let id = self.loaded[idx].info.id.to_string();
        let started = Instant::now();
        newengine_ulog_api::ulog::info!("plugins: start begin id='{}'", id);

        // Startup must never depend on synchronous diagnostics/event-sink work.
        // `start` is the transition that runs immediately after loading
        // state has already loaded engine plugins; emitting per-plugin job events
        // here can re-enter loading/profiler sinks while the plugin manager is still
        // mutating lifecycle state. Keep this phase direct and bounded: call the
        // plugin, record a normal log, then mark the plugin Running. Other lifecycle
        // operations still use the diagnostics bridge through `call_plugin`.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_current_plugin_id(&id, || {
                rresult_unit_to_string(self.loaded[idx].module.start())
            })
        }));

        match result {
            Ok(Ok(())) => {
                if idx < self.loaded.len() && self.loaded[idx].state == PluginState::Registered {
                    self.loaded[idx].state = PluginState::Running;
                }
                newengine_ulog_api::ulog::info!(
                    "plugins: start complete id='{}' elapsed_ms={:.3}",
                    id,
                    crate::diagnostics::elapsed_ms(started)
                );
            }
            Ok(Err(e)) => {
                newengine_ulog_api::ulog::error!("plugins: start failed id='{}': {}", id, e);
                self.disable_plugin(idx, &id, format!("op 'start' failed: {e}"));
            }
            Err(_) => {
                newengine_ulog_api::ulog::error!(
                    "plugins: panic during start id='{}' (plugin disabled)",
                    id
                );
                self.disable_plugin(idx, &id, "panic during op 'start'".to_owned());
            }
        }
    }

    pub fn fixed_update_all(&mut self, dt: f32) -> Result<(), String> {
        for i in 0..self.loaded.len() {
            if self.loaded[i].state != PluginState::Running {
                continue;
            }
            self.call_plugin(i, "fixed_update", |m| {
                rresult_unit_to_string(m.fixed_update(dt))
            });
        }
        Ok(())
    }

    pub fn update_all(&mut self, dt: f32) -> Result<(), String> {
        for i in 0..self.loaded.len() {
            if self.loaded[i].state != PluginState::Running {
                continue;
            }
            self.call_plugin(i, "update", |m| rresult_unit_to_string(m.update(dt)));
        }
        Ok(())
    }

    pub fn render_all(&mut self, dt: f32) -> Result<(), String> {
        for i in 0..self.loaded.len() {
            if self.loaded[i].state != PluginState::Running {
                continue;
            }
            self.call_plugin(i, "render", |m| rresult_unit_to_string(m.render(dt)));
        }
        Ok(())
    }

    pub fn shutdown(&mut self) {
        let retain_libraries = !runtime_dll_unload_enabled();
        newengine_ulog_api::ulog::info!(
            "plugins shutdown: begin count={} dll_policy='{}'",
            self.loaded.len(),
            if retain_libraries {
                "process_lifetime"
            } else {
                "unload"
            }
        );

        let call_module_shutdown = plugin_module_shutdown_enabled();

        for i in (0..self.loaded.len()).rev() {
            let id = self.loaded[i].info.id.to_string();
            newengine_ulog_api::ulog::info!("plugins shutdown: plugin begin id='{}'", id);
            shutdown_services_by_owner(&id, "plugin-manager.shutdown");
            if call_module_shutdown {
                newengine_ulog_api::ulog::debug!(
                    "plugins shutdown: module.shutdown begin id='{}'",
                    id
                );
                self.safe_shutdown_one_reason(i, "plugin-manager.shutdown");
                newengine_ulog_api::ulog::debug!(
                    "plugins shutdown: module.shutdown complete id='{}'",
                    id
                );
            } else {
                newengine_ulog_api::ulog::debug!(
                    "plugins shutdown: module.shutdown disabled id='{}' reason='NEWENGINE_DISABLE_PLUGIN_MODULE_SHUTDOWN is set'",
                    id
                );
            }
            self.loaded[i].state = PluginState::Stopped;
            unregister_by_owner(&id);
            newengine_ulog_api::ulog::info!("plugins shutdown: plugin complete id='{}'", id);
        }

        let loaded = std::mem::take(&mut self.loaded);
        for plugin in loaded {
            plugin.drop_with_library_policy(retain_libraries);
        }
        self.loaded_ids.clear();

        newengine_ulog_api::ulog::info!(
            "plugins shutdown: complete dll_policy='{}'",
            if retain_libraries {
                "process_lifetime"
            } else {
                "unload"
            }
        );
    }

    pub fn stop_by_id(&mut self, id: &str) -> bool {
        let Some(idx) = self.find_index(id) else {
            return false;
        };

        if self.loaded[idx].state == PluginState::Stopped {
            return true;
        }

        shutdown_services_by_owner(id, "plugin-manager.stop_by_id");
        self.safe_shutdown_one_reason(idx, "plugin-manager.stop_by_id");
        self.loaded[idx].state = PluginState::Stopped;
        unregister_by_owner(id);
        true
    }

    pub fn disable_by_id(&mut self, id: &str, reason: impl Into<String>) -> bool {
        let Some(idx) = self.find_index(id) else {
            return false;
        };
        self.disable_plugin(idx, id, reason.into());
        true
    }

    pub fn unload_by_id(&mut self, id: &str) -> bool {
        let Some(idx) = self.find_index(id) else {
            return false;
        };
        self.unload_at(idx);
        true
    }

    pub fn reload_by_id(&mut self, id: &str, host: HostApiV1) -> Result<bool, PluginLoadError> {
        let Some(idx) = self.find_index(id) else {
            return Ok(false);
        };

        let path = self.loaded[idx].path.clone();
        self.unload_at(idx);
        self.load_one(&path, host)?;
        Ok(true)
    }

    pub fn start_by_id(&mut self, id: &str) -> bool {
        let Some(idx) = self.find_index(id) else {
            return false;
        };

        match self.loaded[idx].state {
            PluginState::Registered | PluginState::Stopped => {
                self.call_plugin(idx, "start", |m| rresult_unit_to_string(m.start()));
                true
            }
            _ => true,
        }
    }

    fn unload_at(&mut self, idx: usize) {
        if idx >= self.loaded.len() {
            return;
        }

        let id = self.loaded[idx].info.id.to_string();
        shutdown_services_by_owner(&id, "plugin-manager.unload_at");
        self.safe_shutdown_one_reason(idx, "plugin-manager.unload_at");
        unregister_by_owner(&id);
        self.loaded_ids.remove(&id);
        self.loaded.remove(idx);
    }

    fn call_plugin(
        &mut self,
        idx: usize,
        op: &str,
        f: impl FnOnce(&mut super::adapter::ModuleAdapterAny) -> Result<(), String>,
    ) {
        if idx >= self.loaded.len() {
            return;
        }
        if self.loaded[idx].state == PluginState::Disabled {
            return;
        }

        let id = self.loaded[idx].info.id.to_string();
        let started = Instant::now();

        // Hot-path rule: plugin lifecycle calls are direct ABI calls, not JSON
        // diagnostics jobs. JSON lifecycle envelopes made fixed_update/update/render
        // more expensive than most no-op plugin work. Slow/error paths still log a
        // compact text diagnostic; structured profiler export must be fed by typed
        // engine.jobs/task events, not serde_json on every plugin callback.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_current_plugin_id(&id, || f(&mut self.loaded[idx].module))
        }));

        match result {
            Ok(Ok(())) => {
                let elapsed_ms = crate::diagnostics::elapsed_ms(started);
                if elapsed_ms >= 4.0 {
                    newengine_ulog_api::ulog::debug!(
                        "plugins: lifecycle slow id='{}' op='{}' elapsed_ms={:.3}",
                        id,
                        op,
                        elapsed_ms
                    );
                }
            }
            Ok(Err(e)) => {
                newengine_ulog_api::ulog::error!(
                    "plugins: op '{}' failed for id='{}' elapsed_ms={:.3}: {}",
                    op,
                    id,
                    crate::diagnostics::elapsed_ms(started),
                    e
                );
                self.disable_plugin(idx, &id, format!("op '{op}' failed: {e}"));
            }
            Err(_) => {
                newengine_ulog_api::ulog::error!(
                    "plugins: panic during op '{}' for id='{}' elapsed_ms={:.3} (plugin disabled)",
                    op,
                    id,
                    crate::diagnostics::elapsed_ms(started)
                );
                self.disable_plugin(idx, &id, format!("panic during op '{op}'"));
            }
        }

        if idx < self.loaded.len()
            && op == "start"
            && self.loaded[idx].state == PluginState::Registered
        {
            self.loaded[idx].state = PluginState::Running;
        }
    }

    fn disable_plugin(&mut self, idx: usize, id: &str, reason: String) {
        if idx >= self.loaded.len() || self.loaded[idx].state == PluginState::Disabled {
            return;
        }

        self.loaded[idx].state = PluginState::Disabled;
        self.loaded[idx].disabled_reason = Some(reason);

        shutdown_services_by_owner(id, "plugin-manager.disable_plugin");
        self.safe_shutdown_one_reason(idx, "plugin-manager.disable_plugin");
        unregister_by_owner(id);
    }

    fn safe_shutdown_one_reason(&mut self, idx: usize, reason: &str) -> bool {
        if idx >= self.loaded.len() {
            return false;
        }

        let id = self.loaded[idx].info.id.to_string();
        let started = Instant::now();
        self.emit_provider_shutdown_started(&id, reason);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_current_plugin_id(&id, || self.loaded[idx].module.shutdown())
        }));
        let elapsed_ms = crate::diagnostics::elapsed_ms(started);
        match result {
            Ok(()) => {
                self.emit_provider_shutdown_completed(&id, reason, elapsed_ms);
                true
            }
            Err(_) => {
                self.emit_provider_shutdown_failed(
                    &id,
                    reason,
                    elapsed_ms,
                    "panic during module.shutdown",
                );
                false
            }
        }
    }
}
