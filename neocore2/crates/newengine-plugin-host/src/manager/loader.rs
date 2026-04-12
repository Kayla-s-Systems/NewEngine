#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;
use std::time::Instant;

use abi_stable::std_types::RVec;
use libloading::Library;

use newengine_plugin_api::{
    ConfigDiagLevelV1, ConfigPatchV1, HostApiV1, PluginDescriptor, PluginInfo, PluginModuleDyn,
    PluginRootV1Ref,
};

use crate::host_context::{
    register_plugin_descriptor, unregister_by_owner, with_current_plugin_id,
};
use crate::path_fmt::{canonicalize_if_exists, display_clean};
use crate::plugin_config_service::get_plugin_overrides_with_env;

use super::adapter::{ModuleAdapterAny, V1Adapter, V2Adapter, V3Adapter};
use super::config_patch::config_patch_from_json_merge_patch;
use super::types::{LoadedPlugin, PluginLoadError, PluginState};
use super::ui_assets::{extract_plugin_icon, PluginIconData};
use super::PluginManager;
use crate::root_observers::{record_loaded_plugin_root, LoadedPluginRootSnapshot};

fn pretty_abs_path(path: &Path) -> String {
    // Best-effort canonicalization for log output.
    // If the path does not exist (or cannot be canonicalized), keep the original.
    let p = canonicalize_if_exists(path);
    display_clean(&p)
}

#[derive(Debug, Clone, Default)]
struct InitTimings {
    config_defaults_ms: u128,
    config_apply_ms: u128,
    init_ms: u128,
}

#[derive(Debug, Clone, Default)]
struct LoadTimings {
    dlopen_ms: u128,
    sym_ms: u128,
    root_ms: u128,
    abi_probe_ms: u128,
    select_abi_ms: u128,
    override_lookup_ms: u128,
    init_total_ms: u128,
    init_breakdown: Option<InitTimings>,
    total_ms: u128,
}

impl PluginManager {
    pub(crate) fn load_one(&mut self, path: &Path, host: HostApiV1) -> Result<(), PluginLoadError> {
        let pretty_path = pretty_abs_path(path);
        log::info!("plugins: loading '{}'", pretty_path.as_str());

        let t_total = Instant::now();
        let mut tm = LoadTimings::default();

        let t = Instant::now();
        let lib = unsafe { Library::new(path) }.map_err(|e| PluginLoadError {
            path: path.to_path_buf(),
            message: format!("Library::new failed: {e}"),
        })?;
        tm.dlopen_ms = t.elapsed().as_millis();

        let t = Instant::now();
        let sym: libloading::Symbol<unsafe extern "C" fn() -> PluginRootV1Ref> =
            unsafe { lib.get(b"export_plugin_root\0") }.map_err(|e| PluginLoadError {
                path: path.to_path_buf(),
                message: format!("symbol export_plugin_root not found: {e}"),
            })?;
        tm.sym_ms = t.elapsed().as_millis();

        let t = Instant::now();
        let root = unsafe { sym() };
        let editor_extensions_v1 = root.editor_extensions_v1();
        tm.root_ms = t.elapsed().as_millis();

        let t = Instant::now();
        let has_v3 = root.create_v3().is_some();
        let has_v2 = root.create_v2().is_some();
        tm.abi_probe_ms = t.elapsed().as_millis();
        log::debug!(
            "plugins: abi probe path='{}' v3={} v2={} ",
            pretty_path.as_str(),
            has_v3,
            has_v2
        );

        let t = Instant::now();
        let (mut module_any, info, descriptor, icon_small) = select_abi(root);
        tm.select_abi_ms = t.elapsed().as_millis();

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

        if overrides_non_empty && !matches!(module_any, ModuleAdapterAny::V3(_)) {
            log::error!(
                "plugins: override present for id='{}' but plugin ABI is not V3; overrides will be ignored. path='{}'",
                id_str,
                pretty_path.as_str()
            );
        }

        // Register descriptor metadata for runtime validation (services/sinks).
        if let Some(d) = descriptor.clone() {
            register_plugin_descriptor(&id_str, d);
        }

        let t = Instant::now();
        let init_breakdown = init_with_overrides(
            &mut module_any,
            &id_str,
            host,
            overrides_non_empty,
            &overrides,
        )
            .map_err(|e| PluginLoadError {
                path: path.to_path_buf(),
                message: format!("init failed: {e}"),
            })?;
        tm.init_total_ms = t.elapsed().as_millis();
        tm.init_breakdown = init_breakdown;

        tm.total_ms = t_total.elapsed().as_millis();

        log::info!(
            "plugins: loaded id='{}' ver='{}' from '{}'",
            info.id,
            info.version,
            pretty_path.as_str()
        );

        if log::log_enabled!(log::Level::Debug) {
            if let Some(ref bd) = tm.init_breakdown {
                log::debug!(
                    "plugins: load timing id='{}' total_ms={} dlopen_ms={} sym_ms={} root_ms={} abi_probe_ms={} select_abi_ms={} override_ms={} init_ms={} (cfg_defaults_ms={} cfg_apply_ms={} init_call_ms={})",
                    info.id,
                    tm.total_ms,
                    tm.dlopen_ms,
                    tm.sym_ms,
                    tm.root_ms,
                    tm.abi_probe_ms,
                    tm.select_abi_ms,
                    tm.override_lookup_ms,
                    tm.init_total_ms,
                    bd.config_defaults_ms,
                    bd.config_apply_ms,
                    bd.init_ms,
                );
            } else {
                log::debug!(
                    "plugins: load timing id='{}' total_ms={} dlopen_ms={} sym_ms={} root_ms={} abi_probe_ms={} select_abi_ms={} override_ms={} init_ms={} ",
                    info.id,
                    tm.total_ms,
                    tm.dlopen_ms,
                    tm.sym_ms,
                    tm.root_ms,
                    tm.abi_probe_ms,
                    tm.select_abi_ms,
                    tm.override_lookup_ms,
                    tm.init_total_ms,
                );
            }
        }

        self.loaded_ids.insert(id_str.clone());
        self.loaded.push(LoadedPlugin {
            path: path.to_path_buf(),
            _lib: lib,
            module: module_any,
            info,
            descriptor,
            state: PluginState::Registered,
            disabled_reason: None,
            icon_small,
        });

        record_loaded_plugin_root(LoadedPluginRootSnapshot {
            plugin_id: id_str,
            editor_extensions_v1,
        });

        Ok(())
    }
}

fn select_abi(
    root: PluginRootV1Ref,
) -> (
    ModuleAdapterAny,
    PluginInfo,
    Option<PluginDescriptor>,
    Option<PluginIconData>,
) {
    let icon_small = extract_plugin_icon(root.clone());
    if let Some(create_v3) = root.create_v3() {
        let m3 = create_v3();
        let d = m3.descriptor_v3();
        let info = PluginInfo {
            id: d.id.clone(),
            name: d.name.clone(),
            version: d.version.clone(),
        };
        log::debug!("plugins: abi selected v3 id='{}'", info.id);
        (
            ModuleAdapterAny::V3(V3Adapter { module: m3 }),
            info,
            Some(d),
            icon_small,
        )
    } else if let Some(create_v2) = root.create_v2() {
        let m2 = create_v2();
        let d = m2.descriptor();
        let info = PluginInfo {
            id: d.id.clone(),
            name: d.name.clone(),
            version: d.version.clone(),
        };
        log::debug!("plugins: abi selected v2 id='{}'", info.id);
        (
            ModuleAdapterAny::V2(V2Adapter { module: m2 }),
            info,
            Some(d),
            icon_small,
        )
    } else {
        let m1: PluginModuleDyn<'static> = root.create()();
        let info = m1.info();
        log::debug!("plugins: abi selected v1 id='{}'", info.id);
        (ModuleAdapterAny::V1(V1Adapter { module: m1 }), info, None, icon_small)
    }
}

fn init_with_overrides(
    module_any: &mut ModuleAdapterAny,
    id_str: &str,
    host: HostApiV1,
    overrides_non_empty: bool,
    overrides: &serde_json::Value,
) -> Result<Option<InitTimings>, String> {
    let mut timings: Option<InitTimings> = None;

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
                let mut t = InitTimings::default();

                let t0 = Instant::now();
                let defaults = a
                    .module
                    .config_defaults_v1()
                    .into_result()
                    .map_err(|e| e.to_string())?;
                t.config_defaults_ms = t0.elapsed().as_millis();

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

                let t0 = Instant::now();
                let applied = a
                    .module
                    .config_apply_patches_v1(&defaults, patches)
                    .into_result()
                    .map_err(|e| e.to_string())?;
                t.config_apply_ms = t0.elapsed().as_millis();

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

                    let changed_keys = json_diff_keys_shallow_or_paths(
                        defaults.content_type.as_str(),
                        defaults.bytes.as_slice(),
                        applied.effective.bytes.as_slice(),
                    );

                    if changed_keys.is_empty() {
                        log::debug!(
                            "plugins: config effective id='{}' content_type='{}' len={} changed={} preview='{}'",
                            id_str,
                            applied.effective.content_type,
                            applied.effective.bytes.len(),
                            applied.changed,
                            preview
                        );
                    } else {
                        log::debug!(
                            "plugins: config effective id='{}' content_type='{}' len={} changed={} changed_keys=[{}] preview='{}'",
                            id_str,
                            applied.effective.content_type,
                            applied.effective.bytes.len(),
                            applied.changed,
                            changed_keys.join(", "),
                            preview
                        );
                    }
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

                let t0 = Instant::now();
                let res = a
                    .module
                    .init_v3(host.clone(), applied.effective)
                    .into_result()
                    .map_err(|e| e.to_string());
                t.init_ms = t0.elapsed().as_millis();

                timings = Some(t);
                res
            }
        })
    }));

    match init_res {
        Ok(Ok(())) => Ok(timings),
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

fn json_diff_keys_shallow_or_paths(
    content_type: &str,
    defaults_bytes: &[u8],
    effective_bytes: &[u8],
) -> Vec<String> {
    if content_type != "application/json" {
        return Vec::new();
    }

    let defaults: serde_json::Value = match serde_json::from_slice(defaults_bytes) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let effective: serde_json::Value = match serde_json::from_slice(effective_bytes) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut out: Vec<String> = Vec::new();
    diff_json_paths(&defaults, &effective, "", 0, 4, 64, &mut out);
    out.sort();
    out.dedup();
    out
}

fn diff_json_paths(
    a: &serde_json::Value,
    b: &serde_json::Value,
    prefix: &str,
    depth: usize,
    max_depth: usize,
    max_items: usize,
    out: &mut Vec<String>,
) {
    if out.len() >= max_items {
        return;
    }

    if depth >= max_depth {
        if a != b {
            out.push(prefix.to_owned());
        }
        return;
    }

    match (a, b) {
        (serde_json::Value::Object(ao), serde_json::Value::Object(bo)) => {
            for k in ao.keys() {
                if out.len() >= max_items {
                    return;
                }
                if !bo.contains_key(k) {
                    out.push(join_path(prefix, k));
                }
            }
            for (k, bv) in bo.iter() {
                if out.len() >= max_items {
                    return;
                }
                match ao.get(k) {
                    None => out.push(join_path(prefix, k)),
                    Some(av) => {
                        let p = join_path(prefix, k);
                        diff_json_paths(av, bv, &p, depth + 1, max_depth, max_items, out);
                    }
                }
            }
        }
        (serde_json::Value::Array(aa), serde_json::Value::Array(ba)) => {
            if aa.len() != ba.len() {
                out.push(prefix.to_owned());
                return;
            }
            for (i, (av, bv)) in aa.iter().zip(ba.iter()).enumerate() {
                if out.len() >= max_items {
                    return;
                }
                let p = if prefix.is_empty() {
                    format!("[{}]", i)
                } else {
                    format!("{}[{}]", prefix, i)
                };
                diff_json_paths(av, bv, &p, depth + 1, max_depth, max_items, out);
            }
        }
        _ => {
            if a != b {
                out.push(prefix.to_owned());
            }
        }
    }
}

#[inline]
fn join_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_owned()
    } else {
        format!("{}.{}", prefix, key)
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
