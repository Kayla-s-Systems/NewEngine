use super::super::*;

impl AssetInspectorRuntimeModule {
    pub(in crate::runtime) fn clear_browser_selection(&mut self) {
        self.selected_index = None;
    }

    pub(in crate::runtime) fn clear_selection(&mut self) {
        self.preview_api.clear();
        self.preview_snapshot = None;
        self.preview_pointer_captured = false;
        self.preview_middle_pan_active = false;
        self.current_preview_cache_valid = false;
        self.selected_index = None;
        self.pending_entry_activation = None;
        self.pending_preview_entry_activation = None;
        self.pending_preview_entries_load = None;
        self.preview_entries.clear();
        self.preview_entries_source.clear();
        self.selected_preview_entry = None;
        self.preview_entries_window_start = 0;
        self.info_modal_visible = false;
        self.text_editor = None;
        self.syntax_preview = None;
        self.syntax_editor = None;
        self.document = None;
        self.last_patch_result = None;
        self.selected_container_entry_count = 0;
        self.selected_container_available = false;
    }
}

pub(in crate::runtime::interaction) fn is_preview_image_node(node_id: &str) -> bool {
    node_id == "asset.inspector.preview.image"
        || node_id.starts_with("asset.inspector.preview.image.")
        || node_id == "asset.inspector.preview.controls"
        || node_id.starts_with("asset.inspector.preview.controls.")
}

pub(in crate::runtime) fn parse_index(
    node_id: &str,
    prefix: &str,
    capacity: usize,
) -> Option<usize> {
    node_id
        .strip_prefix(prefix)?
        .split('.')
        .next()?
        .parse::<usize>()
        .ok()
        .filter(|row| *row < capacity)
}
