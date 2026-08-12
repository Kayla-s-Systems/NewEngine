use std::sync::Arc;
use std::time::Instant;

use newengine_assets_api::{AssetDocument, AssetDocumentField, AssetPatchResult};
use newengine_core::{EngineReadinessKey, EngineResult, Module, ModuleCtx};
use newengine_engine_runtime::{AssetPreviewApi, AssetPreviewKind, AssetPreviewSnapshot};
use newengine_math::collections::BoundedCache;
use newengine_ui_api::{UiEventDispatchFrame, UiInputFrame, UiNodeEventTrigger};

use crate::facade::EngineAssetFacade;
use crate::model::{AssetInspectorMode, InspectorEntry};
use crate::surface::mount_asset_inspector_surface;
use crate::syntax_preview::{highlight_editor_page, highlight_preview_page, SyntaxPreviewPage};
use crate::ui_state::{
    publish_inspector_state, InspectorUiSnapshot, ACTION_ROWS, ENTRY_ROWS, FIELD_ROWS,
    PREVIEW_ENTRY_ROWS, TEXT_ROWS,
};
use crate::ASSET_INSPECTOR_SURFACE_ID;

mod activity;
mod document;
mod interaction;
mod lifecycle;
mod navigation;
mod presentation;
#[cfg(test)]
mod tests;
const ACTION_REFRESH: &str = "asset.inspector.refresh";
const ACTION_UP: &str = "asset.inspector.up";
const ACTION_MODE_ALL: &str = "asset.inspector.mode.all";
const ACTION_MODE_ASSETS: &str = "asset.inspector.mode.assets";
const ACTION_MODE_FOLDERS: &str = "asset.inspector.mode.folders";
const ACTION_ENTRY: &str = "asset.inspector.entry";
const ACTION_CONTAINER_OPEN: &str = "asset.inspector.container.open";
const ACTION_PREVIEW_ENTRY: &str = "asset.inspector.preview_entry";
const ACTION_PREVIEW_ENTRIES_REFRESH: &str = "asset.inspector.preview_entries.refresh";
const ACTION_INFO_OPEN: &str = "asset.inspector.info.open";
const ACTION_INFO_CLOSE: &str = "asset.inspector.info.close";
const ACTION_FIELD_EDIT: &str = "asset.inspector.field.edit";
const ACTION_DOCUMENT_ACTION: &str = "asset.inspector.document_action";
const ACTION_HOVER: &str = "asset.inspector.hover";
const ACTION_TEXT_LINE_EDIT: &str = "asset.inspector.text_line.edit";
const ACTION_TEXT_PREVIOUS: &str = "asset.inspector.text.previous";
const ACTION_TEXT_NEXT: &str = "asset.inspector.text.next";
const ACTION_TEXT_SAVE: &str = "asset.inspector.text.save";
const ACTION_TEXT_DISCARD: &str = "asset.inspector.text.discard";
const ACTION_TEXT_CLOSE: &str = "asset.inspector.text.close";
const UI_SCROLLBAR_DRAG_ACTION: &str = "ui.scrollbar.drag";
const UI_SCROLL_WHEEL_ACTION: &str = "ui.scroll.wheel";
const BROWSER_SCROLL_NODE_ID: &str = "asset.inspector.browser.scroll";
const PREVIEW_ENTRIES_SCROLL_NODE_ID: &str = "asset.inspector.entries.scroll";
const STARTUP_ASSET_ENV: &str = "NEWENGINE_ASSET_INSPECTOR_OPEN";
const ACTIVITY_BAR_INNER_WIDTH_PX: f32 = 156.0;
const ACTIVITY_COMPLETE_ANIMATION_FRAMES: u64 = 18;
const ACTIVITY_COMPLETE_HOLD_FRAMES: u64 = 12;
const ACTIVITY_PUBLISH_INTERVAL_FRAMES: u64 = 3;
const DOCUMENT_CACHE_CAPACITY: usize = 8;
const PREVIEW_WIDTH: u32 = 488;
const PREVIEW_HEIGHT: u32 = 236;
const PREVIEW_ENTRY_CACHE_CAPACITY: usize = 8;
const PREVIEW_ENTRY_LOAD_DELAY_FRAMES: u64 = 2;
const PREVIEW_PAN_MOUSE_BUTTON: u32 = 3; // newengine_input_api::mouse_button::MIDDLE

#[derive(Clone, Debug)]
struct InspectorActivity {
    label: String,
    started_frame: u64,
    completed_frame: Option<u64>,
    waiting_for_preview: bool,
    last_published_frame: u64,
}

#[derive(Clone, Debug)]
struct TextEditorState {
    asset_ref: String,
    original_text: String,
    lines: Vec<String>,
    line_ending: &'static str,
    page: usize,
    language: String,
    editable: bool,
    dirty: bool,
}

impl TextEditorState {
    fn from_document(document: &AssetDocument) -> Option<Self> {
        let text = document.text.as_ref()?;
        let line_ending = if text.content.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let normalized = text.content.replace("\r\n", "\n");
        let lines = normalized
            .split('\n')
            .map(str::to_owned)
            .collect::<Vec<_>>();
        Some(Self {
            asset_ref: document.asset_ref.clone(),
            original_text: text.content.clone(),
            lines: if lines.is_empty() {
                vec![String::new()]
            } else {
                lines
            },
            line_ending,
            page: 0,
            language: text.language.clone(),
            editable: text.editable && !text.truncated,
            dirty: false,
        })
    }

    fn compose(&self) -> String {
        self.lines.join(self.line_ending)
    }

    fn total_pages(&self) -> usize {
        self.lines.len().max(1).div_ceil(TEXT_ROWS)
    }

    fn reset(&mut self) {
        let normalized = self.original_text.replace("\r\n", "\n");
        self.lines = normalized.split('\n').map(str::to_owned).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.page = self.page.min(self.total_pages().saturating_sub(1));
        self.dirty = false;
    }
}

#[derive(Clone, Debug)]
struct DocumentCache {
    entries: BoundedCache<String, AssetDocument>,
}

impl Default for DocumentCache {
    fn default() -> Self {
        Self {
            entries: BoundedCache::new(DOCUMENT_CACHE_CAPACITY),
        }
    }
}

impl DocumentCache {
    fn get(&mut self, asset_ref: &str) -> Option<AssetDocument> {
        self.entries.get(asset_ref).cloned()
    }

    fn insert(&mut self, document: &AssetDocument) {
        self.entries
            .insert(document.asset_ref.clone(), document.clone());
    }

    fn invalidate(&mut self, asset_ref: &str) {
        let _ = self.entries.remove(asset_ref);
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

#[derive(Clone, Debug)]
struct PreviewEntryCache {
    entries: BoundedCache<String, Vec<InspectorEntry>>,
}

impl Default for PreviewEntryCache {
    fn default() -> Self {
        Self {
            entries: BoundedCache::new(PREVIEW_ENTRY_CACHE_CAPACITY),
        }
    }
}

impl PreviewEntryCache {
    fn get(&mut self, source_ref: &str) -> Option<Vec<InspectorEntry>> {
        self.entries.get(source_ref).cloned()
    }

    fn insert(&mut self, source_ref: &str, entries: &[InspectorEntry]) {
        self.entries.insert(source_ref.to_owned(), entries.to_vec());
    }

    fn invalidate(&mut self, source_ref: &str) {
        let _ = self.entries.remove(source_ref);
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

#[derive(Clone, Debug)]
struct PendingPreviewEntriesLoad {
    source_ref: String,
    requested_frame: u64,
}

#[derive(Clone, Debug)]
struct PendingPreviewEntryActivation {
    entry: InspectorEntry,
    row: usize,
    requested_frame: u64,
}

#[derive(Clone, Debug)]
struct PendingEntryActivation {
    entry: InspectorEntry,
    absolute_index: usize,
    requested_frame: u64,
}

pub struct AssetInspectorRuntimeModule {
    facade: EngineAssetFacade,
    preview_api: Arc<AssetPreviewApi>,
    preview_snapshot: Option<AssetPreviewSnapshot>,
    startup_asset_ref: Option<String>,
    startup_asset_opened: bool,
    startup_asset_attempts: u32,
    last_startup_asset_attempt_frame: Option<u64>,
    current_path: String,
    inside_container: bool,
    mode: AssetInspectorMode,
    browser_window_start: usize,
    entries: Vec<InspectorEntry>,
    selected_index: Option<usize>,
    pending_entry_activation: Option<PendingEntryActivation>,
    pending_preview_entry_activation: Option<PendingPreviewEntryActivation>,
    pending_preview_entries_load: Option<PendingPreviewEntriesLoad>,
    preview_entry_cache: PreviewEntryCache,
    preview_entries: Vec<InspectorEntry>,
    preview_entries_source: String,
    selected_preview_entry: Option<usize>,
    preview_entries_window_start: usize,
    document_cache: DocumentCache,
    text_editor: Option<TextEditorState>,
    syntax_preview: Option<SyntaxPreviewPage>,
    syntax_editor: Option<SyntaxPreviewPage>,
    document: Option<AssetDocument>,
    last_patch_result: Option<AssetPatchResult>,
    selected_container_entry_count: usize,
    selected_container_available: bool,
    status: String,
    activity: Option<InspectorActivity>,
    hovered_node: Option<String>,
    hover_hint: String,
    preview_pointer_captured: bool,
    preview_middle_pan_active: bool,
    current_preview_cache_valid: bool,
    info_modal_visible: bool,
    last_refresh_frame: Option<u64>,
    last_action_frame: Option<u64>,
    dirty: bool,
    surface_mounted: bool,
    last_surface_mount_attempt_frame: Option<u64>,
}

impl AssetInspectorRuntimeModule {
    pub fn new(preview_api: Arc<AssetPreviewApi>) -> Self {
        Self {
            facade: EngineAssetFacade::new(),
            preview_api,
            preview_snapshot: None,
            startup_asset_ref: crate::env_config::normalized_logical_ref(STARTUP_ASSET_ENV),
            startup_asset_opened: false,
            startup_asset_attempts: 0,
            last_startup_asset_attempt_frame: None,
            current_path: String::new(),
            inside_container: false,
            mode: AssetInspectorMode::All,
            browser_window_start: 0,
            entries: Vec::new(),
            selected_index: None,
            pending_entry_activation: None,
            pending_preview_entry_activation: None,
            pending_preview_entries_load: None,
            preview_entry_cache: PreviewEntryCache::default(),
            preview_entries: Vec::new(),
            preview_entries_source: String::new(),
            selected_preview_entry: None,
            preview_entries_window_start: 0,
            document_cache: DocumentCache::default(),
            text_editor: None,
            syntax_preview: None,
            syntax_editor: None,
            document: None,
            last_patch_result: None,
            selected_container_entry_count: 0,
            selected_container_available: false,
            status: "Waiting for engine.assets and engine.ui".to_owned(),
            activity: None,
            hovered_node: None,
            hover_hint: String::new(),
            preview_pointer_captured: false,
            preview_middle_pan_active: false,
            current_preview_cache_valid: false,
            info_modal_visible: false,
            last_refresh_frame: None,
            last_action_frame: None,
            dirty: true,
            surface_mounted: false,
            last_surface_mount_attempt_frame: None,
        }
    }
}
