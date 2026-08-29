use newengine_ui_api::UiActionDispatch;

use super::super::*;
use super::selection::parse_index;

impl AssetInspectorRuntimeModule {
    pub(in crate::runtime) fn handle_actions(&mut self, frame: &UiEventDispatchFrame) {
        if self.last_action_frame == Some(frame.frame_index) {
            return;
        }
        let mut consumed = false;
        for action in frame
            .actions
            .iter()
            .filter(|action| action.surface_id == ASSET_INSPECTOR_SURFACE_ID)
        {
            consumed |= self.dispatch_action(action, frame.frame_index);
        }
        if consumed {
            self.last_action_frame = Some(frame.frame_index);
            self.dirty = true;
        }
    }

    fn dispatch_action(&mut self, action: &UiActionDispatch, frame_index: u64) -> bool {
        if matches!(
            action.action_id.as_str(),
            UI_SCROLLBAR_DRAG_ACTION | UI_SCROLL_WHEEL_ACTION
        ) {
            return self.handle_generic_scroll_action(action);
        }
        match (action.action_id.as_str(), action.trigger) {
            (ACTION_REFRESH, UiNodeEventTrigger::Click) => {
                self.refresh_with_activity(frame_index);
                true
            }
            (ACTION_UP, UiNodeEventTrigger::Click) => {
                self.navigate_up();
                true
            }
            (ACTION_MODE_ALL, UiNodeEventTrigger::Click) => {
                self.set_mode(AssetInspectorMode::All);
                true
            }
            (ACTION_MODE_ASSETS, UiNodeEventTrigger::Click) => {
                self.set_mode(AssetInspectorMode::Assets);
                true
            }
            (ACTION_MODE_FOLDERS, UiNodeEventTrigger::Click) => {
                self.set_mode(AssetInspectorMode::Folders);
                true
            }
            (ACTION_ENTRY, UiNodeEventTrigger::Click | UiNodeEventTrigger::DoubleClick) => self
                .dispatch_indexed_action(
                    action,
                    "asset.inspector.entry.",
                    ENTRY_ROWS,
                    |runtime, row| {
                        runtime.activate_row(
                            row,
                            action.trigger == UiNodeEventTrigger::DoubleClick,
                            frame_index,
                        );
                    },
                ),
            (ACTION_CONTAINER_OPEN, UiNodeEventTrigger::Click) => {
                self.open_selected_container(frame_index);
                true
            }
            (ACTION_PREVIEW_ENTRY, UiNodeEventTrigger::Click | UiNodeEventTrigger::DoubleClick) => {
                self.dispatch_indexed_action(
                    action,
                    "asset.inspector.preview_entry.",
                    PREVIEW_ENTRY_ROWS,
                    |runtime, row| runtime.activate_preview_entry(row, frame_index),
                )
            }
            (ACTION_PREVIEW_ENTRIES_REFRESH, UiNodeEventTrigger::Click) => {
                self.refresh_preview_entries(frame_index);
                true
            }
            (ACTION_INFO_OPEN, UiNodeEventTrigger::Click) => {
                if self.document.is_none() {
                    return false;
                }
                self.info_modal_visible = true;
                self.status = "Asset information".to_owned();
                true
            }
            (ACTION_INFO_CLOSE, UiNodeEventTrigger::Click) => {
                self.info_modal_visible = false;
                self.status = "Asset preview".to_owned();
                true
            }
            (ACTION_FIELD_EDIT, UiNodeEventTrigger::ValueChanged) => self.dispatch_indexed_action(
                action,
                "asset.inspector.field.",
                FIELD_ROWS,
                |runtime, row| runtime.edit_field(row, &action.payload, frame_index),
            ),
            (ACTION_DOCUMENT_ACTION, UiNodeEventTrigger::Click) => self.dispatch_indexed_action(
                action,
                "asset.inspector.document_action.",
                ACTION_ROWS,
                |runtime, row| runtime.dispatch_document_action(row, frame_index),
            ),
            (ACTION_TEXT_LINE_EDIT, UiNodeEventTrigger::ValueChanged) => self
                .dispatch_indexed_action(
                    action,
                    "asset.inspector.text.line.",
                    TEXT_ROWS,
                    |runtime, row| runtime.edit_text_line(row, &action.payload),
                ),
            (ACTION_TEXT_PREVIOUS, UiNodeEventTrigger::Click) => {
                self.text_previous_page();
                true
            }
            (ACTION_TEXT_NEXT, UiNodeEventTrigger::Click) => {
                self.text_next_page();
                true
            }
            (ACTION_TEXT_SAVE, UiNodeEventTrigger::Click) => {
                self.save_text_document(frame_index);
                true
            }
            (ACTION_TEXT_DISCARD, UiNodeEventTrigger::Click) => {
                self.discard_text_changes();
                true
            }
            (ACTION_TEXT_CLOSE, UiNodeEventTrigger::Click) => {
                self.text_editor = None;
                self.syntax_editor = None;
                self.status = "Asset browser | text preview remains open".to_owned();
                true
            }
            (ACTION_HOVER, UiNodeEventTrigger::HoverEnter | UiNodeEventTrigger::HoverExit) => {
                self.handle_hover(&action.node_id, action.trigger);
                true
            }
            _ => false,
        }
    }

    fn dispatch_indexed_action(
        &mut self,
        action: &UiActionDispatch,
        prefix: &str,
        capacity: usize,
        dispatch: impl FnOnce(&mut Self, usize),
    ) -> bool {
        let Some(row) = parse_index(&action.node_id, prefix, capacity) else {
            return false;
        };
        dispatch(self, row);
        true
    }
}
