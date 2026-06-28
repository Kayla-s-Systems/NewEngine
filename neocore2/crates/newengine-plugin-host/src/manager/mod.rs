#![forbid(unsafe_op_in_unsafe_fn)]

mod adapter;
mod cap_validate;
mod config_patch;
mod discovery;
mod lifecycle;
mod loader;
mod types;
mod ui_assets;

pub use types::{PluginIconSnapshot, PluginLoadError, PluginSnapshotEntry};

use newengine_math::collections::prelude::*;

use self::types::LoadedPlugin;

pub struct PluginManager {
    loaded: Vec<LoadedPlugin>,
    loaded_ids: NeHashSet<String>,
    discovery_cache: Option<discovery::DiscoveryGraph>,
}

impl PluginManager {
    #[inline]
    pub fn new() -> Self {
        Self {
            loaded: Vec::new(),
            loaded_ids: NeHashSet::default(),
            discovery_cache: None,
        }
    }

    #[inline]
    pub fn has_plugin(&self, id: &str) -> bool {
        self.loaded_ids.contains(id)
    }

    #[inline]
    pub fn find_index(&self, id: &str) -> Option<usize> {
        self.loaded.iter().position(|p| p.info.id.as_str() == id)
    }

    #[inline]
    pub fn snapshot(&self) -> Vec<PluginSnapshotEntry> {
        let mut out = types::snapshot_impl(&self.loaded);
        out.extend(
            crate::host_context::list_external_runtime_plugins()
                .into_iter()
                .map(|p| PluginSnapshotEntry {
                    path: p.path,
                    id: p.id,
                    name: p.name,
                    version: p.version,
                    kind: p.kind,
                    capabilities: p.capabilities,
                    state: p.state,
                    disabled_reason: p.disabled_reason,
                    icon_small: None,
                }),
        );
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }
}

impl Default for PluginManager {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
