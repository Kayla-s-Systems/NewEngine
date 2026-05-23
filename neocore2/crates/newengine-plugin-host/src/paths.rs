#![forbid(unsafe_op_in_unsafe_fn)]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::manager::PluginLoadError;
use crate::path_fmt::canonicalize_if_exists;

const PLUGIN_DIR_ENV: &str = "NEWENGINE_PLUGIN_DIR";
const PLUGINS_DIR_ENV: &str = "NEWENGINE_PLUGINS_DIR";

pub(crate) fn resolve_plugins_dir(dir: &Path) -> Result<PathBuf, PluginLoadError> {
    if is_current_dir_path(dir) {
        return default_plugins_dir();
    }

    if dir.is_absolute() {
        return Ok(canonicalize_if_exists(dir));
    }

    let exe_dir = current_exe_dir()?;
    let mut candidates = Vec::new();
    push_unique(&mut candidates, exe_dir.join(dir));

    if let Ok(cwd) = std::env::current_dir() {
        push_unique(&mut candidates, cwd.join(dir));
        push_ancestor_relative_dirs(&mut candidates, &cwd, dir);
    }

    push_ancestor_relative_dirs(&mut candidates, &exe_dir, dir);

    if let Some(found) = best_existing_plugins_dir(candidates) {
        return Ok(found);
    }

    Ok(canonicalize_if_exists(&exe_dir.join(dir)))
}

pub(crate) fn is_dynamic_lib(p: &Path) -> bool {
    match p.extension().and_then(OsStr::to_str) {
        Some("dll") => true,
        Some("so") => true,
        Some("dylib") => true,
        _ => false,
    }
}

pub(crate) fn default_plugins_dir() -> Result<PathBuf, PluginLoadError> {
    let exe_dir = current_exe_dir()?;
    let mut candidates = Vec::new();

    for key in [PLUGIN_DIR_ENV, PLUGINS_DIR_ENV] {
        if let Some(env_dir) = env_dir(key) {
            push_unique(&mut candidates, env_dir);
        }
    }

    push_unique(&mut candidates, exe_dir.join("plugins"));
    push_ancestor_plugin_dirs(&mut candidates, &exe_dir);

    if let Ok(cwd) = std::env::current_dir() {
        push_unique(&mut candidates, cwd.join("plugins"));
        push_ancestor_plugin_dirs(&mut candidates, &cwd);
    }

    // Packaged runtime layout: dynamic libraries may live directly next to the executable.
    push_unique(&mut candidates, exe_dir.clone());

    if let Some(found) = best_existing_plugins_dir(candidates) {
        return Ok(found);
    }

    Ok(canonicalize_if_exists(&exe_dir))
}

fn is_current_dir_path(path: &Path) -> bool {
    path.as_os_str().is_empty()
        || path == Path::new(".")
        || path == Path::new("./")
        || path == Path::new(".\\")
}

fn current_exe_dir() -> Result<PathBuf, PluginLoadError> {
    let exe = std::env::current_exe().map_err(|e| PluginLoadError {
        path: PathBuf::new(),
        message: format!("current_exe failed: {e}"),
    })?;

    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| PluginLoadError {
            path: exe,
            message: "current_exe has no parent".to_string(),
        })
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

fn push_ancestor_plugin_dirs(out: &mut Vec<PathBuf>, start: &Path) {
    for ancestor in start.ancestors() {
        push_unique(out, ancestor.join("plugins"));
    }
}

fn push_ancestor_relative_dirs(out: &mut Vec<PathBuf>, start: &Path, dir: &Path) {
    for ancestor in start.ancestors() {
        push_unique(out, ancestor.join(dir));
    }
}

fn best_existing_plugins_dir(candidates: Vec<PathBuf>) -> Option<PathBuf> {
    let mut existing_without_dynlibs = None;

    for candidate in candidates {
        if !candidate.is_dir() {
            continue;
        }

        let candidate = canonicalize_if_exists(&candidate);
        if dir_has_dynamic_lib(&candidate) {
            return Some(candidate);
        }

        if existing_without_dynlibs.is_none() {
            existing_without_dynlibs = Some(candidate);
        }
    }

    existing_without_dynlibs
}

fn dir_has_dynamic_lib(dir: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };

    rd.flatten().any(|ent| is_dynamic_lib(&ent.path()))
}
