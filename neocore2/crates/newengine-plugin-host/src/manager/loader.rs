#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;
use std::time::Instant;

use abi_stable::std_types::RVec;
use libloading::Library;

use newengine_plugin_api::{
    ConfigDiagLevelV1, ConfigPatchV1, HostApiV1, PluginDescriptor, PluginInfo, PluginRootV1Ref,
    LEGACY_PLUGIN_ROOT_SYMBOL_BYTES_NUL, LEGACY_PLUGIN_ROOT_SYMBOL_NAME,
    PLUGIN_ROOT_SYMBOL_BYTES_NUL, PLUGIN_ROOT_SYMBOL_NAME,
};

use crate::host_context::{
    register_plugin_descriptor, unregister_by_owner, with_current_plugin_id,
};
use crate::plugin_config_service::get_plugin_overrides_with_env;
use crate::root_observers::{record_loaded_plugin_root, LoadedPluginRootSnapshot};
use newengine_ulog_api::path_format::{canonicalize_if_exists, display_clean};

use super::adapter::ModuleAdapterAny;
use super::config_patch::config_patch_from_json_merge_patch;
use super::types::{LoadedPlugin, PluginLoadError, PluginState};
use super::ui_assets::{extract_plugin_icon, PluginIconData};
use super::PluginManager;

fn pretty_abs_path(path: &Path) -> String {
    let p = canonicalize_if_exists(path);
    display_clean(&p)
}

struct LoadProfilerJob {
    id: String,
    path: String,
    started: Instant,
    completed: bool,
}

impl LoadProfilerJob {
    fn begin(path: &str) -> Self {
        let id = crate::diagnostics::next_job_id("host.plugin_load");
        crate::diagnostics::begin(serde_json::json!({
            "id": id.clone(),
            "name": format!("plugin_load:{}", path),
            "category": "plugin_lifecycle",
            "source": "newengine-plugin-host",
            "detail": "dynamic library load + canonical ABI root + init",
            "metadata": { "path": path, "operation": "load_one" }
        }));
        Self {
            id,
            path: path.to_owned(),
            started: Instant::now(),
            completed: false,
        }
    }

    fn complete_ok(&mut self, plugin_id: &str, timings: &LoadTimings) {
        self.completed = true;
        crate::diagnostics::end(serde_json::json!({
            "id": self.id.clone(),
            "status": "completed",
            "detail": format!("plugin loaded in {} ms", timings.total_ms),
            "metadata": {
                "plugin_id": plugin_id,
                "path": self.path.clone(),
                "total_ms": timings.total_ms,
                "dlopen_ms": timings.dlopen_ms,
                "sym_ms": timings.sym_ms,
                "root_ms": timings.root_ms,
                "module_create_ms": timings.module_create_ms,
                "override_lookup_ms": timings.override_lookup_ms,
                "init_total_ms": timings.init_total_ms
            }
        }));
    }
}

impl Drop for LoadProfilerJob {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        crate::diagnostics::end(serde_json::json!({
            "id": self.id.clone(),
            "status": "failed",
            "error": "plugin load exited before completion",
            "detail": format!(
                "plugin load failed or was skipped after {:.3} ms",
                crate::diagnostics::elapsed_ms(self.started)
            ),
            "metadata": { "path": self.path.clone(), "operation": "load_one" }
        }));
    }
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
    module_create_ms: u128,
    override_lookup_ms: u128,
    init_total_ms: u128,
    init_breakdown: Option<InitTimings>,
    total_ms: u128,
}

impl PluginManager {
    pub(crate) fn load_one(&mut self, path: &Path, host: HostApiV1) -> Result<(), PluginLoadError> {
        let pretty_path = pretty_abs_path(path);
        newengine_ulog_api::ulog::info!("plugins: loading '{}'", pretty_path.as_str());

        let mut load_job = LoadProfilerJob::begin(pretty_path.as_str());
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

        let t = Instant::now();
        let root = unsafe { sym() };
        let editor_extensions_v1 = root.editor_extensions_v1();
        tm.root_ms = t.elapsed().as_millis();

        let t = Instant::now();
        let (mut module_any, info, descriptor, icon_small) = select_module(root);
        tm.module_create_ms = t.elapsed().as_millis();

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

        let mut provider_origin =
            crate::service_gateway::GatewayProviderOrigin::from_plugin_path(path);
        provider_origin = register_plugin_descriptor(&id_str, descriptor.clone(), provider_origin);

        let t = Instant::now();
        let init_breakdown = init_with_overrides(
            &mut module_any,
            &id_str,
            host.clone(),
            overrides_non_empty,
            &overrides,
        )
        .map_err(|e| PluginLoadError {
            path: path.to_path_buf(),
            message: format!("init failed: {e}"),
        })?;
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
        self.loaded.push(LoadedPlugin {
            path: path.to_path_buf(),
            module: module_any,
            info,
            descriptor: Some(descriptor),
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

fn init_with_overrides(
    module_any: &mut ModuleAdapterAny,
    id_str: &str,
    host: HostApiV1,
    overrides_non_empty: bool,
    overrides: &serde_json::Value,
) -> Result<InitTimings, String> {
    let init_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_current_plugin_id(id_str, || {
            let mut t = InitTimings::default();

            let t0 = Instant::now();
            let defaults = module_any
                .module
                .config_defaults()
                .into_result()
                .map_err(|e| e.to_string())?;
            t.config_defaults_ms = t0.elapsed().as_millis();

            newengine_ulog_api::ulog::debug!(
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
            let applied = module_any
                .module
                .config_apply_patches(&defaults, patches)
                .into_result()
                .map_err(|e| e.to_string())?;
            t.config_apply_ms = t0.elapsed().as_millis();

            const PREVIEW_MAX: usize = 200;
            let s = String::from_utf8_lossy(applied.effective.bytes.as_slice());
            let mut preview = s.to_string();
            if preview.len() > PREVIEW_MAX {
                preview.truncate(PREVIEW_MAX);
                preview.push_str("...");
            }

            let changed_keys = json_diff_keys_shallow_or_paths(
                defaults.content_type.as_str(),
                defaults.bytes.as_slice(),
                applied.effective.bytes.as_slice(),
            );

            if changed_keys.is_empty() {
                newengine_ulog_api::ulog::debug!(
                    "plugins: config effective id='{}' content_type='{}' len={} changed={} preview='{}'",
                    id_str,
                    applied.effective.content_type,
                    applied.effective.bytes.len(),
                    applied.changed,
                    preview
                );
            } else {
                newengine_ulog_api::ulog::debug!(
                    "plugins: config effective id='{}' content_type='{}' len={} changed={} changed_keys=[{}] preview='{}'",
                    id_str,
                    applied.effective.content_type,
                    applied.effective.bytes.len(),
                    applied.changed,
                    changed_keys.join(", "),
                    preview
                );
            }

            for d in applied.diags.iter() {
                match d.level {
                    ConfigDiagLevelV1::Info => newengine_ulog_api::ulog::info!(
                        "plugins: config info id='{}' {} {}",
                        id_str,
                        d.code,
                        d.message
                    ),
                    ConfigDiagLevelV1::Warn => newengine_ulog_api::ulog::warn!(
                        "plugins: config warn id='{}' {} {}",
                        id_str,
                        d.code,
                        d.message
                    ),
                    ConfigDiagLevelV1::Error => newengine_ulog_api::ulog::error!(
                        "plugins: config error id='{}' {} {}",
                        id_str,
                        d.code,
                        d.message
                    ),
                }
            }

            let t0 = Instant::now();
            module_any
                .module
                .init(host.clone(), applied.effective)
                .into_result()
                .map_err(|e| e.to_string())?;
            t.init_ms = t0.elapsed().as_millis();
            Ok(t)
        })
    }));

    match init_res {
        Ok(Ok(t)) => Ok(t),
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
        with_current_plugin_id(id_str, || module_any.module.shutdown())
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
