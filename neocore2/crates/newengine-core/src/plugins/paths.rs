#![forbid(unsafe_op_in_unsafe_fn)]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::path_fmt::canonicalize_if_exists;
use crate::plugins::manager::PluginLoadError;

pub(crate) fn resolve_plugins_dir(dir: &Path) -> Result<PathBuf, PluginLoadError> {
    if dir.as_os_str().is_empty() {
        return default_plugins_dir();
    }

    let is_dot = dir == Path::new(".");

    if dir.is_absolute() && !is_dot {
        return Ok(canonicalize_if_exists(dir));
    }

    let base = default_plugins_dir()?;
    if is_dot {
        return Ok(canonicalize_if_exists(&base));
    }

    Ok(canonicalize_if_exists(&base.join(dir)))
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
    let exe = std::env::current_exe().map_err(|e| PluginLoadError {
        path: PathBuf::new(),
        message: format!("current_exe failed: {e}"),
    })?;

    let dir = exe
        .parent()
        .ok_or_else(|| PluginLoadError {
            path: exe.clone(),
            message: "current_exe has no parent".to_string(),
        })?
        .to_path_buf();

    Ok(dir)
}
