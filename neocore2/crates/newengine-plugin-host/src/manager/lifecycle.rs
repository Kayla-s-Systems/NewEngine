use newengine_plugin_api::HostApiV1;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::host_context::{
    quiesce_provider_publication, restore_provider_publication,
    shutdown_provider_publication_services, shutdown_services_by_owner,
    snapshot_provider_publication, unregister_by_owner, wait_for_provider_publication_quiescence,
    with_current_plugin_id,
};

use super::types::{rresult_unit_to_string, PluginState};
use super::{PluginLoadError, PluginManager};

fn runtime_dll_unload_enabled() -> bool {
    matches!(
        crate::host_context::environment_var("NEWENGINE_UNLOAD_RUNTIME_DLLS").as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn plugin_module_shutdown_enabled() -> bool {
    !matches!(
        crate::host_context::environment_var("NEWENGINE_DISABLE_PLUGIN_MODULE_SHUTDOWN").as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn prepare_hot_reload_shadow(source: &Path) -> Result<PathBuf, PluginLoadError> {
    let parent = source.parent().unwrap_or_else(|| Path::new("."));
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("plugin");
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("dll");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let shadow = parent.join(format!(
        ".{stem}.northstar-hotreload-{}-{stamp}.{extension}",
        std::process::id()
    ));
    std::fs::copy(source, &shadow).map_err(|error| PluginLoadError {
        path: source.to_path_buf(),
        message: format!(
            "failed to create hot-reload shadow '{}' from '{}': {error}",
            shadow.display(),
            source.display()
        ),
    })?;
    Ok(shadow)
}

#[inline]
fn root_exports_plugin_code(
    snapshot: Option<&crate::root_observers::LoadedPluginRootSnapshot>,
) -> bool {
    snapshot
        .and_then(|snapshot| snapshot.editor_extensions_v1)
        .is_some()
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
        crate::host_context::activate_host_context(&self.host);
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
                if idx < self.loaded.len()
                    && matches!(
                        self.loaded[idx].state,
                        PluginState::Registered | PluginState::Stopped
                    )
                {
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
        crate::host_context::activate_host_context(&self.host);
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
        crate::host_context::activate_host_context(&self.host);
        for i in 0..self.loaded.len() {
            if self.loaded[i].state != PluginState::Running {
                continue;
            }
            self.call_plugin(i, "update", |m| rresult_unit_to_string(m.update(dt)));
        }
        Ok(())
    }

    pub fn render_all(&mut self, dt: f32) -> Result<(), String> {
        crate::host_context::activate_host_context(&self.host);
        for i in 0..self.loaded.len() {
            if self.loaded[i].state != PluginState::Running {
                continue;
            }
            self.call_plugin(i, "render", |m| rresult_unit_to_string(m.render(dt)));
        }
        Ok(())
    }

    pub fn shutdown(&mut self) {
        crate::host_context::activate_host_context(&self.host);
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
        crate::host_context::activate_host_context(&self.host);
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
        crate::host_context::activate_host_context(&self.host);
        let Some(idx) = self.find_index(id) else {
            return false;
        };
        self.disable_plugin(idx, id, reason.into());
        true
    }

    pub fn unload_by_id(&mut self, id: &str) -> bool {
        crate::host_context::activate_host_context(&self.host);
        let Some(idx) = self.find_index(id) else {
            return false;
        };
        self.unload_at(idx);
        true
    }

    pub fn reload_by_id(&mut self, id: &str, host: HostApiV1) -> Result<bool, PluginLoadError> {
        crate::host_context::activate_host_context(&self.host);
        let Some(old_idx) = self.find_index(id) else {
            return Ok(false);
        };

        let source_path = self.loaded[old_idx].path.clone();
        let old_was_running = self.loaded[old_idx].state == PluginState::Running;
        let old_publication = snapshot_provider_publication(id);
        let old_root = crate::root_observers::loaded_plugin_root_snapshot(id);
        let shadow_path = prepare_hot_reload_shadow(&source_path)?;

        // Allow the replacement with the same provider id to pass loader duplicate checks while
        // the old module remains resident and continues serving the currently published topology.
        self.loaded_ids.remove(id);
        let loaded_len_before = self.loaded.len();
        if let Err(error) = self.load_one(&shadow_path, host) {
            self.loaded_ids.insert(id.to_owned());
            let _ = std::fs::remove_file(&shadow_path);
            return Err(error);
        }

        if self.loaded.len() == loaded_len_before {
            self.loaded_ids.insert(id.to_owned());
            let _ = std::fs::remove_file(&shadow_path);
            return Err(PluginLoadError {
                path: source_path,
                message: "hot-reload replacement was not loaded".to_owned(),
            });
        }

        let new_idx = self.loaded.len() - 1;
        let replacement_id = self.loaded[new_idx].info.id.to_string();
        if replacement_id != id {
            self.unload_at(new_idx);
            self.loaded_ids.insert(id.to_owned());
            return Err(PluginLoadError {
                path: source_path,
                message: format!(
                    "hot-reload provider id changed old='{}' replacement='{}'",
                    id, replacement_id
                ),
            });
        }

        // Discovery/rebuild continues to target the stable source file. The library itself is
        // mapped from the unique shadow path so Windows cannot hand us the old loaded image.
        self.loaded[new_idx].path = source_path.clone();

        let start_result = if old_was_running {
            self.try_start_replacement(new_idx)
        } else {
            Ok(())
        };

        if let Err(start_error) = start_result {
            let replacement_publication = snapshot_provider_publication(id);
            let replacement_root = crate::root_observers::loaded_plugin_root_snapshot(id);

            // Restore the previous topology first. This simultaneously retires replacement
            // services/sinks and re-opens admission on the old publication.
            restore_provider_publication(id, old_publication);
            if let Some(snapshot) = old_root.clone() {
                crate::root_observers::record_loaded_plugin_root(snapshot);
            } else {
                crate::root_observers::forget_loaded_plugin_root(id);
            }

            let drained = wait_for_provider_publication_quiescence(
                &replacement_publication,
                Duration::from_secs(2),
            );
            let replacement_has_raw_exports = root_exports_plugin_code(replacement_root.as_ref());

            if drained && !replacement_has_raw_exports {
                shutdown_provider_publication_services(
                    id,
                    &replacement_publication,
                    "plugin-manager.hot-reload.rollback",
                );
                self.safe_shutdown_one_reason(new_idx, "plugin-manager.hot-reload.rollback");
                drop(replacement_publication);
                let replacement = self.loaded.remove(new_idx);
                replacement.drop_with_library_policy(false);
            } else {
                newengine_ulog_api::ulog::warn!(
                    "plugins: hot-reload rollback retired replacement conservatively id='{}' drained={} raw_editor_exports={} policy='retain-mapped-state'",
                    id,
                    drained,
                    replacement_has_raw_exports
                );
                let replacement = self.loaded.remove(new_idx);
                std::mem::forget(replacement);
                std::mem::forget(replacement_publication);
            }
            self.loaded_ids.insert(id.to_owned());
            return Err(PluginLoadError {
                path: source_path,
                message: format!("hot-reload replacement start failed: {start_error}"),
            });
        }

        // The transactional commit already made the replacement active and marked the old
        // service/event admission gates as retired. Drain callbacks that entered before swap.
        let drained =
            wait_for_provider_publication_quiescence(&old_publication, Duration::from_secs(2));
        let old_has_raw_exports = root_exports_plugin_code(old_root.as_ref());

        if drained && !old_has_raw_exports {
            shutdown_provider_publication_services(
                id,
                &old_publication,
                "plugin-manager.hot-reload.retire",
            );
            self.safe_shutdown_one_reason(old_idx, "plugin-manager.hot-reload.retire");
            drop(old_publication);
            let old_plugin = self.loaded.remove(old_idx);
            old_plugin.drop_with_library_policy(false);
        } else {
            // A bounded reload must never deadlock waiting for a provider callback. If a call is
            // hung, or raw editor function pointers may have escaped, remove the old provider from
            // manager scheduling but keep its ABI state/library mapped. This is logical unload with
            // conservative physical reclamation rather than use-after-unload.
            newengine_ulog_api::ulog::warn!(
                "plugins: hot-reload old provider retired conservatively id='{}' drained={} raw_editor_exports={} policy='retain-mapped-state'",
                id,
                drained,
                old_has_raw_exports
            );
            let old_plugin = self.loaded.remove(old_idx);
            std::mem::forget(old_plugin);
            std::mem::forget(old_publication);
        }

        self.loaded_ids.insert(id.to_owned());
        newengine_ulog_api::ulog::info!(
            "plugins: hot-reload committed id='{}' source='{}' mapped='{}'",
            id,
            source_path.display(),
            shadow_path.display()
        );
        Ok(true)
    }

    fn try_start_replacement(&mut self, idx: usize) -> Result<(), String> {
        if idx >= self.loaded.len() {
            return Err("replacement index is out of bounds".to_owned());
        }
        let id = self.loaded[idx].info.id.to_string();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_current_plugin_id(&id, || {
                rresult_unit_to_string(self.loaded[idx].module.start())
            })
        }));
        match result {
            Ok(Ok(())) => {
                self.loaded[idx].state = PluginState::Running;
                Ok(())
            }
            Ok(Err(error)) => Err(error),
            Err(_) => Err("replacement start panicked".to_owned()),
        }
    }

    pub fn start_by_id(&mut self, id: &str) -> bool {
        crate::host_context::activate_host_context(&self.host);
        let Some(idx) = self.find_index(id) else {
            return false;
        };

        match self.loaded[idx].state {
            PluginState::Registered | PluginState::Stopped => {
                self.start_plugin_inline(idx);
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
        let publication = snapshot_provider_publication(&id);
        let root = crate::root_observers::loaded_plugin_root_snapshot(&id);
        let raw_editor_exports = root_exports_plugin_code(root.as_ref());

        // Stop admitting service/event callbacks before module shutdown or DLL reclamation.
        quiesce_provider_publication(&publication);
        crate::root_observers::forget_loaded_plugin_root(&id);
        let drained =
            wait_for_provider_publication_quiescence(&publication, Duration::from_secs(2));

        if drained && !raw_editor_exports {
            shutdown_provider_publication_services(&id, &publication, "plugin-manager.unload_at");
            self.safe_shutdown_one_reason(idx, "plugin-manager.unload_at");
            unregister_by_owner(&id);
            drop(publication);
            self.loaded_ids.remove(&id);
            let plugin = self.loaded.remove(idx);
            plugin.drop_with_library_policy(false);
        } else {
            // Raw exported editor callbacks may have escaped the root registry, and a hung
            // callback cannot be force-killed safely. Logical unload is still immediate;
            // physical reclamation is deferred by retaining the ABI state mapped.
            unregister_by_owner(&id);
            self.loaded_ids.remove(&id);
            let plugin = self.loaded.remove(idx);
            newengine_ulog_api::ulog::warn!(
                "plugins: unload retired conservatively id='{}' drained={} raw_editor_exports={} policy='retain-mapped-state'",
                id,
                drained,
                raw_editor_exports
            );
            std::mem::forget(plugin);
            std::mem::forget(publication);
        }
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
        // engine.threading/task events, not serde_json on every plugin callback.
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

        let publication = snapshot_provider_publication(id);
        quiesce_provider_publication(&publication);
        let drained =
            wait_for_provider_publication_quiescence(&publication, Duration::from_secs(2));
        if drained {
            shutdown_provider_publication_services(
                id,
                &publication,
                "plugin-manager.disable_plugin",
            );
            self.safe_shutdown_one_reason(idx, "plugin-manager.disable_plugin");
        } else {
            newengine_ulog_api::ulog::warn!(
                "plugins: disable skipped provider shutdown id='{}' reason='callback quiescence timeout'",
                id
            );
        }
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
