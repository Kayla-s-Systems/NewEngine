#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};

use libloading::Library;

use super::graph::{DiscoveryGraph, ScannedDynlib, ScannedDynlibKind};
use super::manifest::PluginManifest;
use super::metadata::{
    build_scanned_plugin_kind, platform_runtime_identity_from_probe, probe_plugin_metadata,
    PLATFORM_RUNTIME_SYMBOL,
};
use crate::manager::types::PluginLoadError;
use crate::path_fmt::display_clean;
use crate::paths::is_dynamic_lib;


#[inline]
fn desired_scan_profile() -> &'static str {
    if cfg!(debug_assertions) { "dev" } else { "release" }
}

#[inline]
fn mixed_scan_profile_allowed() -> bool {
    std::env::var("NEWENGINE_ALLOW_MIXED_PLUGIN_PROFILE")
        .ok()
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

#[inline]
fn plugin_file_profile_for_scan(path: &Path) -> &'static str {
    let lower = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if lower.contains("-release.") || lower.contains("-release-") {
        "release"
    } else if lower.contains("-dev.") || lower.contains("-dev-") {
        "dev"
    } else if lower.contains("-debug.") || lower.contains("-debug-") {
        "dev"
    } else if lower.contains("-test.") || lower.contains("-test-") {
        "test"
    } else if lower.contains("-bench.") || lower.contains("-bench-") {
        "bench"
    } else {
        "unknown"
    }
}

#[inline]
fn plugin_scan_profile_matches(path: &Path, desired: &'static str, allow_mixed: bool) -> bool {
    if allow_mixed {
        return true;
    }

    match plugin_file_profile_for_scan(path) {
        "unknown" => true,
        actual => actual == desired,
    }
}


#[inline]
fn is_platform_runtime_filename(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.contains("platform-winit") || lower.contains("winit-platform")
}

pub(super) fn scan_plugins_dir(dir: &Path) -> Result<DiscoveryGraph, PluginLoadError> {
    let rd = std::fs::read_dir(dir).map_err(|e| PluginLoadError {
        path: dir.to_path_buf(),
        message: format!("read_dir failed: {e}"),
    })?;

    let manifest = PluginManifest::load_from_plugins_dir(dir);

    let mut entries_total: usize = 0;
    let mut skipped_non_dynlib: usize = 0;
    let mut dynlib_paths_all: Vec<PathBuf> = Vec::new();
    let mut dynlib_paths: Vec<PathBuf> = Vec::new();
    let mut scan_errors: Vec<String> = Vec::new();
    let desired_profile = desired_scan_profile();
    let allow_mixed_profiles = mixed_scan_profile_allowed();
    let mut skipped_profile_mismatch: usize = 0;

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

        dynlib_paths_all.push(path.clone());
        if plugin_scan_profile_matches(&path, desired_profile, allow_mixed_profiles) {
            dynlib_paths.push(path);
        } else {
            skipped_profile_mismatch = skipped_profile_mismatch.saturating_add(1);
            log::info!(
                "plugins: scan observed profile mismatch path='{}' desired='{}' actual='{}' mixed_allowed={}",
                display_clean(&path),
                desired_profile,
                plugin_file_profile_for_scan(&path),
                allow_mixed_profiles
            );
        }
    }

    dynlib_paths_all.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
    dynlib_paths.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));

    // Standalone game/editor binaries are often launched from a dev cargo profile
    // while the runtime plugins were built through the release plugin profile.
    // Strictly filtering those DLLs out makes the engine fail with misleading
    // "manifest missing required plugin" errors even though valid ABI-stable
    // plugin DLLs are present. Match the platform runtime resolver policy:
    // prefer the requested profile, but fall back to available plugin profiles
    // when the requested profile cannot satisfy the required manifest entries.
    if !allow_mixed_profiles && skipped_profile_mismatch > 0 {
        let matching_missing = manifest
            .as_ref()
            .map(|m| m.required_entries_missing_from(&dynlib_paths))
            .unwrap_or_default();
        let fallback_missing = manifest
            .as_ref()
            .map(|m| m.required_entries_missing_from(&dynlib_paths_all))
            .unwrap_or_default();
        let should_fallback = dynlib_paths.is_empty()
            || (!matching_missing.is_empty() && fallback_missing.len() < matching_missing.len());

        if should_fallback && !dynlib_paths_all.is_empty() {
            log::warn!(
                "plugins: desired '{}' profile did not satisfy runtime plugin manifest; falling back to available plugin DLL profiles mismatched={} missing_before={} missing_after={}",
                desired_profile,
                skipped_profile_mismatch,
                matching_missing.len(),
                fallback_missing.len(),
            );
            dynlib_paths = dynlib_paths_all.clone();
        } else {
            log::warn!(
                "plugins: scan skipped {} dynamic libraries with mismatched build profile desired='{}' mixed_allowed={}",
                skipped_profile_mismatch,
                desired_profile,
                allow_mixed_profiles
            );
        }
    }

    let mut items: Vec<ScannedDynlib> = Vec::with_capacity(dynlib_paths.len());

    if let Some(manifest) = &manifest {
        for missing in manifest.required_entries_missing_from(&dynlib_paths) {
            scan_errors.push(format!("manifest missing required plugin: {missing}"));
        }
    }

    for path in dynlib_paths {
        match scan_dynamic_lib(&path, manifest.as_ref()) {
            Ok(v) => items.push(v),
            Err(e) => {
                log::warn!("plugins: scan failed for '{}': {}", display_clean(&path), e);
                scan_errors.push(format!("{}: {}", display_clean(&path), e));
            }
        }
    }

    items.sort_by(|a, b| sort_key(&a.path).cmp(&sort_key(&b.path)));

    let mut platform_runtime_count = 0usize;
    let mut bootstrap_total = 0usize;
    let mut engine_total = 0usize;
    let mut unknown_dynlibs: Vec<String> = Vec::new();

    for item in &items {
        match &item.kind {
            ScannedDynlibKind::PlatformRuntime { .. } => {
                platform_runtime_count = platform_runtime_count.saturating_add(1);
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
        bootstrap_total,
        engine_total,
        unknown_dynlibs,
    })
}

fn scan_dynamic_lib(path: &Path, manifest: Option<&PluginManifest>) -> Result<ScannedDynlib, String> {
    let file_name = file_name_only(path);

    // Platform runtimes are invoked by `newengine-runtime-host`, not loaded as
    // normal engine plugins. Do not dlopen/probe them during bootstrap scan:
    // stale or profile-mismatched runtime DLLs can otherwise crash the process
    // before platform-runtime diagnostics even start.
    if is_platform_runtime_filename(&file_name) {
        return Ok(ScannedDynlib {
            path: path.to_path_buf(),
            file_name,
            kind: ScannedDynlibKind::PlatformRuntime {
                id: "newengine.platform.winit".to_owned(),
                version: infer_version_from_file_name(path),
            },
        });
    }

    let manifest_entry = manifest.and_then(|m| m.match_file_name(&file_name));
    if let Some(entry) = manifest_entry {
        return Ok(ScannedDynlib {
            path: path.to_path_buf(),
            file_name,
            kind: ScannedDynlibKind::Plugin {
                id: entry.id.clone(),
                version: infer_version_from_file_name(path),
                phase: entry.phase_value(),
                descriptor_kind: Some(entry.kind_value()),
                declared_capabilities: None,
                provides_render_backend: is_render_backend_plugin_id(&entry.id),
                provides_render_service: is_render_backend_plugin_id(&entry.id),
            },
        });
    }

    if !metadata_probe_enabled() {
        return Ok(ScannedDynlib {
            path: path.to_path_buf(),
            file_name: file_name.clone(),
            kind: infer_kind_from_file_name(path, &file_name),
        });
    }

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

    if let Some(kind) = build_scanned_plugin_kind(&plugin_probe) {
        return Ok(ScannedDynlib {
            path: path.to_path_buf(),
            file_name,
            kind,
        });
    }

    Ok(ScannedDynlib {
        path: path.to_path_buf(),
        file_name,
        kind: ScannedDynlibKind::Unknown,
    })
}

#[inline]
fn metadata_probe_enabled() -> bool {
    std::env::var("NEWENGINE_PLUGIN_DISCOVERY_ABI_PROBE")
        .ok()
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

fn infer_kind_from_file_name(path: &Path, file_name: &str) -> ScannedDynlibKind {
    let lower = file_name.to_ascii_lowercase();
    let version = infer_version_from_file_name(path);
    let plugin = |id: &str, phase, descriptor_kind| ScannedDynlibKind::Plugin {
        id: id.to_owned(),
        version: version.clone(),
        phase,
        descriptor_kind: Some(descriptor_kind),
        declared_capabilities: None,
        provides_render_backend: is_render_backend_plugin_id(id),
        provides_render_service: is_render_backend_plugin_id(id),
    };

    if lower.starts_with("logging-") {
        return plugin(
            "newengine.logging",
            newengine_plugin_api::PluginBootstrapPhase::Bootstrap,
            newengine_plugin_api::PluginKind::Runtime,
        );
    }
    if lower.starts_with("input-") {
        return plugin(
            "newengine.input",
            newengine_plugin_api::PluginBootstrapPhase::Bootstrap,
            newengine_plugin_api::PluginKind::Runtime,
        );
    }
    if lower.starts_with("assetmanager-") {
        return plugin(
            "newengine.assets",
            newengine_plugin_api::PluginBootstrapPhase::Engine,
            newengine_plugin_api::PluginKind::Runtime,
        );
    }
    if lower.starts_with("vulkan_renderer-") {
        return plugin(
            "newengine.renderer.vulkan",
            newengine_plugin_api::PluginBootstrapPhase::Engine,
            newengine_plugin_api::PluginKind::Runtime,
        );
    }

    if lower.starts_with("null_renderer-") {
        return plugin(
            "newengine.renderer.null",
            newengine_plugin_api::PluginBootstrapPhase::Engine,
            newengine_plugin_api::PluginKind::Runtime,
        );
    }
    if lower.starts_with("egui_ui_provider-") {
        return plugin(
            "newengine.ui.provider.egui",
            newengine_plugin_api::PluginBootstrapPhase::Engine,
            newengine_plugin_api::PluginKind::Runtime,
        );
    }
    if lower.starts_with("newengine_modules_math-") {
        return plugin(
            "newengine.math",
            newengine_plugin_api::PluginBootstrapPhase::Bootstrap,
            newengine_plugin_api::PluginKind::Runtime,
        );
    }

    ScannedDynlibKind::Unknown
}

#[inline]
fn is_render_backend_plugin_id(id: &str) -> bool {
    id == "newengine.renderer.null" || id.starts_with("newengine.renderer.")
}

fn infer_version_from_file_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let parts: Vec<&str> = stem.split('-').collect();
    let Some(idx) = parts.iter().position(|p| p.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)) else {
        return "-".to_owned();
    };
    let raw = parts[idx..].join("-");
    raw.strip_suffix("-dev")
        .or_else(|| raw.strip_suffix("-debug"))
        .or_else(|| raw.strip_suffix("-release"))
        .or_else(|| raw.strip_suffix("-test"))
        .or_else(|| raw.strip_suffix("-bench"))
        .unwrap_or(raw.as_str())
        .to_owned()
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
