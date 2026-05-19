#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};

use crate::storage_root::{self, EngineStorageRootSpec};

/// Engine-level durable user configuration root.
///
/// Unlike CACHE_FILES, CONFIG stores user settings and must not be treated as
/// disposable generated data. Bindings, device preferences, graphics presets
/// and editor/game settings should resolve under this root.
pub const CONFIG_ENV: &str = "NEWENGINE_CONFIG";
pub const CONFIG_ALIAS_ENV: &str = "CONFIG";
pub const CONFIG_READY_ENV: &str = "NEWENGINE_CONFIG_READY";
pub const DEFAULT_CONFIG_DIR: &str = "config";

pub const CONFIG_ROOT_SPEC: EngineStorageRootSpec = EngineStorageRootSpec::new(
    "config",
    CONFIG_ENV,
    CONFIG_ALIAS_ENV,
    CONFIG_READY_ENV,
    DEFAULT_CONFIG_DIR,
    DEFAULT_CONFIG_DIR,
);

pub fn resolve_config_dir(default_base: Option<&Path>) -> PathBuf {
    storage_root::resolve_dir(CONFIG_ROOT_SPEC, default_base)
}

#[inline]
pub fn normalize_config_path(path: PathBuf, default_base: Option<&Path>) -> PathBuf {
    storage_root::normalize_path(path, default_base)
}

#[inline]
pub fn publish_config_env(path: &Path) {
    storage_root::publish_env(CONFIG_ROOT_SPEC, path);
}

#[inline]
pub fn config_child(child: impl AsRef<Path>) -> PathBuf {
    storage_root::child(CONFIG_ROOT_SPEC, child)
}

#[inline]
pub fn resolve_under_config_root(root: &Path, child: &Path) -> PathBuf {
    storage_root::resolve_under_root(CONFIG_ROOT_SPEC, root, child)
}

#[inline]
pub fn display_config_path(path: &Path) -> String {
    storage_root::display_path(path)
}
