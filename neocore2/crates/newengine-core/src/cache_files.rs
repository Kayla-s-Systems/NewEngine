#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};

use crate::storage_root::{self, EngineStorageRootSpec};

pub const CACHE_FILES_ENV: &str = "NEWENGINE_CACHE_FILES";
pub const CACHE_FILES_ALIAS_ENV: &str = "CACHE_FILES";
pub const CACHE_FILES_READY_ENV: &str = "NEWENGINE_CACHE_FILES_READY";
pub const DEFAULT_CACHE_FILES_DIR: &str = "cache";

pub const CACHE_FILES_ROOT_SPEC: EngineStorageRootSpec = EngineStorageRootSpec::new(
    "cache_files",
    CACHE_FILES_ENV,
    CACHE_FILES_ALIAS_ENV,
    CACHE_FILES_READY_ENV,
    DEFAULT_CACHE_FILES_DIR,
    DEFAULT_CACHE_FILES_DIR,
);

/// Resolves the engine-wide cache-files root.
///
/// `CACHE_FILES` is disposable generated data. Shader caches, derived build
/// data and diagnostics that can be regenerated should use this root.
pub fn resolve_cache_files_dir(default_base: Option<&Path>) -> PathBuf {
    storage_root::resolve_dir(CACHE_FILES_ROOT_SPEC, default_base)
}

#[inline]
pub fn normalize_cache_path(path: PathBuf, default_base: Option<&Path>) -> PathBuf {
    storage_root::normalize_path(path, default_base)
}

#[inline]
pub fn publish_cache_files_env(path: &Path) {
    storage_root::publish_env(CACHE_FILES_ROOT_SPEC, path);
}

#[inline]
pub fn cache_child(child: impl AsRef<Path>) -> PathBuf {
    storage_root::child(CACHE_FILES_ROOT_SPEC, child)
}

#[inline]
pub fn resolve_under_cache_root(root: &Path, child: &Path) -> PathBuf {
    storage_root::resolve_under_root(CACHE_FILES_ROOT_SPEC, root, child)
}

#[inline]
pub fn display_cache_path(path: &Path) -> String {
    storage_root::display_path(path)
}
