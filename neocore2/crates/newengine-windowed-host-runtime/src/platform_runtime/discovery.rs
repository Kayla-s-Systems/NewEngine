use newengine_math::collections_prelude::NeHashSet as HashSet;
use std::path::{Path, PathBuf};

use newengine_core::{EngineError, EngineResult};
use newengine_platform_api::PlatformRuntimeDescriptorV1;

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
    newengine_plugin_host::current_host_context()
        .environment_var("NEWENGINE_ALLOW_MIXED_PLUGIN_PROFILE")
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
    } else if lower.contains("-dev.")
        || lower.contains("-dev-")
        || lower.contains("-debug.")
        || lower.contains("-debug-")
    {
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
fn is_platform_runtime_filename_candidate(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "dll" | "so" | "dylib"))
        .unwrap_or(false)
}

pub(crate) fn try_read_runtime_descriptor(
    path: &Path,
) -> Option<PlatformRuntimeDescriptorV1> {
    let manifest = newengine_plugin_host::read_verified_plugin_discovery_manifest(path).ok()?;
    let platform = manifest.platform_runtime?;
    Some(
        PlatformRuntimeDescriptorV1::new(platform.id, platform.name, platform.version)
            .with_backend_priority(platform.backend_priority)
            .with_system_tags(platform.system_tags),
    )
}

fn normalized_runtime_tags(descriptor: &PlatformRuntimeDescriptorV1) -> Vec<String> {
    let mut tags = descriptor
        .system_tags
        .iter()
        .filter_map(|tag| newengine_service_api::normalize_system_tag(tag.as_str()))
        .collect::<Vec<_>>();
    tags.sort();
    tags.dedup();
    tags
}

fn runtime_descriptor_allowed(descriptor: Option<&PlatformRuntimeDescriptorV1>) -> bool {
    let Some(descriptor) = descriptor else {
        return !newengine_plugin_host::engine_composition_has_forbidden_system_tags();
    };
    let tags = normalized_runtime_tags(descriptor);
    newengine_plugin_host::engine_composition_allows_system_tags(&tags)
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
        for ent in rd.flatten() {
            let path = ent.path();
            if !is_platform_runtime_filename_candidate(&path) {
                continue;
            }
            let descriptor = try_read_runtime_descriptor(&path);
            if descriptor.is_none() {
                continue;
            }
            if !runtime_descriptor_allowed(descriptor.as_ref()) {
                crate::platform_early_log!(
                    "host.discovery.skip_composition_tags path='{}'",
                    path.display()
                );
                continue;
            }
            out.push(path);
        }
    }

    if let Some(explicit) = newengine_plugin_host::current_host_context()
        .environment_var_os("NEWENGINE_PLATFORM_RUNTIME")
    {
        let explicit = PathBuf::from(explicit);
        crate::platform_early_log!(
            "host.discovery.explicit path='{}' exists={}",
            explicit.display(),
            explicit.is_file()
        );
        if explicit.is_file() {
            let descriptor = try_read_runtime_descriptor(&explicit);
            if descriptor.is_none() {
                return Err(EngineError::other(format!(
                    "NEWENGINE_PLATFORM_RUNTIME '{}' has no valid verified platform-runtime discovery sidecar",
                    explicit.display()
                )));
            }
            if !runtime_descriptor_allowed(descriptor.as_ref()) {
                return Err(EngineError::other(format!(
                    "NEWENGINE_PLATFORM_RUNTIME '{}' is incompatible with the active composition tags",
                    explicit.display()
                )));
            }
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
            "platform runtime discovery: no '{}' platform runtime build found; falling back to another available runtime profile.",
            desired_profile
        );
    } else {
        sort_runtime_candidates(&mut candidates, desired_profile);
    }

    crate::platform_early_log!(
        "host.discovery.candidates.after_profile_filter={}",
        candidates.len()
    );

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
    let raw = newengine_plugin_host::current_host_context().environment_var_os(name)?;
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

fn runtime_candidate_rank(path: &Path, desired_profile: &'static str) -> (u8, i32, String) {
    let profile_rank = match runtime_file_profile(path) {
        p if p == desired_profile => 0,
        "unknown" => 1,
        "dev" => 2,
        "release" => 3,
        "test" => 4,
        "bench" => 5,
        _ => 9,
    };
    let priority_rank = try_read_runtime_descriptor(path)
        .map(|descriptor| descriptor.backend_priority.saturating_neg())
        .unwrap_or(0);
    (profile_rank, priority_rank, path.to_string_lossy().to_string())
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
