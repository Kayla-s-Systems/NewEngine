#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};

pub const CACHE_FILES_ENV: &str = "NEWENGINE_CACHE_FILES";
pub const CACHE_FILES_ENV_LEGACY: &str = "CACHE_FILES";
pub const CACHE_FILES_READY_ENV: &str = "NEWENGINE_CACHE_FILES_READY";
pub const DEFAULT_CACHE_FILES_DIR: &str = "cache";

/// Resolves the engine-wide cache-files root.
///
/// Source order:
/// 1. `NEWENGINE_CACHE_FILES` env override.
/// 2. `CACHE_FILES` env override/legacy integration point.
/// 3. `default_base/cache`.
///
/// `default_base` should be the config file directory when available.
pub fn resolve_cache_files_dir(default_base: Option<&Path>) -> PathBuf {
    if let Some(path) = std::env::var_os(CACHE_FILES_ENV).filter(|v| !v.as_os_str().is_empty()) {
        return normalize_cache_path(PathBuf::from(path), default_base);
    }
    if let Some(path) = std::env::var_os(CACHE_FILES_ENV_LEGACY).filter(|v| !v.as_os_str().is_empty()) {
        return normalize_cache_path(PathBuf::from(path), default_base);
    }

    normalize_cache_path(PathBuf::from(DEFAULT_CACHE_FILES_DIR), default_base)
}

pub fn normalize_cache_path(path: PathBuf, default_base: Option<&Path>) -> PathBuf {
    if path.is_absolute() {
        return path;
    }

    if let Some(base) = default_base {
        return base.join(path);
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(path)
}

pub fn publish_cache_files_env(path: &Path) {
    std::env::set_var(CACHE_FILES_ENV, path);
    std::env::set_var(CACHE_FILES_ENV_LEGACY, path);
    std::env::set_var(CACHE_FILES_READY_ENV, "1");
}

pub fn cache_child(child: impl AsRef<Path>) -> PathBuf {
    let root = resolve_cache_files_dir(None);
    resolve_under_cache_root(&root, child.as_ref())
}

/// Resolves a path under the cache root unless it is already absolute.
///
/// Backward-compatible behavior: if callers still pass `cache/logs/foo.log`
/// while the root itself is already `.../cache`, the leading `cache/` segment is
/// stripped to avoid `cache/cache/...`.
pub fn resolve_under_cache_root(root: &Path, child: &Path) -> PathBuf {
    if child.is_absolute() {
        return child.to_path_buf();
    }

    let child = strip_leading_cache_segment(child);
    root.join(child)
}

fn strip_leading_cache_segment(path: &Path) -> PathBuf {
    let mut components = path.components();
    let Some(first) = components.next() else {
        return PathBuf::new();
    };

    let first_is_cache = match first {
        std::path::Component::Normal(s) => s.to_string_lossy().eq_ignore_ascii_case("cache"),
        _ => false,
    };

    if !first_is_cache {
        return path.to_path_buf();
    }

    let mut out = PathBuf::new();
    for c in components {
        out.push(c.as_os_str());
    }
    out
}

pub fn display_cache_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
