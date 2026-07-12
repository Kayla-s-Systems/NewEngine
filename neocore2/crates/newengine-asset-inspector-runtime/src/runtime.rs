use newengine_assets::AssetServiceClient;
use newengine_assets_api::{
    AssetDecodeRequest, AssetFileManifest, ASSET_LIST_FILE_MANIFEST_OUTPUT,
};
use newengine_core::{EngineReadinessKey, EngineResult, Module, ModuleCtx};
use newengine_ui_api::{UiEventDispatchFrame, UiNodeEventTrigger};
use serde_json::{json, Value};

use crate::inspection::NativeAssetInspector;
use crate::model::{AssetInspectorMode, AssetInspectorReport, InspectorEntry};
use crate::mounts::mount_asset_roots;
use crate::source_pair::{is_source_asset_ref, source_runtime_counterpart};
use crate::surface::mount_asset_inspector_surface;
use crate::ui_state::{publish_inspector_state, InspectorUiSnapshot, ENTRY_ROWS};
use crate::ASSET_INSPECTOR_SURFACE_ID;

const ACTION_REFRESH: &str = "asset.inspector.refresh";
const ACTION_UP: &str = "asset.inspector.up";
const ACTION_PAGE_PREVIOUS: &str = "asset.inspector.page.previous";
const ACTION_PAGE_NEXT: &str = "asset.inspector.page.next";
const ACTION_MODE_ALL: &str = "asset.inspector.mode.all";
const ACTION_MODE_RUNTIME: &str = "asset.inspector.mode.runtime";
const ACTION_MODE_SOURCE: &str = "asset.inspector.mode.source";
const ACTION_ENTRY: &str = "asset.inspector.entry";
const ACTION_COUNTERPART: &str = "asset.inspector.counterpart";

pub struct AssetInspectorRuntimeModule {
    assets: AssetServiceClient,
    inspector: NativeAssetInspector,
    current_path: String,
    mode: AssetInspectorMode,
    page: usize,
    entries: Vec<InspectorEntry>,
    selected_index: Option<usize>,
    report: Option<AssetInspectorReport>,
    status: String,
    last_refresh_frame: u64,
    last_action_frame: u64,
    dirty: bool,
    roots_mounted: bool,
    last_root_mount_attempt_frame: u64,
    surface_mounted: bool,
    last_surface_mount_attempt_frame: u64,
}

impl Default for AssetInspectorRuntimeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetInspectorRuntimeModule {
    pub fn new() -> Self {
        Self {
            assets: AssetServiceClient::new(newengine_plugin_host::default_host_api()),
            inspector: NativeAssetInspector::new(),
            current_path: String::new(),
            mode: AssetInspectorMode::All,
            page: 0,
            entries: Vec::new(),
            selected_index: None,
            report: None,
            status: "Waiting for engine.assets".to_owned(),
            last_refresh_frame: 0,
            last_action_frame: u64::MAX,
            dirty: true,
            roots_mounted: false,
            last_root_mount_attempt_frame: u64::MAX,
            surface_mounted: false,
            last_surface_mount_attempt_frame: u64::MAX,
        }
    }

    fn ensure_roots_mounted(&mut self, frame_index: u64) {
        if self.roots_mounted {
            return;
        }
        let should_attempt = self.last_root_mount_attempt_frame == u64::MAX
            || frame_index.saturating_sub(self.last_root_mount_attempt_frame) >= 30;
        if !should_attempt {
            return;
        }
        self.last_root_mount_attempt_frame = frame_index;
        match mount_asset_roots(&self.assets) {
            Ok(roots) => {
                self.roots_mounted = true;
                self.last_refresh_frame = 0;
                self.status = format!("Mounted {} standalone gameAssets root(s)", roots.len());
                self.dirty = true;
                for root in roots {
                    newengine_ulog_api::ulog::info!(
                        "asset inspector: standalone VFS root mounted path='{}'",
                        root.display()
                    );
                }
            }
            Err(error) => {
                self.status = format!("Waiting for engine.assets VFS: {error}");
                if frame_index == 0 || frame_index.is_multiple_of(120) {
                    newengine_ulog_api::ulog::warn!(
                        "asset inspector: standalone VFS mount deferred frame={} err='{}'",
                        frame_index,
                        error
                    );
                }
            }
        }
    }

    fn ensure_surface_mounted(&mut self, frame_index: u64) {
        if self.surface_mounted || !self.roots_mounted {
            return;
        }
        let should_attempt = self.last_surface_mount_attempt_frame == u64::MAX
            || frame_index.saturating_sub(self.last_surface_mount_attempt_frame) >= 30;
        if !should_attempt {
            return;
        }
        self.last_surface_mount_attempt_frame = frame_index;
        match mount_asset_inspector_surface() {
            Ok(_) => {
                self.surface_mounted = true;
                self.status = "Standalone authored UI mounted".to_owned();
                self.dirty = true;
            }
            Err(error) => {
                self.status = format!("Waiting for standalone UI services: {error}");
                if frame_index == 0 || frame_index.is_multiple_of(120) {
                    newengine_ulog_api::ulog::warn!(
                        "asset inspector: standalone surface mount deferred frame={} err='{}'",
                        frame_index,
                        error
                    );
                }
            }
        }
    }

    fn refresh(&mut self, frame_index: u64) {
        match list_path(&self.assets, &self.current_path) {
            Ok(mut entries) => {
                entries.retain(|entry| self.mode.accepts(entry.is_directory, entry.source_asset));
                entries.sort_by(|a, b| {
                    b.is_directory.cmp(&a.is_directory).then_with(|| {
                        a.name
                            .to_ascii_lowercase()
                            .cmp(&b.name.to_ascii_lowercase())
                    })
                });
                self.entries = entries;
                let total_pages = self.entries.len().max(1).div_ceil(ENTRY_ROWS);
                self.page = self.page.min(total_pages.saturating_sub(1));
                if self
                    .selected_index
                    .is_some_and(|index| index >= self.entries.len())
                {
                    self.selected_index = None;
                    self.report = None;
                }
                self.status = format!(
                    "{} entries · {} mode · native codecs through engine.assets",
                    self.entries.len(),
                    self.mode.label()
                );
                newengine_ulog_api::ulog::info!(
                    "asset inspector: VFS snapshot path='{}' mode={} entries={} source_entries={} runtime_entries={}",
                    if self.current_path.is_empty() { "<root>" } else { self.current_path.as_str() },
                    self.mode.label(),
                    self.entries.len(),
                    self.entries.iter().filter(|entry| entry.source_asset).count(),
                    self.entries.iter().filter(|entry| !entry.source_asset && !entry.is_directory).count(),
                );
            }
            Err(error) => {
                self.entries.clear();
                self.status = error;
            }
        }
        self.last_refresh_frame = frame_index;
        self.dirty = true;
    }

    fn handle_actions(&mut self, frame: &UiEventDispatchFrame) {
        if self.last_action_frame == frame.frame_index {
            return;
        }
        let mut consumed = false;
        for action in frame
            .actions
            .iter()
            .filter(|action| action.surface_id == ASSET_INSPECTOR_SURFACE_ID)
            .filter(|action| {
                matches!(
                    action.trigger,
                    UiNodeEventTrigger::Click | UiNodeEventTrigger::DoubleClick
                )
            })
        {
            consumed = true;
            match action.action_id.as_str() {
                ACTION_REFRESH => self.refresh(frame.frame_index),
                ACTION_UP => self.navigate_up(),
                ACTION_PAGE_PREVIOUS => self.page = self.page.saturating_sub(1),
                ACTION_PAGE_NEXT => {
                    let max_page = self.entries.len().max(1).div_ceil(ENTRY_ROWS) - 1;
                    self.page = (self.page + 1).min(max_page);
                }
                ACTION_MODE_ALL => self.set_mode(AssetInspectorMode::All),
                ACTION_MODE_RUNTIME => self.set_mode(AssetInspectorMode::Runtime),
                ACTION_MODE_SOURCE => self.set_mode(AssetInspectorMode::Source),
                ACTION_ENTRY => {
                    if let Some(row) = parse_row(&action.node_id) {
                        self.activate_row(row);
                    }
                }
                ACTION_COUNTERPART => self.open_counterpart(),
                _ => consumed = false,
            }
        }
        if consumed {
            self.last_action_frame = frame.frame_index;
            self.dirty = true;
        }
    }

    fn set_mode(&mut self, mode: AssetInspectorMode) {
        if self.mode != mode {
            self.mode = mode;
            self.page = 0;
            self.selected_index = None;
            self.report = None;
            self.last_refresh_frame = 0;
        }
    }

    fn navigate_up(&mut self) {
        if let Some((parent, _)) = self.current_path.rsplit_once('/') {
            self.current_path = parent.to_owned();
        } else {
            self.current_path.clear();
        }
        self.page = 0;
        self.selected_index = None;
        self.report = None;
        self.last_refresh_frame = 0;
    }

    fn activate_row(&mut self, row: usize) {
        let absolute = self.page * ENTRY_ROWS + row;
        let Some(entry) = self.entries.get(absolute).cloned() else {
            return;
        };
        if entry.is_directory {
            self.current_path = entry.logical_path;
            self.page = 0;
            self.selected_index = None;
            self.report = None;
            self.last_refresh_frame = 0;
        } else {
            self.selected_index = Some(absolute);
            self.report = Some(self.inspector.inspect(&entry.logical_path));
            self.status = format!(
                "Inspected {} through {}",
                entry.name,
                self.report
                    .as_ref()
                    .map_or("native provider", |it| it.decoder.as_str())
            );
        }
    }

    fn open_counterpart(&mut self) {
        let counterpart = self
            .report
            .as_ref()
            .and_then(|report| report.counterpart.clone())
            .or_else(|| {
                self.selected_index
                    .and_then(|index| self.entries.get(index))
                    .and_then(|entry| source_runtime_counterpart(&entry.logical_path))
            });
        let Some(counterpart) = counterpart else {
            self.status = "No source/runtime counterpart resolved".to_owned();
            return;
        };
        let looks_like_file = counterpart
            .rsplit('/')
            .next()
            .is_some_and(|name| name.contains('.'));
        if looks_like_file {
            self.report = Some(self.inspector.inspect(&counterpart));
            self.status = format!("Opened counterpart {counterpart}");
        } else {
            self.current_path = counterpart;
            self.page = 0;
            self.selected_index = None;
            self.report = None;
            self.last_refresh_frame = 0;
        }
    }
}

impl<E: Send + 'static> Module<E> for AssetInspectorRuntimeModule {
    fn id(&self) -> &'static str {
        "app.asset_inspector.runtime"
    }

    fn startup_requires(&self) -> &'static [EngineReadinessKey] {
        const REQUIRES: &[EngineReadinessKey] = &[EngineReadinessKey::EnginePluginsReady];
        REQUIRES
    }

    fn start(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        self.dirty = true;
        Ok(())
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let frame_index = ctx.frame().map(|frame| frame.frame_index).unwrap_or(0);
        self.ensure_roots_mounted(frame_index);
        self.ensure_surface_mounted(frame_index);
        if let Some(dispatch) = ctx.resources().get::<UiEventDispatchFrame>().cloned() {
            self.handle_actions(&dispatch);
        }
        if self.roots_mounted
            && (self.last_refresh_frame == 0
                || frame_index.saturating_sub(self.last_refresh_frame) >= 120)
        {
            self.refresh(frame_index);
        }
        if self.dirty && self.surface_mounted {
            let published = publish_inspector_state(InspectorUiSnapshot {
                frame_index,
                current_path: &self.current_path,
                mode: self.mode,
                page: self.page,
                entries: &self.entries,
                selected_index: self.selected_index,
                report: self.report.as_ref(),
                status: &self.status,
            });
            self.dirty = !published;
        }
        Ok(())
    }
}

fn list_path(
    client: &AssetServiceClient,
    logical_path: &str,
) -> Result<Vec<InspectorEntry>, String> {
    match client.vfs_list_json_v1(logical_path) {
        Ok(value) => Ok(entries_from_vfs(&value)),
        Err(vfs_error) if !logical_path.is_empty() && !logical_path.contains('@') => {
            let request = AssetDecodeRequest {
                logical_path: logical_path.to_owned(),
                output_kind: ASSET_LIST_FILE_MANIFEST_OUTPUT.to_owned(),
                selector: json!({}),
            };
            let bytes = client.decode_v1(&request).map_err(|decode_error| {
                format!("VFS listing failed: {vfs_error}; ListFile listing failed: {decode_error}")
            })?;
            let manifest = serde_json::from_slice::<AssetFileManifest>(&bytes)
                .map_err(|error| format!("invalid native ListFile manifest: {error}"))?;
            Ok(manifest
                .entries
                .into_iter()
                .map(|entry| InspectorEntry {
                    name: entry.name,
                    logical_path: entry.entry_ref,
                    kind: entry.asset_kind,
                    extension: extension_from_ref(logical_path),
                    is_directory: false,
                    source_asset: false,
                    byte_len: None,
                })
                .collect())
        }
        Err(error) => Err(format!("engine.assets VFS listing failed: {error}")),
    }
}

fn entries_from_vfs(value: &Value) -> Vec<InspectorEntry> {
    value
        .get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|entry| {
            let name = first_string(entry, &["name", "file_name", "display_name"])
                .unwrap_or_else(|| "<unnamed>".to_owned());
            let logical_path = first_string(entry, &["logical_path", "path", "reference", "id"])
                .unwrap_or_else(|| name.clone())
                .replace('\\', "/")
                .trim_matches('/')
                .to_owned();
            let kind = first_string(entry, &["kind", "node_kind", "entry_kind"])
                .unwrap_or_else(|| "asset".to_owned());
            let is_directory = entry
                .get("is_dir")
                .or_else(|| entry.get("directory"))
                .or_else(|| entry.get("is_directory"))
                .and_then(Value::as_bool)
                .unwrap_or_else(|| {
                    matches!(
                        kind.to_ascii_lowercase().as_str(),
                        "directory" | "dir" | "folder" | "mount"
                    )
                });
            InspectorEntry {
                name,
                extension: extension_from_ref(&logical_path),
                source_asset: is_source_asset_ref(&logical_path),
                logical_path,
                kind,
                is_directory,
                byte_len: entry
                    .get("byte_len")
                    .or_else(|| entry.get("size"))
                    .and_then(Value::as_u64),
            }
        })
        .collect()
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

fn extension_from_ref(value: &str) -> String {
    let path = value
        .split('@')
        .next()
        .unwrap_or(value)
        .to_ascii_lowercase();
    for compound in ["ymap.xml", "ytyp.xml", "nemat.xml", "neui.xml"] {
        if path.ends_with(compound) {
            return compound.to_owned();
        }
    }
    path.rsplit_once('.')
        .map(|(_, extension)| extension.to_owned())
        .unwrap_or_default()
}

fn parse_row(node_id: &str) -> Option<usize> {
    node_id
        .strip_prefix("asset.inspector.entry.")?
        .parse::<usize>()
        .ok()
        .filter(|row| *row < ENTRY_ROWS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_authored_row_ids() {
        assert_eq!(parse_row("asset.inspector.entry.07"), Some(7));
        assert_eq!(parse_row("other.07"), None);
    }

    #[test]
    fn source_mode_keeps_directories_and_source_files() {
        assert!(AssetInspectorMode::Source.accepts(true, false));
        assert!(AssetInspectorMode::Source.accepts(false, true));
        assert!(!AssetInspectorMode::Source.accepts(false, false));
    }
}
