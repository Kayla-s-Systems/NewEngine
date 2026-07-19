use super::super::document::{available_document_action, document_field};
use super::super::*;
use super::selection::parse_index;

impl AssetInspectorRuntimeModule {
    pub(in crate::runtime) fn handle_hover(&mut self, node_id: &str, trigger: UiNodeEventTrigger) {
        match trigger {
            UiNodeEventTrigger::HoverEnter => {
                self.hovered_node = Some(node_id.to_owned());
                self.hover_hint = self.hover_hint_for_node(node_id);
            }
            UiNodeEventTrigger::HoverExit => {
                if self.hovered_node.as_deref() == Some(node_id) {
                    self.hovered_node = None;
                    self.hover_hint.clear();
                }
            }
            _ => {}
        }
    }

    pub(in crate::runtime::interaction) fn hover_hint_for_node(&self, node_id: &str) -> String {
        if let Some(row) = parse_index(node_id, "asset.inspector.entry.", ENTRY_ROWS) {
            let absolute = self.browser_window_start + row;
            if let Some(entry) = self.entries.get(absolute) {
                return if entry.is_parent_navigation() {
                    "Go to the parent directory".to_owned()
                } else if entry.is_directory {
                    format!("Open folder {}", entry.logical_path)
                } else {
                    format!(
                        "Single-click to select {} | Double-click to preview | Mouse wheel scrolls this list | {}",
                        entry.name, entry.logical_path
                    )
                };
            }
        }
        if let Some(row) = parse_index(
            node_id,
            "asset.inspector.preview_entry.",
            PREVIEW_ENTRY_ROWS,
        ) {
            let absolute = self.preview_entries_window_start + row;
            if let Some(entry) = self.preview_entries.get(absolute) {
                return format!(
                    "Preview provider entry {} | {}",
                    entry.name, entry.logical_path
                );
            }
        }
        if let Some(row) = parse_index(node_id, "asset.inspector.document_action.", ACTION_ROWS) {
            if let Some(document) = self.document.as_ref() {
                if let Some(action) = available_document_action(document, row) {
                    if !action.tooltip.trim().is_empty() {
                        return action.tooltip.clone();
                    }
                    return format!("Run provider action: {}", action.label);
                }
            }
        }
        if let Some(row) = parse_index(node_id, "asset.inspector.field.", FIELD_ROWS) {
            if let Some(document) = self.document.as_ref() {
                if let Some(field) = document_field(document, row) {
                    if !field.help.trim().is_empty() {
                        return format!("{} | {}", field.label, field.help);
                    }
                    let editable = document.can_apply_patch
                        && field.editable
                        && (!field.source_pointer.trim().is_empty()
                            || field
                                .schema_property
                                .as_ref()
                                .is_some_and(|property| !property.json_pointer.trim().is_empty()));
                    return if editable {
                        format!("Edit {} ({})", field.label, field.value_kind)
                    } else {
                        format!("Read-only metadata: {} ({})", field.label, field.value_kind)
                    };
                }
            }
        }
        match node_id {
            "asset.inspector.up" => {
                "Go to the parent location. The currently inspected asset stays open.".to_owned()
            }
            "asset.inspector.refresh" => {
                "Refresh the current VFS listing without closing the preview.".to_owned()
            }
            "asset.inspector.mode.all" => "Show folders and assets.".to_owned(),
            "asset.inspector.mode.assets" => "Show assets only.".to_owned(),
            "asset.inspector.mode.folders" => "Show folders only.".to_owned(),
            "asset.inspector.info.open" => {
                "Open complete provider, schema and diagnostic information.".to_owned()
            }
            "asset.inspector.info.close" => "Close asset information.".to_owned(),
            "asset.inspector.preview_entries.refresh" => {
                "Refresh the provider entry list without rebuilding the active preview.".to_owned()
            }
            "asset.inspector.container.open" => format!(
                "Open {} addressable provider entr{}.",
                self.selected_container_entry_count,
                if self.selected_container_entry_count == 1 {
                    "y"
                } else {
                    "ies"
                }
            ),
            "asset.inspector.preview" | "asset.inspector.preview.image" => {
                if self
                    .preview_snapshot
                    .as_ref()
                    .is_some_and(|preview| preview.kind == AssetPreviewKind::Scene3d)
                {
                    "LMB drag orbits | MMB drag pans the camera | Mouse wheel zooms | Resetting the preview restores the centered view."
                        .to_owned()
                } else {
                    "Provider-resolved preview. The Inspector never invents proxy content."
                        .to_owned()
                }
            }
            "asset.inspector.diagnostics" => {
                "Provider, schema, preview and mutation diagnostics for the open asset.".to_owned()
            }
            _ => String::new(),
        }
    }
}
