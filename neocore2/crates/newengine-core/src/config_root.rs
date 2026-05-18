#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};

/// Engine-level user configuration root.
///
/// Unlike CACHE_FILES, CONFIG stores durable user settings and must not be
/// treated as disposable generated data. Bindings, user preferences, graphics
/// presets and other player/editor settings should resolve under this root.
pub const CONFIG_ENV: &str = "NEWENGINE_CONFIG";
pub const CONFIG_ENV_LEGACY: &str = "CONFIG";
pub const CONFIG_READY_ENV: &str = "NEWENGINE_CONFIG_READY";
pub const DEFAULT_CONFIG_DIR: &str = "config";

/// Resolves the engine-wide durable configuration root.
///
/// Source order:
/// 1. `NEWENGINE_CONFIG` env override.
/// 2. `CONFIG` env override/user integration point.
/// 3. `default_base/config`.
///
/// `default_base` should be the startup config file directory when available.
pub fn resolve_config_dir(default_base: Option<&Path>) -> PathBuf {
    if let Some(path) = std::env::var_os(CONFIG_ENV).filter(|v| !v.as_os_str().is_empty()) {
        return normalize_config_path(PathBuf::from(path), default_base);
    }
    if let Some(path) = std::env::var_os(CONFIG_ENV_LEGACY).filter(|v| !v.as_os_str().is_empty()) {
        return normalize_config_path(PathBuf::from(path), default_base);
    }

    normalize_config_path(PathBuf::from(DEFAULT_CONFIG_DIR), default_base)
}

pub fn normalize_config_path(path: PathBuf, default_base: Option<&Path>) -> PathBuf {
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

pub fn publish_config_env(path: &Path) {
    std::env::set_var(CONFIG_ENV, path);
    std::env::set_var(CONFIG_ENV_LEGACY, path);
    std::env::set_var(CONFIG_READY_ENV, "1");
}

pub fn config_child(child: impl AsRef<Path>) -> PathBuf {
    let root = resolve_config_dir(None);
    resolve_under_config_root(&root, child.as_ref())
}

/// Resolves a path under the durable CONFIG root unless it is already absolute.
///
/// If callers pass `config/foo.json` while the root itself is already
/// `.../config`, the leading `config/` segment is stripped to avoid
/// `config/config/...`.
pub fn resolve_under_config_root(root: &Path, child: &Path) -> PathBuf {
    if child.is_absolute() {
        return child.to_path_buf();
    }

    let child = strip_leading_config_segment(child);
    root.join(child)
}

fn strip_leading_config_segment(path: &Path) -> PathBuf {
    let mut components = path.components();
    let Some(first) = components.next() else {
        return PathBuf::new();
    };

    let first_is_config = match first {
        std::path::Component::Normal(s) => s.to_string_lossy().eq_ignore_ascii_case("config"),
        _ => false,
    };

    if !first_is_config {
        return path.to_path_buf();
    }

    let mut out = PathBuf::new();
    for c in components {
        out.push(c.as_os_str());
    }
    out
}

pub fn display_config_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
