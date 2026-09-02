use newengine_math::collections_prelude::NeHashSet as HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContentSetSpec {
    pub id: &'static str,
    pub app_dir_name: Option<&'static str>,
    pub env_roots: &'static [&'static str],
    pub priority: i32,
    pub mount: &'static str,
    pub include_shared_assets: bool,
    pub include_app_assets: bool,
}

impl ContentSetSpec {
    #[inline]
    pub const fn runtime_app(
        id: &'static str,
        app_dir_name: &'static str,
        env_roots: &'static [&'static str],
    ) -> Self {
        Self {
            id,
            app_dir_name: Some(app_dir_name),
            env_roots,
            priority: 200,
            mount: "",
            include_shared_assets: true,
            include_app_assets: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfileMountSpec {
    pub profile_id: &'static str,
    pub content_sets: &'static [ContentSetSpec],
}

impl ProfileMountSpec {
    #[inline]
    pub const fn new(profile_id: &'static str, content_sets: &'static [ContentSetSpec]) -> Self {
        Self {
            profile_id,
            content_sets,
        }
    }
}

/// Resolves declarative content sets into OS candidates. Profiles describe content only;
/// runtime-host owns CWD/executable discovery and identity deduplication.
pub fn collect_profile_mount_roots(spec: ProfileMountSpec) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut dedup: HashSet<PathBuf> = HashSet::default();
    for content in spec.content_sets {
        for root in collect_content_set_roots(*content) {
            push_unique_root(&mut out, &mut dedup, root);
        }
    }
    out
}

pub(crate) fn collect_content_set_roots(content: ContentSetSpec) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut dedup: HashSet<PathBuf> = HashSet::default();

    for env_var in content.env_roots {
        if let Some(path) = newengine_plugin_host::current_host_context().environment_var(env_var) {
            let path = path.trim();
            if !path.is_empty() {
                push_unique_root(&mut roots, &mut dedup, PathBuf::from(path));
            }
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        walk_ancestors(cwd, |base| {
            push_content_roots_from_base(&mut roots, &mut dedup, base, content)
        });
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            walk_ancestors(dir.to_path_buf(), |base| {
                push_content_roots_from_base(&mut roots, &mut dedup, base, content)
            });
        }
    }

    roots
}

/// Discovers canonical engine/app roots for standalone tools that do not use a profile.
///
/// The ancestor walk already visits `NewEngine` and `neocore2` directly, so explicit
/// `base/NewEngine/...` compatibility probes are intentionally not part of this path.
pub fn collect_app_asset_roots(app_dir_name: &str, env_var: &str) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut dedup: HashSet<PathBuf> = HashSet::default();

    if let Some(path) = newengine_plugin_host::current_host_context().environment_var(env_var) {
        let path = path.trim();
        if !path.is_empty() {
            push_unique_root(&mut roots, &mut dedup, PathBuf::from(path));
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        walk_ancestors(cwd, |base| {
            push_app_roots_from_base(&mut roots, &mut dedup, base, app_dir_name)
        });
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            walk_ancestors(dir.to_path_buf(), |base| {
                push_app_roots_from_base(&mut roots, &mut dedup, base, app_dir_name)
            });
        }
    }

    roots
}

fn walk_ancestors(mut current: PathBuf, mut visit: impl FnMut(&Path)) {
    for _ in 0..8 {
        visit(&current);
        let Some(parent) = current.parent() else {
            break;
        };
        current = parent.to_path_buf();
    }
}

fn push_content_roots_from_base(
    roots: &mut Vec<PathBuf>,
    dedup: &mut HashSet<PathBuf>,
    base: &Path,
    content: ContentSetSpec,
) {
    if content.include_shared_assets {
        push_if_dir(roots, dedup, base.join("Shared").join("Content"));
        push_if_dir(roots, dedup, base.join("assets"));
    }

    if content.include_app_assets {
        if let Some(app_dir_name) = content.app_dir_name {
            push_if_dir(
                roots,
                dedup,
                base.join("apps").join(app_dir_name).join("assets"),
            );
        }
    }
}

fn push_app_roots_from_base(
    roots: &mut Vec<PathBuf>,
    dedup: &mut HashSet<PathBuf>,
    base: &Path,
    app_dir_name: &str,
) {
    push_if_dir(roots, dedup, base.join("Shared").join("Content"));
    push_if_dir(roots, dedup, base.join("assets"));

    if !app_dir_name.trim().is_empty() {
        push_if_dir(
            roots,
            dedup,
            base.join("apps").join(app_dir_name).join("assets"),
        );
    }
}

fn push_if_dir(roots: &mut Vec<PathBuf>, dedup: &mut HashSet<PathBuf>, root: PathBuf) {
    if root.is_dir() {
        push_unique_root(roots, dedup, root);
    }
}

fn push_unique_root(roots: &mut Vec<PathBuf>, dedup: &mut HashSet<PathBuf>, root: PathBuf) {
    let identity = std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
    if dedup.insert(identity) {
        roots.push(root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_app_has_no_legacy_layout_switch() {
        let spec = ContentSetSpec::runtime_app("test", "NewEngine", &[]);
        assert!(spec.include_shared_assets);
        assert!(spec.include_app_assets);
        assert_eq!(spec.priority, 200);
        assert_eq!(spec.mount, "");
    }

    #[test]
    fn ancestor_walk_visits_start_once() {
        let mut visited = Vec::new();
        walk_ancestors(PathBuf::from("a/b/c"), |path| {
            visited.push(path.to_path_buf())
        });
        assert_eq!(visited.first(), Some(&PathBuf::from("a/b/c")));
        assert_eq!(
            visited
                .iter()
                .filter(|path| *path == &PathBuf::from("a/b/c"))
                .count(),
            1
        );
    }
}
