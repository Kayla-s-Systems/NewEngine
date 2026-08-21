use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ContentMountNamespace {
    Engine,
    Project,
    #[default]
    Game,
    Plugin,
    User,
}

impl ContentMountNamespace {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Engine => "engine",
            Self::Project => "project",
            Self::Game => "game",
            Self::Plugin => "plugin",
            Self::User => "user",
        }
    }

    pub const fn default_priority(self) -> i32 {
        match self {
            Self::Engine => 100,
            Self::Plugin => 250,
            Self::Game => 300,
            Self::Project => 400,
            Self::User => 500,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ContentMountDescriptor {
    pub id: String,
    pub namespace: ContentMountNamespace,
    pub root: PathBuf,
    pub mount: String,
    pub priority: i32,
    pub writable: bool,
    pub required: bool,
    pub owner: String,
}

impl Default for ContentMountDescriptor {
    fn default() -> Self {
        let namespace = ContentMountNamespace::Game;
        Self {
            id: String::new(),
            namespace,
            root: PathBuf::new(),
            mount: namespace.id().to_owned(),
            priority: namespace.default_priority(),
            writable: false,
            required: false,
            owner: String::new(),
        }
    }
}

impl ContentMountDescriptor {
    pub fn normalized(mut self, base: &Path) -> Self {
        if self.mount.trim().is_empty() {
            self.mount = self.namespace.id().to_owned();
        }
        if self.priority == 0 {
            self.priority = self.namespace.default_priority();
        }
        if self.root.is_relative() {
            self.root = base.join(&self.root);
        }
        self
    }

    pub fn logical_prefix(&self) -> String {
        format!("{}:/", self.namespace.id())
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProjectContentMountState {
    pub required: bool,
    pub mounted: bool,
    pub attempts: u64,
    pub last_error: Option<String>,
}

impl ProjectContentMountState {
    pub fn pending() -> Self {
        Self {
            required: true,
            mounted: false,
            attempts: 0,
            last_error: None,
        }
    }

    pub const fn ready(&self) -> bool {
        !self.required || self.mounted
    }
}

#[derive(Clone, Debug, Default)]
pub struct ContentMountRegistry {
    mounts: Vec<ContentMountDescriptor>,
}

impl ContentMountRegistry {
    pub fn register(&mut self, descriptor: ContentMountDescriptor) -> Result<(), String> {
        let id = descriptor.id.trim();
        if id.is_empty() {
            return Err("content mount id must not be empty".to_owned());
        }
        if self.mounts.iter().any(|mount| mount.id == id) {
            return Err(format!("content mount already registered: {id}"));
        }
        self.mounts.push(descriptor);
        self.mounts
            .sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        Ok(())
    }

    pub fn mounts(&self) -> &[ContentMountDescriptor] {
        &self.mounts
    }

    pub fn namespace_mounts(
        &self,
        namespace: ContentMountNamespace,
    ) -> impl Iterator<Item = &ContentMountDescriptor> {
        self.mounts
            .iter()
            .filter(move |mount| mount.namespace == namespace)
    }

    pub fn resolve_logical(&self, logical: &str) -> Option<PathBuf> {
        let (prefix, tail) = logical.split_once(":/")?;
        self.mounts.iter().find_map(|mount| {
            (mount.namespace.id() == prefix).then(|| {
                mount
                    .root
                    .join(tail.replace('/', std::path::MAIN_SEPARATOR_STR))
            })
        })
    }

    /// Resolve a physical file under the highest-priority registered mount back to a stable logical ref.
    /// Shadowed lower-priority files intentionally return `None`: changing them must not invalidate the
    /// currently winning VFS asset until mount priority changes.
    pub fn logical_for_physical(&self, physical: &Path) -> Option<String> {
        for mount in &self.mounts {
            let Ok(relative) = physical.strip_prefix(&mount.root) else {
                continue;
            };
            if relative.as_os_str().is_empty() {
                continue;
            }
            let tail = relative.to_string_lossy().replace('\\', "/");
            let logical = format!("{}:/{}", mount.namespace.id(), tail.trim_start_matches('/'));
            let Some(winner) = self.resolve_logical(&logical) else {
                continue;
            };
            if winner == physical {
                return Some(logical);
            }
        }
        None
    }

    /// Map a winning physical file to the actual AssetManager/VFS logical path.
    /// Unlike `logical_for_physical`, this uses the authored `mount` prefix (`game/foo`)
    /// instead of the editor-facing namespace syntax (`game:/foo`).
    pub fn asset_ref_for_physical(&self, physical: &Path) -> Option<String> {
        for mount in &self.mounts {
            let Ok(relative) = physical.strip_prefix(&mount.root) else {
                continue;
            };
            if relative.as_os_str().is_empty() {
                continue;
            }
            let tail = relative.to_string_lossy().replace('\\', "/");
            let mount_prefix = mount.mount.trim().trim_matches('/');
            let asset_ref = if mount_prefix.is_empty() {
                tail.trim_start_matches('/').to_owned()
            } else {
                format!("{mount_prefix}/{}", tail.trim_start_matches('/'))
            };
            if self.resolve_asset_ref(&asset_ref).as_deref() == Some(physical) {
                return Some(asset_ref);
            }
        }
        None
    }

    /// Resolve a provider/VFS asset ref through the same ordered mount semantics used
    /// when the project registry is mounted into `engine.assets`.
    pub fn resolve_asset_ref(&self, asset_ref: &str) -> Option<PathBuf> {
        let normalized = asset_ref.replace('\\', "/");
        for mount in &self.mounts {
            let prefix = mount.mount.trim().trim_matches('/');
            let relative = if prefix.is_empty() {
                normalized.as_str()
            } else {
                let wanted = format!("{prefix}/");
                let Some(relative) = normalized.strip_prefix(&wanted) else {
                    continue;
                };
                relative
            };
            if relative.is_empty() {
                continue;
            }
            return Some(
                mount
                    .root
                    .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR)),
            );
        }
        None
    }
}
