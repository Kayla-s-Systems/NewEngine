#![forbid(unsafe_op_in_unsafe_fn)]

use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

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
        Self {
            key,
            primary_env,
            alias_env,
            ready_env,
            default_dir,
            leading_segment,
        }
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

/// Resolve a storage path to an absolute, lexically normalized path.
///
/// Storage roots are passed to external tools such as shader compilers. Keeping
/// `..` components in an otherwise absolute path unnecessarily consumes the
/// Windows legacy path budget and can make a valid cache location unusable.
/// This normalization is filesystem-independent and therefore also works before
/// the target directory exists.
pub fn normalize_path(path: PathBuf, default_base: Option<&Path>) -> PathBuf {
    let absolute = if path.is_absolute() {
        path
    } else if let Some(base) = default_base {
        base.join(path)
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    lexical_normalize(&absolute)
}

pub fn publish_env(spec: EngineStorageRootSpec, path: &Path) {
    let normalized = lexical_normalize(path);
    std::env::set_var(spec.primary_env, &normalized);
    std::env::set_var(spec.alias_env, &normalized);
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
    let resolved = if child.is_absolute() {
        child.to_path_buf()
    } else {
        root.join(strip_leading_segment(spec.leading_segment, child))
    };
    lexical_normalize(&resolved)
}

pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut prefix: Option<OsString> = None;
    let mut rooted = false;
    let mut parts = Vec::<OsString>::new();

    for component in path.components() {
        match component {
            Component::Prefix(value) => prefix = Some(value.as_os_str().to_owned()),
            Component::RootDir => rooted = true,
            Component::CurDir => {}
            Component::ParentDir => {
                let can_pop = parts
                    .last()
                    .is_some_and(|part| part.as_os_str() != OsStr::new(".."));
                if can_pop {
                    parts.pop();
                } else if !rooted {
                    parts.push(OsString::from(".."));
                }
            }
            Component::Normal(value) => parts.push(value.to_owned()),
        }
    }

    let mut out = PathBuf::new();
    if let Some(prefix) = prefix {
        out.push(prefix);
    }
    if rooted {
        out.push(std::path::MAIN_SEPARATOR.to_string());
    }
    for part in parts {
        out.push(part);
    }
    out
}

fn strip_leading_segment(segment: &str, path: &Path) -> PathBuf {
    let mut components = path.components();
    let Some(first) = components.next() else {
        return PathBuf::new();
    };

    let first_matches = match first {
        Component::Normal(s) => s.to_string_lossy().eq_ignore_ascii_case(segment),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_path_removes_parent_components_before_external_tool_use() {
        let base = std::env::current_dir()
            .unwrap()
            .join("apps")
            .join("AssetInspector");
        let normalized = normalize_path(PathBuf::from("../../cache/asset-inspector"), Some(&base));
        assert!(normalized.is_absolute());
        assert!(!normalized
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir)));
        assert!(normalized.ends_with(Path::new("cache/asset-inspector")));
    }

    #[test]
    fn resolve_under_root_is_also_lexically_normalized() {
        let root = normalize_path(PathBuf::from("cache/asset-inspector"), None);
        let spec = EngineStorageRootSpec::new(
            "CACHE_FILES",
            "TEST_CACHE_PRIMARY",
            "TEST_CACHE_ALIAS",
            "TEST_CACHE_READY",
            "cache",
            "cache",
        );
        let child = resolve_under_root(spec, &root, Path::new("shaders/../shaders/vulkan"));
        assert!(child.ends_with(Path::new("cache/asset-inspector/shaders/vulkan")));
        assert!(!child
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir)));
    }
}
