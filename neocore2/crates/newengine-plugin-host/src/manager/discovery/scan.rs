#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};

use libloading::Library;

use super::graph::{DiscoveryGraph, ScannedDynlib, ScannedDynlibKind};
use super::metadata::{
    build_scanned_plugin_kind, infer_render_backend_identity, platform_runtime_identity_from_probe,
    probe_plugin_metadata, PLATFORM_RUNTIME_SYMBOL, RENDER_BACKEND_SYMBOL,
};
use crate::manager::types::PluginLoadError;
use crate::path_fmt::display_clean;
use crate::paths::is_dynamic_lib;

pub(super) fn scan_plugins_dir(dir: &Path) -> Result<DiscoveryGraph, PluginLoadError> {
    let rd = std::fs::read_dir(dir).map_err(|e| PluginLoadError {
        path: dir.to_path_buf(),
        message: format!("read_dir failed: {e}"),
    })?;

    let mut entries_total: usize = 0;
    let mut skipped_non_dynlib: usize = 0;
    let mut dynlib_paths: Vec<PathBuf> = Vec::new();

    for ent in rd {
        entries_total = entries_total.saturating_add(1);

        let ent = ent.map_err(|e| PluginLoadError {
            path: dir.to_path_buf(),
            message: format!("read_dir entry failed: {e}"),
        })?;

        let path = ent.path();
        if !is_dynamic_lib(&path) {
            skipped_non_dynlib = skipped_non_dynlib.saturating_add(1);
            continue;
        }

        dynlib_paths.push(path);
    }

    dynlib_paths.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));

    let mut items: Vec<ScannedDynlib> = Vec::with_capacity(dynlib_paths.len());
    let mut scan_errors: Vec<String> = Vec::new();

    for path in dynlib_paths {
        match scan_dynamic_lib(&path) {
            Ok(v) => items.push(v),
            Err(e) => {
                log::warn!("plugins: scan failed for '{}': {}", display_clean(&path), e);
                scan_errors.push(format!("{}: {}", display_clean(&path), e));
            }
        }
    }

    items.sort_by(|a, b| sort_key(&a.path).cmp(&sort_key(&b.path)));

    let mut platform_runtime_count = 0usize;
    let mut legacy_render_backend_count = 0usize;
    let mut bootstrap_total = 0usize;
    let mut engine_total = 0usize;
    let mut unknown_dynlibs: Vec<String> = Vec::new();

    for item in &items {
        match &item.kind {
            ScannedDynlibKind::PlatformRuntime { .. } => {
                platform_runtime_count = platform_runtime_count.saturating_add(1);
            }
            ScannedDynlibKind::LegacyRenderBackend { .. } => {
                legacy_render_backend_count = legacy_render_backend_count.saturating_add(1);
            }
            ScannedDynlibKind::Plugin { phase, .. } => match phase {
                newengine_plugin_api::PluginBootstrapPhase::Bootstrap => {
                    bootstrap_total = bootstrap_total.saturating_add(1);
                }
                newengine_plugin_api::PluginBootstrapPhase::Platform
                | newengine_plugin_api::PluginBootstrapPhase::Engine => {
                    engine_total = engine_total.saturating_add(1);
                }
            },
            ScannedDynlibKind::Unknown => {
                unknown_dynlibs.push(item.file_name.clone());
            }
        }
    }

    Ok(DiscoveryGraph {
        dir: dir.to_path_buf(),
        entries_total,
        skipped_non_dynlib,
        items,
        scan_errors,
        platform_runtime_count,
        legacy_render_backend_count,
        bootstrap_total,
        engine_total,
        unknown_dynlibs,
    })
}

fn scan_dynamic_lib(path: &Path) -> Result<ScannedDynlib, String> {
    let file_name = file_name_only(path);
    let lib = unsafe { Library::new(path) }.map_err(|e| format!("Library::new failed: {e}"))?;
    let plugin_probe = probe_plugin_metadata(&lib)?;

    if unsafe { lib.get::<unsafe extern "C" fn()>(PLATFORM_RUNTIME_SYMBOL) }.is_ok() {
        let (id, version) = platform_runtime_identity_from_probe(path, &plugin_probe);
        return Ok(ScannedDynlib {
            path: path.to_path_buf(),
            file_name,
            kind: ScannedDynlibKind::PlatformRuntime { id, version },
        });
    }

    let has_legacy_render_backend_abi =
        unsafe { lib.get::<unsafe extern "C" fn()>(RENDER_BACKEND_SYMBOL) }.is_ok();

    if let Some(kind) = build_scanned_plugin_kind(&plugin_probe) {
        return Ok(ScannedDynlib {
            path: path.to_path_buf(),
            file_name,
            kind,
        });
    }

    if has_legacy_render_backend_abi {
        let (id, version) = infer_render_backend_identity(path);
        return Ok(ScannedDynlib {
            path: path.to_path_buf(),
            file_name,
            kind: ScannedDynlibKind::LegacyRenderBackend { id, version },
        });
    }

    Ok(ScannedDynlib {
        path: path.to_path_buf(),
        file_name,
        kind: ScannedDynlibKind::Unknown,
    })
}

#[inline]
fn file_name_only(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "<unnamed>".to_owned())
}

#[inline]
fn sort_key(path: &Path) -> (String, String) {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "<unnamed>".to_owned());
    (file_name, display_clean(path))
}
