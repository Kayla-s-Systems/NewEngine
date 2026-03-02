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

use types::LoadedPlugin;

pub struct PluginManager {
    loaded: Vec<LoadedPlugin>,
    loaded_ids: NeHashSet<String>,
}

impl PluginManager {
    #[inline]
    pub fn new() -> Self {
        Self {
            loaded: Vec::new(),
            loaded_ids: NeHashSet::default(),
        }
    }

    #[inline]
    pub fn has_plugin(&self, id: &str) -> bool {
        self.loaded_ids.contains(id)
    }

    #[inline]
    pub fn find_index(&self, id: &str) -> Option<usize> {
        self.loaded.iter().position(|p| p.info.id.to_string() == id)
    }

    #[inline]
    pub fn snapshot(&self) -> Vec<PluginSnapshotEntry> {
        types::snapshot_impl(&self.loaded)
    }
}
