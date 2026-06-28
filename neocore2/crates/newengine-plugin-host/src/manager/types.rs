use std::path::PathBuf;

use abi_stable::std_types::RResult;
use libloading::Library;
use newengine_plugin_api::{CapabilityDesc, PluginDescriptor, PluginInfo, PluginKind};

use super::adapter::ModuleAdapterAny;
use super::ui_assets::PluginIconData;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum PluginState {
    Registered,
    Running,
    Stopped,
    Disabled,
}

#[derive(Debug)]
pub struct PluginLoadError {
    pub path: PathBuf,
    pub message: String,
}

impl std::fmt::Display for PluginLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for PluginLoadError {}

#[derive(Clone, Debug)]
pub struct PluginIconSnapshot {
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct PluginSnapshotEntry {
    pub path: PathBuf,
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: Option<PluginKind>,
    pub capabilities: Vec<CapabilityDesc>,
    pub state: String,
    pub disabled_reason: Option<String>,
    pub icon_small: Option<PluginIconSnapshot>,
}

pub(crate) struct LoadedPlugin {
    pub(crate) path: PathBuf,
    pub(crate) module: ModuleAdapterAny,
    pub(crate) info: PluginInfo,
    pub(crate) descriptor: Option<PluginDescriptor>,
    pub(crate) state: PluginState,
    pub(crate) disabled_reason: Option<String>,
    pub(crate) icon_small: Option<PluginIconData>,

    /// Must be dropped last. `module` contains ABI trait objects whose vtables and
    /// destructors live in this library; unloading the DLL before dropping the
    /// module is a classic shutdown-time access violation on Windows.
    pub(crate) _lib: Library,
}

#[inline]
pub(crate) fn rresult_unit_to_string(
    r: RResult<(), abi_stable::std_types::RString>,
) -> Result<(), String> {
    r.into_result().map_err(|e| e.to_string())
}

pub(crate) fn snapshot_impl(loaded: &[LoadedPlugin]) -> Vec<PluginSnapshotEntry> {
    let mut out = Vec::with_capacity(loaded.len());
    for p in loaded.iter() {
        let (kind, caps) = match &p.descriptor {
            Some(d) => (Some(d.kind), d.capabilities.iter().cloned().collect()),
            None => (None, Vec::new()),
        };

        let state = match p.state {
            PluginState::Registered => "registered",
            PluginState::Running => "running",
            PluginState::Stopped => "stopped",
            PluginState::Disabled => "disabled",
        }
        .to_string();

        out.push(PluginSnapshotEntry {
            path: p.path.clone(),
            id: p.info.id.to_string(),
            name: p.info.name.to_string(),
            version: p.info.version.to_string(),
            kind,
            capabilities: caps,
            state,
            disabled_reason: p.disabled_reason.clone(),
            icon_small: p.icon_small.as_ref().map(|icon| PluginIconSnapshot {
                media_type: icon.media_type.clone(),
                bytes: icon.bytes.clone(),
            }),
        });
    }

    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

impl LoadedPlugin {
    /// Drops plugin ABI objects first and optionally keeps the dynamic library
    /// mapped until process exit. This avoids shutdown-time vtable/destructor
    /// races on Windows while preserving explicit unload for hot-reload paths.
    pub(crate) fn drop_with_library_policy(self, retain_library: bool) {
        let LoadedPlugin {
            path: _,
            module,
            info: _,
            descriptor: _,
            state: _,
            disabled_reason: _,
            icon_small: _,
            _lib,
        } = self;

        drop(module);
        if retain_library {
            std::mem::forget(_lib);
        }
    }
}
