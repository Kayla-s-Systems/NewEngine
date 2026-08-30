#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;
use std::time::Instant;

use libloading::Library;

use newengine_plugin_api::{
    HostApiV1, PluginDescriptor, PluginDescriptorV2, PluginInfo, PluginRootV1Ref,
    LEGACY_PLUGIN_ROOT_SYMBOL_BYTES_NUL, LEGACY_PLUGIN_ROOT_SYMBOL_NAME,
    PLUGIN_DESCRIPTOR_V2_SYMBOL_BYTES_NUL, PLUGIN_ROOT_SYMBOL_BYTES_NUL, PLUGIN_ROOT_SYMBOL_NAME,
};

use crate::host_context::{
    begin_provider_transaction, commit_provider_transaction, register_plugin_descriptor,
    rollback_provider_transaction, validate_provider_transaction,
};
use crate::plugin_config_service::get_plugin_overrides_with_env;
use crate::root_observers::{record_loaded_plugin_root, LoadedPluginRootSnapshot};
use newengine_ulog_api::path_format::{canonicalize_if_exists, display_clean};

use super::adapter::ModuleAdapterAny;
use super::load_profile::{LoadProfilerJob, LoadTimings};
use super::plugin_init::{
    init_with_overrides, require_non_empty, shutdown_adapter_any, shutdown_after_failed_init,
};
use super::types::{LoadedPlugin, PluginLoadError, PluginLoadOrigin, PluginState};
use super::ui_assets::{extract_plugin_icon, PluginIconData};
use super::PluginManager;

fn pretty_abs_path(path: &Path) -> String {
    let p = canonicalize_if_exists(path);
    display_clean(&p)
}

impl PluginManager {
    pub(crate) fn load_one(&mut self, path: &Path, host: HostApiV1) -> Result<(), PluginLoadError> {
        self.load_one_with_origin(path, host, PluginLoadOrigin::Auto)
    }

    pub(crate) fn load_one_with_origin(
        &mut self,
        path: &Path,
        host: HostApiV1,
        load_origin: PluginLoadOrigin,
    ) -> Result<(), PluginLoadError> {
        crate::host_context::activate_host_context(&self.host);
        let frozen_plan_present = self.frozen_composition_plan.is_some();
        let frozen_manifest = self
            .frozen_composition_plan
            .as_ref()
            .and_then(|plan| plan.artifact_manifest(path))
            .cloned();
        let pretty_path = pretty_abs_path(path);
        newengine_ulog_api::ulog::info!("plugins: loading '{}'", pretty_path.as_str());

        let mut load_job = LoadProfilerJob::begin(pretty_path.as_str());
        let t_total = Instant::now();
        let mut tm = LoadTimings::default();

        // Freeze-time discovery owns the expensive artifact fingerprint. Before mapping
        // executable code, validate that the observed file identity is still unchanged;
        // only metadata drift falls back to SHA-256. Manual loads capture their verified
        // snapshot here, so they still hash exactly once before `Library::new`.
        let t_discovery = Instant::now();
        let verification_snapshot = match frozen_manifest {
            Some(snapshot) => snapshot,
            None if frozen_plan_present => {
                return Err(PluginLoadError {
                    path: path.to_path_buf(),
                    message: format!(
                        "artifact '{}' is absent from the frozen discovery manifest inventory",
                        path.display()
                    ),
                });
            }
            None => {
                super::discovery::read_verified_manifest(path).map_err(|error| PluginLoadError {
                    path: path.to_path_buf(),
                    message: format!("discovery metadata verification failed: {error}"),
                })?
            }
        };
        super::discovery::verify_artifact_against_manifest(path, &verification_snapshot).map_err(
            |error| PluginLoadError {
                path: path.to_path_buf(),
                message: format!("frozen discovery artifact verification failed: {error}"),
            },
        )?;
        tm.discovery_verify_ms = t_discovery.elapsed().as_millis();

        let t = Instant::now();
        newengine_ulog_api::ulog::info!(
            "plugins: load stage='dlopen-begin' path='{}'",
            pretty_path.as_str()
        );
        let lib = unsafe { Library::new(path) }.map_err(|e| PluginLoadError {
            path: path.to_path_buf(),
            message: format!("Library::new failed: {e}"),
        })?;
        tm.dlopen_ms = t.elapsed().as_millis();
        newengine_ulog_api::ulog::info!(
            "plugins: load stage='dlopen-done' path='{}'",
            pretty_path.as_str()
        );

        let t = Instant::now();
        newengine_ulog_api::ulog::info!(
            "plugins: load stage='root-symbol-begin' path='{}'",
            pretty_path.as_str()
        );
        let sym: libloading::Symbol<unsafe extern "C" fn() -> PluginRootV1Ref> =
            unsafe { lib.get(PLUGIN_ROOT_SYMBOL_BYTES_NUL) }.map_err(|e| {
                let has_legacy_root = unsafe {
                    lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(LEGACY_PLUGIN_ROOT_SYMBOL_BYTES_NUL)
                }
                .is_ok();
                let detail = if has_legacy_root {
                    format!(
                        "stale plugin ABI: found legacy root symbol '{}' but missing canonical root symbol '{}'; rebuild this plugin after API cleanup ({e})",
                        LEGACY_PLUGIN_ROOT_SYMBOL_NAME,
                        PLUGIN_ROOT_SYMBOL_NAME,
                    )
                } else {
                    format!("symbol {} not found: {e}", PLUGIN_ROOT_SYMBOL_NAME)
                };
                PluginLoadError {
                    path: path.to_path_buf(),
                    message: detail,
                }
            })?;
        tm.sym_ms = t.elapsed().as_millis();
        newengine_ulog_api::ulog::info!(
            "plugins: load stage='root-symbol-done' path='{}'",
            pretty_path.as_str()
        );
        let t = Instant::now();
        let descriptor_v2 = unsafe {
            lib.get::<unsafe extern "C" fn() -> PluginDescriptorV2>(
                PLUGIN_DESCRIPTOR_V2_SYMBOL_BYTES_NUL,
            )
        }
        .ok()
        .map(|symbol| unsafe { symbol() });
        tm.descriptor_v2_ms = t.elapsed().as_millis();

        let t = Instant::now();
        newengine_ulog_api::ulog::info!(
            "plugins: load stage='root-call-begin' path='{}'",
            pretty_path.as_str()
        );
        let root = unsafe { sym() };
        newengine_ulog_api::ulog::info!(
            "plugins: load stage='root-call-done' path='{}'",
            pretty_path.as_str()
        );
        let editor_extensions_v1 = root.editor_extensions_v1();
        newengine_ulog_api::ulog::info!(
            "plugins: load stage='ui-assets-done' path='{}'",
            pretty_path.as_str()
        );
        tm.root_ms = t.elapsed().as_millis();

        let t = Instant::now();
        newengine_ulog_api::ulog::info!(
            "plugins: load stage='module-create-begin' path='{}'",
            pretty_path.as_str()
        );
        let (mut module_any, info, descriptor, icon_small) = select_module(root);
        newengine_ulog_api::ulog::info!(
            "plugins: load stage='module-create-done' id='{}' path='{}'",
            info.id,
            pretty_path.as_str()
        );
        tm.module_create_ms = t.elapsed().as_millis();

        let t_identity = Instant::now();
        let id_str = info.id.to_string();
        if let Err(e) = require_non_empty("plugin id", &id_str, path) {
            shutdown_adapter_any(module_any);
            return Err(e);
        }
        let name_str = info.name.to_string();
        if let Err(e) = require_non_empty("plugin name", &name_str, path) {
            shutdown_adapter_any(module_any);
            return Err(e);
        }
        let ver_str = info.version.to_string();
        if let Err(e) = require_non_empty("plugin version", &ver_str, path) {
            shutdown_adapter_any(module_any);
            return Err(e);
        }
        if let Some(typed) = descriptor_v2.as_ref() {
            let typed_id = typed.id.as_str();
            let typed_version = typed.version.as_str();
            if typed_id != id_str || typed_version != ver_str || typed.kind != descriptor.kind {
                shutdown_adapter_any(module_any);
                return Err(PluginLoadError {
                    path: path.to_path_buf(),
                    message: format!(
                        "typed descriptor V2 identity mismatch v1={{id:'{}', version:'{}', kind:{:?}}} v2={{id:'{}', version:'{}', kind:{:?}}}",
                        id_str, ver_str, descriptor.kind, typed_id, typed_version, typed.kind
                    ),
                });
            }
        }

        let normalized_descriptor_v2 = descriptor_v2
            .clone()
            .unwrap_or_else(|| PluginDescriptorV2::from_legacy(&descriptor));
        tm.identity_validation_ms = t_identity.elapsed().as_millis();
        let t_discovery = Instant::now();
        let discovery_verification = super::discovery::verify_live_descriptor_against_manifest(
            path,
            &normalized_descriptor_v2,
            &verification_snapshot,
        );
        tm.discovery_verify_ms = tm
            .discovery_verify_ms
            .saturating_add(t_discovery.elapsed().as_millis());
        if let Err(error) = discovery_verification {
            shutdown_adapter_any(module_any);
            return Err(PluginLoadError {
                path: path.to_path_buf(),
                message: format!("frozen discovery metadata verification failed: {error}"),
            });
        }

        if self.loaded_ids.contains(&id_str) {
            newengine_ulog_api::ulog::warn!(
                "plugins: duplicate id='{}' from '{}' ignored (already loaded)",
                id_str,
                pretty_path.as_str()
            );
            shutdown_adapter_any(module_any);
            return Ok(());
        }

        let t = Instant::now();
        let overrides = get_plugin_overrides_with_env(&id_str);
        tm.override_lookup_ms = t.elapsed().as_millis();
        let overrides_non_empty =
            !matches!(overrides, serde_json::Value::Object(ref mm) if mm.is_empty());

        let t_provider_prepare = Instant::now();
        let mut provider_origin = load_origin.gateway_origin(path);
        begin_provider_transaction(&id_str).map_err(|e| PluginLoadError {
            path: path.to_path_buf(),
            message: format!("provider transaction begin failed: {e}"),
        })?;
        provider_origin = register_plugin_descriptor(
            &id_str,
            descriptor.clone(),
            descriptor_v2.clone(),
            provider_origin,
        );
        tm.provider_prepare_ms = t_provider_prepare.elapsed().as_millis();

        let t = Instant::now();
        let init_breakdown = match init_with_overrides(
            &mut module_any,
            &id_str,
            host.clone(),
            overrides_non_empty,
            &overrides,
        ) {
            Ok(value) => value,
            Err(error) => {
                rollback_provider_transaction(&id_str);
                return Err(PluginLoadError {
                    path: path.to_path_buf(),
                    message: format!("init failed: {error}"),
                });
            }
        };
        if let Err(error) = validate_provider_transaction(&id_str) {
            rollback_provider_transaction(&id_str);
            shutdown_after_failed_init(&id_str, &mut module_any);
            return Err(PluginLoadError {
                path: path.to_path_buf(),
                message: format!("provider transaction validation failed: {error}"),
            });
        }
        let committed_services =
            commit_provider_transaction(&id_str).map_err(|error| PluginLoadError {
                path: path.to_path_buf(),
                message: format!("provider transaction commit failed: {error}"),
            })?;
        newengine_ulog_api::ulog::debug!(
            "plugins: provider transaction committed id='{}' services={}",
            id_str,
            committed_services
        );
        tm.init_total_ms = t.elapsed().as_millis();
        tm.init_breakdown = Some(init_breakdown);
        tm.total_ms = t_total.elapsed().as_millis();

        newengine_ulog_api::ulog::info!(
            "plugins: loaded id='{}' ver='{}' origin='{}' from '{}'",
            info.id,
            info.version,
            provider_origin.as_str(),
            pretty_path.as_str()
        );

        if newengine_ulog_api::ulog::debug_enabled() {
            if let Some(ref bd) = tm.init_breakdown {
                newengine_ulog_api::ulog::debug!(
                    "plugins: load timing id='{}' total_ms={} dlopen_ms={} sym_ms={} root_ms={} module_create_ms={} override_ms={} init_ms={} (cfg_defaults_ms={} cfg_apply_ms={} init_call_ms={})",
                    info.id,
                    tm.total_ms,
                    tm.dlopen_ms,
                    tm.sym_ms,
                    tm.root_ms,
                    tm.module_create_ms,
                    tm.override_lookup_ms,
                    tm.init_total_ms,
                    bd.config_defaults_ms,
                    bd.config_apply_ms,
                    bd.init_ms,
                );
            }
        }

        self.loaded_ids.insert(id_str.clone());
        let descriptor_v2 = normalized_descriptor_v2;
        self.loaded.push(LoadedPlugin {
            path: path.to_path_buf(),
            loaded_binary_path: path.to_path_buf(),
            module: module_any,
            info,
            descriptor: Some(descriptor),
            descriptor_v2: Some(descriptor_v2),
            state: PluginState::Registered,
            disabled_reason: None,
            icon_small,
            _lib: lib,
        });

        crate::ulog_event::emit_ulog_event(
            &host,
            "engine.provider.registered",
            "INFO",
            "Provider registered",
            serde_json::json!({
                "provider_id": id_str,
                "version": ver_str,
                "origin": provider_origin.as_str(),
                "path": pretty_path,
                "total_ms": tm.total_ms,
                "dlopen_ms": tm.dlopen_ms,
                "sym_ms": tm.sym_ms,
                "root_ms": tm.root_ms,
                "module_create_ms": tm.module_create_ms,
                "init_total_ms": tm.init_total_ms
            }),
        );

        record_loaded_plugin_root(LoadedPluginRootSnapshot {
            plugin_id: id_str.clone(),
            editor_extensions_v1,
        });
        load_job.complete_ok(&id_str, &tm);
        Ok(())
    }
}

fn select_module(
    root: PluginRootV1Ref,
) -> (
    ModuleAdapterAny,
    PluginInfo,
    PluginDescriptor,
    Option<PluginIconData>,
) {
    let module = root.create()();
    let descriptor = module.descriptor();
    let info = PluginInfo {
        id: descriptor.id.clone(),
        name: descriptor.name.clone(),
        version: descriptor.version.clone(),
    };
    let icon_small = extract_plugin_icon(root);
    newengine_ulog_api::ulog::debug!("plugins: canonical ABI selected id='{}'", info.id);
    (ModuleAdapterAny::new(module), info, descriptor, icon_small)
}
