use newengine_asset_preview_runtime::{AssetPreviewKind, AssetPreviewSnapshot};
use newengine_assets_api::{AssetDocument, AssetDocumentField, AssetPatchResult};
use newengine_ui_api::{
    UiStatePatch, ENGINE_UI_SERVICE_ID, UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1,
};
use serde_json::{json, Value};

use crate::model::{AssetInspectorMode, InspectorEntry};
use crate::syntax_preview::{SyntaxPreviewPage, SYNTAX_LAYER_NAMES, SYNTAX_PREVIEW_ROWS};
use crate::{
    ASSET_INSPECTOR_STATE_CONTRACT, ASSET_INSPECTOR_STATE_SOURCE, ASSET_INSPECTOR_SURFACE_ID,
};

mod browser;
mod document;
mod preview;
mod shell;
#[cfg(test)]
mod tests;
mod text_editor;
mod transport;

use browser::publish_browser_entries;
use document::{
    publish_action_state, publish_diagnostics, publish_document_state, publish_field_state,
};
use preview::{publish_preview_entry_state, publish_preview_state};
use shell::publish_shell_state;
use text_editor::publish_text_editor_state;
use transport::submit_state_patch;

pub(crate) const ENTRY_ROWS: usize = 12;
pub(crate) const FIELD_ROWS: usize = 10;
pub(crate) const ACTION_ROWS: usize = 6;
pub(crate) const DIAGNOSTIC_ROWS: usize = 4;
pub(crate) const TEXT_ROWS: usize = 16;
pub(crate) const PREVIEW_ENTRY_ROWS: usize = 5;

pub(crate) struct InspectorUiSnapshot<'a> {
    pub(crate) frame_index: u64,
    pub(crate) current_path: &'a str,
    pub(crate) inside_container: bool,
    pub(crate) mode: AssetInspectorMode,
    pub(crate) browser_window_start: usize,
    pub(crate) entries: &'a [InspectorEntry],
    pub(crate) selected_index: Option<usize>,
    pub(crate) document: Option<&'a AssetDocument>,
    pub(crate) preview: Option<&'a AssetPreviewSnapshot>,
    pub(crate) last_patch_result: Option<&'a AssetPatchResult>,
    pub(crate) selected_container_entry_count: usize,
    pub(crate) selected_container_available: bool,
    pub(crate) preview_entries: &'a [InspectorEntry],
    pub(crate) selected_preview_entry: Option<usize>,
    pub(crate) preview_entries_window_start: usize,
    pub(crate) preview_entries_loading: bool,
    pub(crate) info_modal_visible: bool,
    pub(crate) status: &'a str,
    pub(crate) hover_hint: &'a str,
    pub(crate) activity_progress_01: f32,
    pub(crate) activity_width_px: f32,
    pub(crate) activity_label: &'a str,
    pub(crate) text_asset_ref: Option<&'a str>,
    pub(crate) text_lines: Option<&'a [String]>,
    pub(crate) text_page: usize,
    pub(crate) text_language: &'a str,
    pub(crate) text_editable: bool,
    pub(crate) text_dirty: bool,
    pub(crate) syntax_preview: Option<&'a SyntaxPreviewPage>,
    pub(crate) syntax_editor: Option<&'a SyntaxPreviewPage>,
    pub(crate) preview_pointer_captured: bool,
}

pub(crate) fn publish_inspector_state(snapshot: InspectorUiSnapshot<'_>) -> bool {
    let max_start = snapshot.entries.len().saturating_sub(ENTRY_ROWS);
    let start = snapshot.browser_window_start.min(max_start);
    let end = (start + ENTRY_ROWS).min(snapshot.entries.len());
    let visible_entries = &snapshot.entries[start..end];

    let mut patch = UiStatePatch::new(snapshot.frame_index, ASSET_INSPECTOR_SURFACE_ID);
    patch = publish_shell_state(patch, &snapshot, start);
    patch = publish_text_editor_state(patch, &snapshot);
    patch = publish_browser_entries(patch, &snapshot, start, visible_entries);
    patch = publish_document_state(
        patch,
        snapshot.document,
        snapshot.selected_container_available,
        snapshot.selected_container_entry_count,
    );
    patch = publish_preview_state(
        patch,
        snapshot.document,
        snapshot.preview,
        snapshot.syntax_preview,
        snapshot.preview_pointer_captured,
        snapshot.selected_container_available,
    );
    patch = publish_preview_entry_state(
        patch,
        snapshot.preview_entries,
        snapshot.selected_preview_entry,
        snapshot.preview_entries_window_start,
        snapshot.preview_entries_loading,
        snapshot.selected_container_available,
    );
    patch = publish_field_state(patch, snapshot.document, snapshot.info_modal_visible);
    patch = publish_action_state(patch, snapshot.document);
    patch = publish_diagnostics(
        patch,
        snapshot.document,
        snapshot.preview,
        snapshot.last_patch_result,
        snapshot.info_modal_visible,
    );
    submit_state_patch(patch)
}
