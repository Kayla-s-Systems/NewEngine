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

        if !plugin_scan_profile_matches(&path, desired_profile, allow_mixed_profiles) {
            skipped_profile_mismatch = skipped_profile_mismatch.saturating_add(1);
            log::info!(
                "plugins: scan skipped profile mismatch path='{}' desired='{}' actual='{}' mixed_allowed={}",
                display_clean(&path),
                desired_profile,
                plugin_file_profile_for_scan(&path),
                allow_mixed_profiles
            );
            continue;
        }

        dynlib_paths.push(path);
    }

    // Importer worker DLLs are private to AssetManager.
    // The plugin host must not scan plugins/importers as runtime plugins.
    if skipped_profile_mismatch > 0 {
        log::warn!(
            "plugins: scan skipped {} dynamic libraries with mismatched build profile desired='{}' mixed_allowed={}",
            skipped_profile_mismatch,
            desired_profile,
            allow_mixed_profiles
        );
    }

    dynlib_paths.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));

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
                version: "-".to_owned(),
            },
        });
    }

    let manifest_entry = manifest.and_then(|m| m.match_file_name(&file_name));
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
        let kind = apply_manifest_overlay(kind, manifest_entry);
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

fn apply_manifest_overlay(
    kind: ScannedDynlibKind,
    manifest_entry: Option<&super::manifest::ManifestPluginEntry>,
) -> ScannedDynlibKind {
    let Some(entry) = manifest_entry else {
        return kind;
    };

    match kind {
        ScannedDynlibKind::Plugin {
            id,
            version,
            phase,
            descriptor_kind,
            declared_capabilities,
        } => {
            let manifest_id = entry.id.trim();
            let id = if id == "<unknown-plugin>" && !manifest_id.is_empty() {
                manifest_id.to_owned()
            } else {
                if !manifest_id.is_empty() && manifest_id != id {
                    log::warn!(
                        "plugins: manifest id '{}' does not match plugin descriptor id '{}'",
                        manifest_id,
                        id
                    );
                }
                id
            };

            // Deployment manifest is allowed to override host load phase and kind.
            // The descriptor still remains the source of truth for capabilities.
            let phase = if entry.phase.trim().is_empty() {
                phase
            } else {
                entry.phase_value()
            };
            let descriptor_kind = if entry.kind.trim().is_empty() {
                descriptor_kind
            } else {
                Some(entry.kind_value())
            };

            ScannedDynlibKind::Plugin {
                id,
                version,
                phase,
                descriptor_kind,
                declared_capabilities,
            }
        }
        other => other,
    }
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
