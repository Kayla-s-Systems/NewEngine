use newengine_math::collections_prelude::NeHashSet as HashSet;
use std::path::{Path, PathBuf};

use libloading::Library;
use newengine_core::{EngineError, EngineResult};
use newengine_platform_api::PlatformRuntimeRunFnV1;
use newengine_plugin_api::{PluginRootV1Ref, PluginSignatureV1};

use crate::platform_runtime::constants::{
    PLATFORM_PLUGIN_ID, PLATFORM_RUNTIME_SYMBOL, PLUGIN_ROOT_SYMBOL, PLUGIN_SIGNATURE_SYMBOL,
};

#[inline]
fn desired_runtime_profile() -> &'static str {
    if cfg!(debug_assertions) {
        "dev"
    } else {
        "release"
    }
}

#[inline]
fn mixed_plugin_profile_allowed() -> bool {
    std::env::var("NEWENGINE_ALLOW_MIXED_PLUGIN_PROFILE")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

#[inline]
fn runtime_file_profile(path: &Path) -> &'static str {
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
fn runtime_profile_matches(path: &Path, desired: &'static str, allow_mixed: bool) -> bool {
    if allow_mixed {
        return true;
    }

    match runtime_file_profile(path) {
        "unknown" => true,
        actual => actual == desired,
    }
}

#[inline]
fn runtime_metadata_probe_enabled() -> bool {
    std::env::var("NEWENGINE_PLATFORM_RUNTIME_METADATA_PROBE")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

#[inline]
fn runtime_symbol_validation_enabled() -> bool {
    std::env::var("NEWENGINE_PLATFORM_RUNTIME_VALIDATE_SYMBOL")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(true)
}

#[inline]
fn legacy_platform_runtime_name_allowed() -> bool {
    std::env::var("NEWENGINE_ALLOW_LEGACY_PLATFORM_RUNTIME_NAME")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

#[inline]
fn is_legacy_platform_runtime_filename(path: &Path) -> bool {
    let lower = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    [
        "platform-winit-",
        "winit-platform-plugin-",
        "engine.platform.winit-",
        "engine-platform-winit-",
        "winit-",
    ]
    .iter()
    .any(|prefix| {
        lower
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
    })
}

#[inline]
fn is_platform_runtime_filename_candidate(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };

    let lower = name.to_ascii_lowercase();
    (lower.ends_with(".dll") || lower.ends_with(".so") || lower.ends_with(".dylib"))
        && (lower.contains("platform-winit") || lower.contains("winit-platform"))
}

fn runtime_symbol_present(path: &Path) -> bool {
    let Ok(lib) = (unsafe { libloading::Library::new(path) }) else {
        return false;
    };

    unsafe { lib.get::<PlatformRuntimeRunFnV1>(PLATFORM_RUNTIME_SYMBOL) }.is_ok()
}

fn try_read_runtime_identity(path: &Path) -> Option<(String, String)> {
    let lib = unsafe { Library::new(path) }.ok()?;
    let has_runtime = unsafe { lib.get::<PlatformRuntimeRunFnV1>(PLATFORM_RUNTIME_SYMBOL) }.is_ok();
    if !has_runtime {
        return None;
    }

    if let Ok(sym) =
        unsafe { lib.get::<unsafe extern "C" fn() -> PluginSignatureV1>(PLUGIN_SIGNATURE_SYMBOL) }
    {
        let signature = unsafe { sym() };
        let id = signature.id.to_string();
        let version = signature.version.to_string();
        if !id.trim().is_empty() {
            return Some((id, version));
        }
    }

    let root_sym =
        unsafe { lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(PLUGIN_ROOT_SYMBOL) }.ok()?;
    let root = unsafe { root_sym() };

    let module = root.create()();
    let descriptor = module.descriptor();
    Some((descriptor.id.to_string(), descriptor.version.to_string()))
}

pub fn detect_platform_runtime_path(modules_dir: &Path) -> EngineResult<PathBuf> {
    crate::platform_early_log!(
        "host.discovery.begin modules_dir='{}' desired_profile='{}' allow_mixed={}",
        modules_dir.display(),
        desired_runtime_profile(),
        mixed_plugin_profile_allowed()
    );
    #[inline]
    fn collect_candidates(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };

        let validate_symbol = runtime_symbol_validation_enabled();
        for ent in rd.flatten() {
            let path = ent.path();
            if !is_platform_runtime_filename_candidate(&path) {
                continue;
            }

            if is_legacy_platform_runtime_filename(&path) && !legacy_platform_runtime_name_allowed()
            {
                crate::platform_early_log!(
                    "host.discovery.skip_legacy_platform_name path='{}'",
                    path.display()
                );
                continue;
            }

            if validate_symbol && !runtime_symbol_present(&path) {
                crate::platform_early_log!(
                    "host.discovery.skip_symbol_absent path='{}'",
                    path.display()
                );
                continue;
            }

            out.push(path);
        }
    }

    if let Some(explicit) = std::env::var_os("NEWENGINE_PLATFORM_RUNTIME") {
        let explicit = PathBuf::from(explicit);
        crate::platform_early_log!(
            "host.discovery.explicit path='{}' exists={}",
            explicit.display(),
            explicit.is_file()
        );
        if explicit.is_file() {
            return Ok(explicit);
        }

        return Err(EngineError::other(format!(
            "NEWENGINE_PLATFORM_RUNTIME points to missing file '{}'",
            explicit.display()
        )));
    }

    let exe_dir = std::env::current_exe()
        .map_err(|e| EngineError::other(format!("current_exe failed: {e}")))?
        .parent()
        .ok_or_else(|| EngineError::other("current_exe has no parent"))?
        .to_path_buf();

    let mut search_dirs: Vec<PathBuf> = Vec::new();

    if let Some(dir) = env_dir("NEWENGINE_PLATFORM_RUNTIME_DIR") {
        push_runtime_dirs(&mut search_dirs, &dir);
    }
    for key in ["NEWENGINE_PLUGIN_DIR", "NEWENGINE_PLUGINS_DIR"] {
        if let Some(dir) = env_dir(key) {
            push_runtime_dirs(&mut search_dirs, &dir);
        }
    }

    push_runtime_dirs(&mut search_dirs, &exe_dir.join("platforms"));
    push_runtime_dirs(&mut search_dirs, &exe_dir.join("pluginsRuntime"));
    push_runtime_dirs(&mut search_dirs, &exe_dir.join("plugins"));

    for dir in resolve_module_dir_candidates(modules_dir, &exe_dir) {
        push_runtime_dirs(&mut search_dirs, &dir);
    }

    push_ancestor_plugin_dirs(&mut search_dirs, &exe_dir);
    if let Ok(cwd) = std::env::current_dir() {
        push_runtime_dirs(&mut search_dirs, &cwd.join("platforms"));
        push_runtime_dirs(&mut search_dirs, &cwd.join("pluginsRuntime"));
        push_runtime_dirs(&mut search_dirs, &cwd.join("plugins"));
        push_ancestor_plugin_dirs(&mut search_dirs, &cwd);
    }

    push_unique(&mut search_dirs, exe_dir.clone());

    let mut dedup = HashSet::default();
    search_dirs.retain(|p| dedup.insert(p.clone()));

    let mut candidates: Vec<PathBuf> = Vec::new();
    for dir in &search_dirs {
        collect_candidates(dir, &mut candidates);
    }

    candidates.sort();
    candidates.dedup();
    crate::platform_early_log!("host.discovery.candidates.total={}", candidates.len());
    for path in candidates.iter().take(16) {
        crate::platform_early_log!(
            "host.discovery.candidate path='{}' profile='{}'",
            path.display(),
            runtime_file_profile(path)
        );
    }

    let desired_profile = desired_runtime_profile();
    let allow_mixed_profiles = mixed_plugin_profile_allowed();
    let all_profile_candidates = candidates.clone();
    let skipped_profile_mismatch = candidates
        .iter()
        .filter(|path| !runtime_profile_matches(path, desired_profile, allow_mixed_profiles))
        .count();
    candidates.retain(|path| runtime_profile_matches(path, desired_profile, allow_mixed_profiles));
    if skipped_profile_mismatch > 0 {
        crate::platform_early_log!(
            "host.discovery.skipped_profile_mismatch={} desired='{}' mixed_allowed={}",
            skipped_profile_mismatch,
            desired_profile,
            allow_mixed_profiles
        );
        newengine_ulog_api::ulog::warn!(
            "platform runtime discovery: skipped_profile_mismatch={} desired='{}' mixed_allowed={}",
            skipped_profile_mismatch,
            desired_profile,
            allow_mixed_profiles
        );
    }

    if candidates.is_empty() && !all_profile_candidates.is_empty() {
        candidates = all_profile_candidates;
        sort_runtime_candidates(&mut candidates, desired_profile);
        crate::platform_early_log!(
            "host.discovery.profile_fallback.enabled desired='{}' candidates={}",
            desired_profile,
            candidates.len()
        );
        newengine_ulog_api::ulog::warn!(
            "platform runtime discovery: no '{}' platform DLL found; falling back to available platform runtime profile. Rebuild winit-platform-plugin with matching profile to remove this warning.",
            desired_profile
        );
    } else {
        sort_runtime_candidates(&mut candidates, desired_profile);
    }

    crate::platform_early_log!(
        "host.discovery.candidates.after_profile_filter={}",
        candidates.len()
    );

    if runtime_metadata_probe_enabled() {
        if let Some(path) = candidates
            .iter()
            .find(|path| {
                matches!(try_read_runtime_identity(path), Some((ref id, _)) if id == PLATFORM_PLUGIN_ID)
            })
            .cloned()
        {
            crate::platform_early_log!("host.discovery.selected.identity path='{}'", path.display());
            return Ok(path);
        }
    } else {
        crate::platform_early_log!("host.discovery.metadata_probe.disabled");
    }

    if let Some(path) = candidates
        .iter()
        .find(|path| {
            path.file_name()
                .and_then(|s| s.to_str())
                .map(|name| name.to_ascii_lowercase().contains("winit-platform"))
                .unwrap_or(false)
        })
        .cloned()
    {
        crate::platform_early_log!("host.discovery.selected.filename path='{}'", path.display());
        return Ok(path);
    }

    if let Some(path) = candidates.into_iter().next() {
        crate::platform_early_log!("host.discovery.selected.fallback path='{}'", path.display());
        return Ok(path);
    }

    crate::platform_early_log!("host.discovery.failed.no_candidate");
    Err({
        EngineError::other(format!(
            "platform runtime DLL not found; searched [{}] and expected exported symbol 'newengine_platform_runtime_run_v1'. Build/copy the platform runtime into NEWENGINE_PLUGIN_DIR, NEWENGINE_PLATFORM_RUNTIME_DIR, <engine-root>/pluginsRuntime, legacy <engine-root>/plugins, or set NEWENGINE_PLATFORM_RUNTIME to the exact DLL path.",
            search_dirs
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })
}

fn is_current_dir_path(path: &Path) -> bool {
    path.as_os_str().is_empty()
        || path == Path::new(".")
        || path == Path::new("./")
        || path == Path::new(".\\")
}

fn env_dir(name: &str) -> Option<PathBuf> {
    let raw = std::env::var_os(name)?;
    let path = PathBuf::from(raw);
    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path)
    }
}

fn push_unique(out: &mut Vec<PathBuf>, path: PathBuf) {
    if !out.iter().any(|p| p == &path) {
        out.push(path);
    }
}

fn push_runtime_dirs(out: &mut Vec<PathBuf>, dir: &Path) {
    let is_platforms_dir = dir
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("platforms"))
        .unwrap_or(false);

    if !is_platforms_dir {
        push_unique(out, dir.join("platforms"));
    }
    push_unique(out, dir.to_path_buf());
}

fn sort_runtime_candidates(candidates: &mut [PathBuf], desired_profile: &'static str) {
    candidates.sort_by(|a, b| {
        runtime_candidate_rank(a, desired_profile).cmp(&runtime_candidate_rank(b, desired_profile))
    });
}

fn runtime_candidate_rank(path: &Path, desired_profile: &'static str) -> (u8, u8, String) {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let profile_rank = match runtime_file_profile(path) {
        p if p == desired_profile => 0,
        "unknown" => 1,
        "dev" => 2,
        "release" => 3,
        "test" => 4,
        "bench" => 5,
        _ => 9,
    };
    let name_rank = if name.contains("winit-platform") {
        0
    } else {
        1
    };
    (profile_rank, name_rank, name)
}

fn push_ancestor_plugin_dirs(out: &mut Vec<PathBuf>, start: &Path) {
    for ancestor in start.ancestors() {
        push_runtime_dirs(out, &ancestor.join("pluginsRuntime"));
        push_runtime_dirs(out, &ancestor.join("plugins"));
    }
}

fn resolve_module_dir_candidates(modules_dir: &Path, exe_dir: &Path) -> Vec<PathBuf> {
    if is_current_dir_path(modules_dir) {
        return vec![exe_dir.to_path_buf()];
    }

    if modules_dir.is_absolute() {
        return vec![modules_dir.to_path_buf()];
    }

    let mut out = Vec::new();
    push_unique(&mut out, exe_dir.join(modules_dir));

    if let Ok(cwd) = std::env::current_dir() {
        push_unique(&mut out, cwd.join(modules_dir));
        for ancestor in cwd.ancestors() {
            push_unique(&mut out, ancestor.join(modules_dir));
        }
    }

    for ancestor in exe_dir.ancestors() {
        push_unique(&mut out, ancestor.join(modules_dir));
    }

    out
}
