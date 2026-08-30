use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::PROJECT_MANIFEST_FILE;

/// Stable semantic roles in a NewEngine project filesystem.
///
/// Consumers resolve a role instead of embedding layout strings such as
/// `Source`, `plugins` or `asset.build.json` in product/runtime code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectPathRole {
    Root,
    Manifest,
    Source,
    Plugins,
    AssetBuildPlan,
}

pub const PROJECT_PLUGINS_DIR: &str = "plugins";
pub const PROJECT_SOURCE_DIR: &str = "Source";
pub const PROJECT_ASSET_BUILD_PLAN_FILE: &str = "asset.build.json";

/// Project-relative filesystem authority shared by launchers, runtime providers,
/// authoring systems and asset tooling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectFilesystem {
    root: PathBuf,
}

impl ProjectFilesystem {
    #[inline]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[inline]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[inline]
    pub fn resolve(&self, role: ProjectPathRole) -> PathBuf {
        match role {
            ProjectPathRole::Root => self.root.clone(),
            ProjectPathRole::Manifest => self.root.join(PROJECT_MANIFEST_FILE),
            ProjectPathRole::Source => self.root.join(PROJECT_SOURCE_DIR),
            ProjectPathRole::Plugins => self.root.join(PROJECT_PLUGINS_DIR),
            ProjectPathRole::AssetBuildPlan => self.root.join(PROJECT_ASSET_BUILD_PLAN_FILE),
        }
    }

    #[inline]
    pub fn manifest_path(&self) -> PathBuf {
        self.resolve(ProjectPathRole::Manifest)
    }

    #[inline]
    pub fn source_dir(&self) -> PathBuf {
        self.resolve(ProjectPathRole::Source)
    }

    #[inline]
    pub fn plugins_dir(&self) -> PathBuf {
        self.resolve(ProjectPathRole::Plugins)
    }

    /// Compatibility spelling retained while callers migrate to semantic roles.
    #[inline]
    pub fn conventional_plugins_dir(&self) -> PathBuf {
        self.plugins_dir()
    }

    #[inline]
    pub fn asset_build_plan_path(&self) -> PathBuf {
        self.resolve(ProjectPathRole::AssetBuildPlan)
    }

    /// Resolve an authored project-relative path. Absolute paths remain explicit
    /// escape hatches for tools/manifests that deliberately author one.
    #[inline]
    pub fn resolve_authored(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        }
    }

    /// Compatibility spelling retained for existing runtime callers.
    #[inline]
    pub fn resolve_authored_path(&self, path: &Path) -> PathBuf {
        self.resolve_authored(path)
    }
}

#[inline]
pub fn normalize_project_manifest_request(path: impl Into<PathBuf>) -> PathBuf {
    let path = path.into();
    if path.is_dir() {
        ProjectFilesystem::new(path).manifest_path()
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_roles_resolve_from_selected_project_root() {
        let fs = ProjectFilesystem::new(PathBuf::from("project-root"));
        assert_eq!(
            fs.resolve(ProjectPathRole::Root),
            PathBuf::from("project-root")
        );
        assert_eq!(
            fs.resolve(ProjectPathRole::Manifest),
            PathBuf::from("project-root").join(PROJECT_MANIFEST_FILE)
        );
        assert_eq!(
            fs.resolve(ProjectPathRole::Source),
            PathBuf::from("project-root").join(PROJECT_SOURCE_DIR)
        );
        assert_eq!(
            fs.resolve(ProjectPathRole::Plugins),
            PathBuf::from("project-root").join(PROJECT_PLUGINS_DIR)
        );
        assert_eq!(
            fs.resolve(ProjectPathRole::AssetBuildPlan),
            PathBuf::from("project-root").join(PROJECT_ASSET_BUILD_PLAN_FILE)
        );
    }

    #[test]
    fn authored_relative_paths_resolve_against_project_root() {
        let fs = ProjectFilesystem::new(PathBuf::from("project-root"));
        assert_eq!(
            fs.resolve_authored(Path::new("plugins/game.dll")),
            PathBuf::from("project-root").join("plugins/game.dll")
        );
    }
}
