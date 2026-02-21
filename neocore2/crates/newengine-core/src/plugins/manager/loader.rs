#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;
use std::time::Instant;

use abi_stable::std_types::RVec;
use libloading::Library;

use newengine_plugin_api::{
    ConfigDiagLevelV1, ConfigPatchV1, HostApiV1, PluginDescriptor, PluginInfo, PluginModuleDyn,
    PluginRootV1Ref,
};

use crate::plugins::host_context::{unregister_by_owner, with_current_plugin_id};
use crate::plugins::plugin_config_service::get_plugin_overrides_with_env;

use super::adapter::{ModuleAdapterAny, V1Adapter, V2Adapter, V3Adapter};
use super::config_patch::config_patch_from_json_merge_patch;
use super::types::{LoadedPlugin, PluginLoadError, PluginState};
use super::PluginManager;

impl PluginManager {
    pub(crate) fn load_one(&mut self, path: &Path, host: HostApiV1) -> Result<(), PluginLoadError> {
        log::info!("plugins: loading '{}'", path.display());
        let t0 = Instant::now();

        let lib = unsafe { Library::new(path) }.map_err(|e| PluginLoadError {
            path: path.to_path_buf(),
            message: format!("Library::new failed: {e}"),
        })?;

        let sym: libloading::Symbol<unsafe extern "C" fn() -> PluginRootV1Ref> =
            unsafe { lib.get(b"export_plugin_root\0") }.map_err(|e| PluginLoadError {
                path: path.to_path_buf(),
                message: format!("symbol export_plugin_root not found: {e}"),
            })?;

        let root = unsafe { sym() };

        let has_v3 = root.create_v3().is_some();
        let has_v2 = root.create_v2().is_some();
        log::debug!(
            "plugins: abi probe path='{}' v3={} v2={} ",
            path.display(),
            has_v3,
            has_v2
        );

        let (mut module_any, info, descriptor) = select_abi(root);

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

        if self.loaded_ids.contains(&id_str) {
            log::warn!(
                "plugins: duplicate id='{}' from '{}' ignored (already loaded)",
                id_str,
                path.display()
            );
            shutdown_adapter_any(module_any);
            return Ok(());
        }

        let overrides = get_plugin_overrides_with_env(&id_str);
        let overrides_non_empty =
            !matches!(overrides, serde_json::Value::Object(ref mm) if mm.is_empty());

        if overrides_non_empty && !matches!(module_any, ModuleAdapterAny::V3(_)) {
            log::error!(
                "plugins: override present for id='{}' but plugin ABI is not V3; overrides will be ignored. path='{}'",
                id_str,
                path.display()
            );
        }

        init_with_overrides(&mut module_any, &id_str, host, overrides_non_empty, &overrides)
            .map_err(|e| PluginLoadError {
                path: path.to_path_buf(),
                message: format!("init failed: {e}"),
            })?;

        log::info!(
            "plugins: loaded id='{}' ver='{}' from '{}'",
            info.id,
            info.version,
            path.display()
        );
        log::debug!(
            "plugins: load timing id='{}' elapsed_ms={}",
            info.id,
            t0.elapsed().as_millis()
        );

        self.loaded_ids.insert(id_str);
        self.loaded.push(LoadedPlugin {
            path: path.to_path_buf(),
            _lib: lib,
            module: module_any,
            info,
            descriptor,
            state: PluginState::Registered,
            disabled_reason: None,
        });

        Ok(())
    }
}

fn select_abi(root: PluginRootV1Ref) -> (ModuleAdapterAny, PluginInfo, Option<PluginDescriptor>) {
    if let Some(create_v3) = root.create_v3() {
        let m3 = create_v3();
        let d = m3.descriptor_v3();
        let info = PluginInfo {
            id: d.id.clone(),
            name: d.name.clone(),
            version: d.version.clone(),
        };
        log::debug!("plugins: abi selected v3 id='{}'", info.id);
        (ModuleAdapterAny::V3(V3Adapter { module: m3 }), info, Some(d))
    } else if let Some(create_v2) = root.create_v2() {
        let m2 = create_v2();
        let d = m2.descriptor();
        let info = PluginInfo {
            id: d.id.clone(),
            name: d.name.clone(),
            version: d.version.clone(),
        };
        log::debug!("plugins: abi selected v2 id='{}'", info.id);
        (ModuleAdapterAny::V2(V2Adapter { module: m2 }), info, Some(d))
    } else {
        let m1: PluginModuleDyn<'static> = root.create()();
        let info = m1.info();
        log::debug!("plugins: abi selected v1 id='{}'", info.id);
        (ModuleAdapterAny::V1(V1Adapter { module: m1 }), info, None)
    }
}

fn init_with_overrides(
    module_any: &mut ModuleAdapterAny,
    id_str: &str,
    host: HostApiV1,
    overrides_non_empty: bool,
    overrides: &serde_json::Value,
) -> Result<(), String> {
    let init_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_current_plugin_id(id_str, || match module_any {
            ModuleAdapterAny::V1(a) => a
                .module
                .init(host.clone())
                .into_result()
                .map_err(|e| e.to_string()),
            ModuleAdapterAny::V2(a) => a
                .module
                .init(host.clone())
                .into_result()
                .map_err(|e| e.to_string()),
            ModuleAdapterAny::V3(a) => {
                let defaults = a
                    .module
                    .config_defaults_v1()
                    .into_result()
                    .map_err(|e| e.to_string())?;

                log::debug!(
                    "plugins: config defaults id='{}' content_type='{}' len={} fmt_v={} ",
                    id_str,
                    defaults.content_type,
                    defaults.bytes.len(),
                    defaults.format_version
                );

                let mut patches = RVec::<ConfigPatchV1>::new();
                if overrides_non_empty {
                    patches.push(config_patch_from_json_merge_patch(
                        id_str,
                        "config+env",
                        0,
                        overrides,
                    ));
                }

                let applied = a
                    .module
                    .config_apply_patches_v1(&defaults, patches)
                    .into_result()
                    .map_err(|e| e.to_string())?;

                // Cheap delivery sanity: content type + length + short preview.
                // This is intentionally `debug!` so production logs stay clean.
                {
                    const PREVIEW_MAX: usize = 200;
                    let s = String::from_utf8_lossy(applied.effective.bytes.as_slice());
                    let mut preview = s.to_string();
                    if preview.len() > PREVIEW_MAX {
                        preview.truncate(PREVIEW_MAX);
                        preview.push_str("…");
                    }
                    log::debug!(
                        "plugins: config effective id='{}' content_type='{}' len={} changed={} preview='{}'",
                        id_str,
                        applied.effective.content_type,
                        applied.effective.bytes.len(),
                        applied.changed,
                        preview
                    );
                }

                for d in applied.diags.iter() {
                    match d.level {
                        ConfigDiagLevelV1::Info => {
                            log::info!(
                                "plugins: config info id='{}' {} {}",
                                id_str,
                                d.code,
                                d.message
                            );
                        }
                        ConfigDiagLevelV1::Warn => {
                            log::warn!(
                                "plugins: config warn id='{}' {} {}",
                                id_str,
                                d.code,
                                d.message
                            );
                        }
                        ConfigDiagLevelV1::Error => {
                            log::error!(
                                "plugins: config error id='{}' {} {}",
                                id_str,
                                d.code,
                                d.message
                            );
                        }
                    }
                }

                a.module
                    .init_v3(host.clone(), applied.effective)
                    .into_result()
                    .map_err(|e| e.to_string())
            }
        })
    }));

    match init_res {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => {
            unregister_by_owner(id_str);
            shutdown_after_failed_init(id_str, module_any);
            Err(e)
        }
        Err(_) => {
            unregister_by_owner(id_str);
            shutdown_after_failed_init(id_str, module_any);
            Err("init panicked".to_string())
        }
    }
}

fn shutdown_after_failed_init(id_str: &str, module_any: &mut ModuleAdapterAny) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_current_plugin_id(id_str, || match module_any {
            ModuleAdapterAny::V1(a) => a.module.shutdown(),
            ModuleAdapterAny::V2(a) => a.module.shutdown(),
            ModuleAdapterAny::V3(a) => a.module.shutdown(),
        })
    }));
}

fn shutdown_adapter_any(mut module_any: ModuleAdapterAny) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        module_any.shutdown();
    }));
}

fn require_non_empty(field: &str, value: &str, path: &Path) -> Result<(), PluginLoadError> {
    if value.trim().is_empty() {
        return Err(PluginLoadError {
            path: path.to_path_buf(),
            message: format!("{field} is empty"),
        });
    }
    Ok(())
}
