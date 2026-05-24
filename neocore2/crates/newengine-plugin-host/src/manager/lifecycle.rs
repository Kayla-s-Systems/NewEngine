use newengine_plugin_api::HostApiV1;
use std::time::Instant;

use crate::host_context::{shutdown_services_by_owner, unregister_by_owner, with_current_plugin_id};

use super::types::{rresult_unit_to_string, PluginState};
use super::{PluginLoadError, PluginManager};

fn runtime_dll_unload_enabled() -> bool {
    matches!(
        std::env::var("NEWENGINE_UNLOAD_RUNTIME_DLLS").ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn plugin_module_shutdown_enabled() -> bool {
    !matches!(
        std::env::var("NEWENGINE_DISABLE_PLUGIN_MODULE_SHUTDOWN").ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

impl PluginManager {
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item=&newengine_plugin_api::PluginModuleDyn<'static>> {
        self.loaded.iter().filter_map(|p| p.module.as_v1())
    }

    pub fn start_all(&mut self) -> Result<(), String> {
        for i in 0..self.loaded.len() {
            if self.loaded[i].state != PluginState::Registered {
                continue;
            }
            self.call_plugin(i, "start", |m| rresult_unit_to_string(m.start()));
        }
        Ok(())
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
        log::info!(
            "plugins shutdown: begin count={} dll_policy='{}'",
            self.loaded.len(),
            if retain_libraries { "process_lifetime" } else { "unload" }
        );

        let call_module_shutdown = plugin_module_shutdown_enabled();

        for i in (0..self.loaded.len()).rev() {
            let id = self.loaded[i].info.id.to_string();
            log::info!("plugins shutdown: plugin begin id='{}'", id);
            shutdown_services_by_owner(&id, "plugin-manager.shutdown");
            if call_module_shutdown {
                log::debug!("plugins shutdown: module.shutdown begin id='{}'", id);
                self.safe_shutdown_one(i);
                log::debug!("plugins shutdown: module.shutdown complete id='{}'", id);
            } else {
                log::debug!(
                    "plugins shutdown: module.shutdown disabled id='{}' reason='NEWENGINE_DISABLE_PLUGIN_MODULE_SHUTDOWN is set'",
                    id
                );
            }
            self.loaded[i].state = PluginState::Stopped;
            unregister_by_owner(&id);
            log::info!("plugins shutdown: plugin complete id='{}'", id);
        }

        let loaded = std::mem::take(&mut self.loaded);
        for plugin in loaded {
            plugin.drop_with_library_policy(retain_libraries);
        }
        self.loaded_ids.clear();

        log::info!(
            "plugins shutdown: complete dll_policy='{}'",
            if retain_libraries { "process_lifetime" } else { "unload" }
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
        self.safe_shutdown_one(idx);
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
        self.safe_shutdown_one(idx);
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
        let job_id = crate::diagnostics::next_job_id("host.plugin_lifecycle");
        let started = Instant::now();

        crate::diagnostics::begin(serde_json::json!({
            "id": job_id.clone(),
            "name": format!("plugin:{}::{}", id, op),
            "category": "plugin_lifecycle",
            "source": "newengine-plugin-host",
            "plugin_id": id.clone(),
            "operation": op,
            "metadata": {
                "plugin_id": id.clone(),
                "operation": op
            }
        }));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_current_plugin_id(&id, || f(&mut self.loaded[idx].module))
        }));

        match result {
            Ok(Ok(())) => {
                crate::diagnostics::end(serde_json::json!({
                    "id": job_id.clone(),
                    "status": "completed",
                    "detail": format!(
                        "plugin lifecycle op completed in {:.3} ms",
                        crate::diagnostics::elapsed_ms(started)
                    ),
                    "metadata": {
                        "plugin_id": id.clone(),
                        "operation": op
                    }
                }));
            }
            Ok(Err(e)) => {
                crate::diagnostics::end(serde_json::json!({
                    "id": job_id.clone(),
                    "status": "failed",
                    "error": e.clone(),
                    "detail": format!(
                        "plugin lifecycle op failed in {:.3} ms",
                        crate::diagnostics::elapsed_ms(started)
                    ),
                    "metadata": {
                        "plugin_id": id.clone(),
                        "operation": op
                    }
                }));
                log::error!("plugins: op '{}' failed for id='{}': {}", op, id, e);
                self.disable_plugin(idx, &id, format!("op '{op}' failed: {e}"));
            }
            Err(_) => {
                crate::diagnostics::end(serde_json::json!({
                    "id": job_id.clone(),
                    "status": "failed",
                    "error": "panic",
                    "detail": format!(
                        "plugin lifecycle op panicked in {:.3} ms",
                        crate::diagnostics::elapsed_ms(started)
                    ),
                    "metadata": {
                        "plugin_id": id.clone(),
                        "operation": op
                    }
                }));
                log::error!(
                    "plugins: panic during op '{}' for id='{}' (plugin disabled)",
                    op,
                    id
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
        self.safe_shutdown_one(idx);
        unregister_by_owner(id);
    }

    fn safe_shutdown_one(&mut self, idx: usize) {
        if idx >= self.loaded.len() {
            return;
        }

        let id = self.loaded[idx].info.id.to_string();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_current_plugin_id(&id, || self.loaded[idx].module.shutdown())
        }));
    }
}
