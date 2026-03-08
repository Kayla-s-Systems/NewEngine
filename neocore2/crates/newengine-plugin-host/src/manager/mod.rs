#![forbid(unsafe_op_in_unsafe_fn)]

mod adapter;
mod cap_validate;
mod config_patch;
mod discovery;
mod lifecycle;
mod loader;
mod types;

pub use types::{PluginLoadError, PluginSnapshotEntry};

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
        self.loaded
            .iter()
            .position(|p| p.info.id.as_str() == id)
    }

    #[inline]
    pub fn snapshot(&self) -> Vec<PluginSnapshotEntry> {
        types::snapshot_impl(&self.loaded)
    }
}

impl Default for PluginManager {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}