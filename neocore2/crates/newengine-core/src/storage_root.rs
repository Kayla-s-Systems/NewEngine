#![forbid(unsafe_op_in_unsafe_fn)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Declarative specification for an engine-runtime filesystem root.
///
/// Roots are intentionally small data records: callers describe semantics once
/// and reuse the same resolver/publisher/child-path behavior. This keeps
/// durable user data roots (`CONFIG`) and disposable generated roots
/// (`CACHE_FILES`) on the same architectural level without duplicating code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineStorageRootSpec {
    pub key: &'static str,
    pub primary_env: &'static str,
    pub alias_env: &'static str,
    pub ready_env: &'static str,
    pub default_dir: &'static str,
    pub leading_segment: &'static str,
}

impl EngineStorageRootSpec {
    #[inline]
    pub const fn new(
        key: &'static str,
        primary_env: &'static str,
        alias_env: &'static str,
        ready_env: &'static str,
        default_dir: &'static str,
        leading_segment: &'static str,
    ) -> Self {
        Self { key, primary_env, alias_env, ready_env, default_dir, leading_segment }
    }
}

pub fn resolve_dir(spec: EngineStorageRootSpec, default_base: Option<&Path>) -> PathBuf {
    if let Some(path) = non_empty_env(spec.primary_env) {
        return normalize_path(PathBuf::from(path), default_base);
    }
    if let Some(path) = non_empty_env(spec.alias_env) {
        return normalize_path(PathBuf::from(path), default_base);
    }

    normalize_path(PathBuf::from(spec.default_dir), default_base)
}

pub fn normalize_path(path: PathBuf, default_base: Option<&Path>) -> PathBuf {
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

pub fn publish_env(spec: EngineStorageRootSpec, path: &Path) {
    std::env::set_var(spec.primary_env, path);
    std::env::set_var(spec.alias_env, path);
    std::env::set_var(spec.ready_env, "1");
}

pub fn child(spec: EngineStorageRootSpec, child: impl AsRef<Path>) -> PathBuf {
    let root = resolve_dir(spec, None);
    resolve_under_root(spec, &root, child.as_ref())
}

/// Resolves a path under a storage root unless it is already absolute.
///
/// If a caller already includes the root's own leading segment (for example
/// `cache/logs/x.log` under the cache root or `config/input/x.json` under the
/// config root), that segment is stripped to avoid `cache/cache` and
/// `config/config` paths. This is a canonicalization rule, not an alternate routing path.
pub fn resolve_under_root(spec: EngineStorageRootSpec, root: &Path, child: &Path) -> PathBuf {
    if child.is_absolute() {
        return child.to_path_buf();
    }

    root.join(strip_leading_segment(spec.leading_segment, child))
}

pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn strip_leading_segment(segment: &str, path: &Path) -> PathBuf {
    let mut components = path.components();
    let Some(first) = components.next() else {
        return PathBuf::new();
    };

    let first_matches = match first {
        std::path::Component::Normal(s) => s.to_string_lossy().eq_ignore_ascii_case(segment),
        _ => false,
    };

    if !first_matches {
        return path.to_path_buf();
    }

    let mut out = PathBuf::new();
    for c in components {
        out.push(c.as_os_str());
    }
    out
}

fn non_empty_env(name: &str) -> Option<OsString> {
    std::env::var_os(name).filter(|value| !value.as_os_str().is_empty())
}
