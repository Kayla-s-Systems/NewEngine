use newengine_math::collections_prelude::NeHashSet as HashSet;
use std::path::{Path, PathBuf};

use abi_stable::std_types::RString;
use libloading::Library;
use newengine_core::{EngineError, EngineResult};
use newengine_platform_api::{PlatformAppConfigV1, PlatformHostApiV1, PlatformRuntimeRunFnV1};
use newengine_plugin_api::{HostApiV1, PluginRootV1Ref, PluginSignatureV1};

use crate::platform_runtime::constants::{
    PLATFORM_PLUGIN_ID, PLATFORM_RUNTIME_SYMBOL, PLUGIN_ROOT_SYMBOL,
    PLUGIN_SIGNATURE_SYMBOL,
};

fn try_read_runtime_identity(path: &Path) -> Option<(String, String)> {
    let lib = unsafe { Library::new(path) }.ok()?;
    let has_runtime =
        unsafe { lib.get::<PlatformRuntimeRunFnV1>(PLATFORM_RUNTIME_SYMBOL) }.is_ok();
    if !has_runtime {
        return None;
    }

    if let Ok(sym) = unsafe {
        lib.get::<unsafe extern "C" fn() -> PluginSignatureV1>(PLUGIN_SIGNATURE_SYMBOL)
    } {
        let signature = unsafe { sym() };
        let id = signature.id.to_string();
        let version = signature.version.to_string();
        if !id.trim().is_empty() {
            return Some((id, version));
        }
    }

    let root_sym = unsafe {
        lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(PLUGIN_ROOT_SYMBOL)
    }
        .ok()?;
    let root = unsafe { root_sym() };

    if let Some(create_v3) = root.create_v3() {
        let module = create_v3();
        let descriptor = module.descriptor_v3();
        return Some((descriptor.id.to_string(), descriptor.version.to_string()));
    }

    if let Some(create_v2) = root.create_v2() {
        let module = create_v2();
        let descriptor = module.descriptor();
        return Some((descriptor.id.to_string(), descriptor.version.to_string()));
    }

    let module = root.create()();
    let info = module.info();
    Some((info.id.to_string(), info.version.to_string()))
}

pub fn detect_platform_runtime_path(modules_dir: &Path) -> EngineResult<PathBuf> {
    type PlatformRuntimeEntryFn = unsafe extern "C" fn(
        HostApiV1,
        PlatformHostApiV1,
        PlatformAppConfigV1,
    ) -> abi_stable::std_types::RResult<(), RString>;

    #[inline]
    fn is_runtime_candidate(path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }

        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            return false;
        };

        let lower = name.to_ascii_lowercase();
        if !(lower.ends_with(".dll") || lower.ends_with(".so") || lower.ends_with(".dylib")) {
            return false;
        }

        let Ok(lib) = (unsafe { libloading::Library::new(path) }) else {
            return false;
        };

        unsafe { lib.get::<PlatformRuntimeEntryFn>(PLATFORM_RUNTIME_SYMBOL) }.is_ok()
    }

    #[inline]
    fn collect_candidates(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };

        for ent in rd.flatten() {
            let path = ent.path();
            if is_runtime_candidate(&path) {
                out.push(path);
            }
        }
    }

    if let Some(explicit) = std::env::var_os("NEWENGINE_PLATFORM_RUNTIME") {
        let explicit = PathBuf::from(explicit);
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
    push_runtime_dirs(&mut search_dirs, &exe_dir.join("plugins"));

    for dir in resolve_module_dir_candidates(modules_dir, &exe_dir) {
        push_runtime_dirs(&mut search_dirs, &dir);
    }

    push_ancestor_plugin_dirs(&mut search_dirs, &exe_dir);
    if let Ok(cwd) = std::env::current_dir() {
        push_runtime_dirs(&mut search_dirs, &cwd.join("platforms"));
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

    if let Some(path) = candidates
        .iter()
        .find(|path| {
            matches!(try_read_runtime_identity(path), Some((ref id, _)) if id == PLATFORM_PLUGIN_ID)
        })
        .cloned()
    {
        return Ok(path);
    }

    candidates.into_iter().next().ok_or_else(|| {
        EngineError::other(format!(
            "platform runtime DLL not found; searched [{}] and expected exported symbol 'newengine_platform_runtime_run_v1'. Build/copy the platform runtime into NEWENGINE_PLUGIN_DIR, NEWENGINE_PLATFORM_RUNTIME_DIR, <engine-root>/plugins, or set NEWENGINE_PLATFORM_RUNTIME to the exact DLL path.",
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
    push_unique(out, dir.join("platforms"));
    push_unique(out, dir.to_path_buf());
}

fn push_ancestor_plugin_dirs(out: &mut Vec<PathBuf>, start: &Path) {
    for ancestor in start.ancestors() {
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
