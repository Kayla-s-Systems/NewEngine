use super::*;

impl AssetsCatalogUiRuntimeModule {
    pub(crate) fn handle_ui_dispatch_frame(
        &mut self,
        dispatch: Option<&UiEventDispatchFrame>,
        _input: &UiInputFrame,
        surface_size_px: [u32; 2],
        frame_index: u64,
    ) {
        let Some(dispatch) = dispatch else {
            return;
        };
        let hovered = dispatch
            .hovered_node
            .as_ref()
            .filter(|hit| hit.surface_id == ASSETS_CATALOG_SURFACE_ID);

        let previous_hover = self.hovered_entry_index;
        self.hovered_entry_index = hovered.and_then(|hit| self.entry_index_from_dispatch_hit(hit));
        if self.hovered_entry_index != previous_hover {
            self.invalidate_node();
        }

        if let Some(wheel_y) = dispatch_wheel_y(dispatch, ASSETS_CATALOG_SURFACE_ID) {
            self.select_by_wheel(wheel_y);
        }

        let mut consumed = false;
        for action in dispatch
            .actions
            .iter()
            .filter(|action| action.surface_id == ASSETS_CATALOG_SURFACE_ID)
        {
            consumed |= self.handle_ui_action(action, surface_size_px, frame_index);
        }

        if self.main_scrollbar_dragging {
            if let Some(action) = dispatch.actions.iter().find(|action| {
                action.action_id == UI_SCROLLBAR_DRAG_ACTION
                    && action.trigger == UiNodeEventTrigger::DragMove
            }) {
                consumed |= self.handle_scrollbar_action(action);
            }
        }

        if !consumed
            && self.context_menu_open
            && dispatch.actions.iter().any(|action| {
                matches!(
                    action.trigger,
                    UiNodeEventTrigger::Press
                        | UiNodeEventTrigger::ContextMenu
                        | UiNodeEventTrigger::Click
                )
            })
            && hovered.is_none()
        {
            self.context_menu_open = false;
            self.invalidate_node();
        }
    }

    pub(crate) fn handle_ui_action(
        &mut self,
        action: &UiActionDispatch,
        surface_size_px: [u32; 2],
        frame_index: u64,
    ) -> bool {
        if self.last_pointer_frame == frame_index
            && matches!(
                action.trigger,
                UiNodeEventTrigger::Click | UiNodeEventTrigger::ContextMenu
            )
        {
            return false;
        }

        if action.action_id == UI_SCROLLBAR_DRAG_ACTION {
            if matches!(
                action.trigger,
                UiNodeEventTrigger::Press | UiNodeEventTrigger::DragStart
            ) {
                self.main_scrollbar_dragging = true;
                return self.handle_scrollbar_action(action);
            }
            if action.trigger == UiNodeEventTrigger::DragMove {
                return self.handle_scrollbar_action(action);
            }
            if matches!(
                action.trigger,
                UiNodeEventTrigger::Release | UiNodeEventTrigger::DragEnd
            ) {
                self.main_scrollbar_dragging = false;
                return true;
            }
            return false;
        }

        if action.action_id == UI_SCROLL_WHEEL_ACTION
            && action.trigger == UiNodeEventTrigger::ValueChanged
        {
            return action_payload_array_f32(action, "wheel", 1)
                .map(|wheel_y| self.select_by_wheel(wheel_y))
                .unwrap_or(false);
        }

        if action.trigger == UiNodeEventTrigger::ContextMenu {
            let Some(entry_index) = self.entry_index_from_action(action) else {
                return false;
            };
            if let Some(snapshot) = self.cached_snapshot.as_ref() {
                if entry_index < snapshot.entries.len() {
                    self.selected_index = entry_index;
                    self.context_menu_open = true;
                    self.focus_scope = CatalogFocusScope::Grid;
                    self.invalidate_node();
                    self.last_pointer_frame = frame_index;
                    return true;
                }
            }
            return false;
        }

        if action.trigger != UiNodeEventTrigger::Click {
            return false;
        }

        self.last_pointer_frame = frame_index;
        match action.action_id.as_str() {
            "asset_browser.view.tree" => self.set_view_mode(CatalogViewMode::Tree),
            "asset_browser.view.list" => self.set_view_mode(CatalogViewMode::List),
            "asset_browser.view.grid" => self.set_view_mode(CatalogViewMode::Grid),
            "asset_browser.view.inspector" => self.set_view_mode(CatalogViewMode::Inspector),
            "asset_browser.search.focus" => {
                self.focus_scope = CatalogFocusScope::Search;
                self.invalidate_node();
                true
            }
            "asset_browser.breadcrumb.open" => {
                let Some(snapshot) = self.cached_snapshot.as_ref() else {
                    return false;
                };
                let path = breadcrumb_path_from_action(action, snapshot);
                self.current_path = normalize_catalog_path(&path);
                self.selected_index = 0;
                self.focus_scope = CatalogFocusScope::Breadcrumb;
                self.cached_snapshot = None;
                self.context_menu_open = false;
                self.invalidate_node();
                self.refresh_cache(frame_index, surface_size_px);
                newengine_ulog_api::ulog::info!(
                    "asset browser UI: breadcrumb open path='{}' via ui.dispatch_input_v1",
                    display_path(&self.current_path)
                );
                true
            }
            "asset_browser.root.open" => {
                self.current_path.clear();
                self.selected_index = 0;
                self.view_mode = CatalogViewMode::Grid;
                self.search_query.clear();
                self.focus_scope = CatalogFocusScope::Tree;
                self.cached_snapshot = None;
                self.context_menu_open = false;
                self.invalidate_node();
                self.refresh_cache(frame_index, surface_size_px);
                newengine_ulog_api::ulog::info!(
                    "asset browser UI: root opened via ui.dispatch_input_v1"
                );
                true
            }
            "asset_browser.folder.open" | "asset_browser.sidebar.select" => {
                let Some(entry_index) = self.entry_index_from_action(action) else {
                    return false;
                };
                self.open_folder_entry(entry_index, frame_index, surface_size_px)
            }
            "asset_browser.asset.select" | "asset_browser.details.inspect" => {
                let Some(entry_index) = self.entry_index_from_action(action) else {
                    return false;
                };
                self.select_asset_entry(entry_index, frame_index, surface_size_px)
            }
            id if self
                .cached_document_actions
                .iter()
                .any(|action| action.id == id) =>
            {
                self.context_menu_open = false;
                self.dispatch_asset_document_action(id, frame_index, surface_size_px);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn set_view_mode(&mut self, view_mode: CatalogViewMode) -> bool {
        self.view_mode = view_mode;
        self.focus_scope = match view_mode {
            CatalogViewMode::Tree => CatalogFocusScope::Tree,
            CatalogViewMode::List | CatalogViewMode::Grid => CatalogFocusScope::Grid,
            CatalogViewMode::Inspector => CatalogFocusScope::Inspector,
        };
        self.context_menu_open = false;
        self.invalidate_node();
        true
    }
}
