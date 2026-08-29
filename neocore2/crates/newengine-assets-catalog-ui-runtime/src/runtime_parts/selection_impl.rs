use super::*;

impl AssetsCatalogUiRuntimeModule {
    pub(crate) fn select_by_wheel(&mut self, wheel_y: f32) -> bool {
        let Some(snapshot) = self.cached_snapshot.as_ref() else {
            return false;
        };
        let visible = filtered_entry_indices(
            snapshot,
            self.view_mode,
            &self.search_query,
            &self.collapsed_paths,
        );
        if visible.is_empty() {
            return false;
        }
        let slot = visible
            .iter()
            .position(|idx| *idx == self.selected_index)
            .unwrap_or(0);
        let wheel_steps = wheel_y.abs().ceil().max(1.0) as usize;
        let next_slot = if wheel_y > 0.0 {
            slot.saturating_sub(wheel_steps)
        } else {
            (slot + wheel_steps).min(visible.len().saturating_sub(1))
        };
        let next_index = visible[next_slot];
        if self.selected_index == next_index {
            return false;
        }
        self.selected_index = next_index;
        self.focus_scope = CatalogFocusScope::Grid;
        self.context_menu_open = false;
        self.invalidate_node();
        true
    }

    pub(crate) fn handle_scrollbar_action(&mut self, action: &UiActionDispatch) -> bool {
        let Some(snapshot) = self.cached_snapshot.as_ref() else {
            return false;
        };
        let visible = filtered_entry_indices(
            snapshot,
            self.view_mode,
            &self.search_query,
            &self.collapsed_paths,
        );
        if visible.is_empty() {
            return false;
        }
        let Some(local_y) = action_payload_f32(action, "local_pos", 1) else {
            return false;
        };
        let Some(height) =
            action_payload_f32(action, "global_rect", 3).filter(|height| *height > 0.0)
        else {
            return false;
        };
        let ratio = (local_y / height).clamp(0.0, 1.0);
        let slot = ((visible.len().saturating_sub(1)) as f32 * ratio).round() as usize;
        let target_index = visible[slot.min(visible.len().saturating_sub(1))];
        if self.selected_index == target_index {
            return false;
        }
        self.selected_index = target_index;
        self.focus_scope = CatalogFocusScope::Grid;
        self.context_menu_open = false;
        self.invalidate_node();
        true
    }

    pub(crate) fn open_folder_entry(
        &mut self,
        entry_index: usize,
        frame_index: u64,
        surface_size_px: [u32; 2],
    ) -> bool {
        let Some(snapshot) = self.cached_snapshot.clone() else {
            return false;
        };
        let Some(entry) = snapshot
            .entries
            .get(entry_index)
            .filter(|entry| entry.is_directory())
        else {
            return false;
        };
        self.current_path = normalize_catalog_path(&entry.logical_path);
        self.selected_index = 0;
        self.view_mode = CatalogViewMode::Grid;
        self.focus_scope = CatalogFocusScope::Grid;
        self.cached_snapshot = None;
        self.context_menu_open = false;
        self.invalidate_node();
        self.refresh_cache(frame_index, surface_size_px);
        newengine_ulog_api::ulog::info!(
            "asset browser UI: directory opened path='{}' via ui.dispatch_input_v1",
            display_path(&self.current_path)
        );
        true
    }

    pub(crate) fn select_asset_entry(
        &mut self,
        entry_index: usize,
        frame_index: u64,
        surface_size_px: [u32; 2],
    ) -> bool {
        let Some(snapshot) = self.cached_snapshot.clone() else {
            return false;
        };
        if entry_index >= snapshot.entries.len() {
            return false;
        }
        let was_selected = self.selected_index == entry_index;
        self.selected_index = entry_index;
        self.focus_scope = CatalogFocusScope::Grid;
        self.context_menu_open = false;
        self.cached_document_actions = self.document_actions_for_snapshot(&snapshot);
        self.invalidate_node();
        let Some(entry) = snapshot.entries.get(entry_index) else {
            return true;
        };
        if was_selected
            && self.open_asset_as_entry_directory(&entry.logical_path, frame_index, surface_size_px)
        {
            return true;
        }
        newengine_ulog_api::ulog::info!(
            "asset browser UI: selected asset path='{}' kind='{}' gateway='{}' via ui.dispatch_input_v1",
            entry.logical_path,
            entry.asset_kind,
            entry.semantic_gateway,
        );
        true
    }

    pub(crate) fn entry_index_from_dispatch_hit(&self, hit: &UiHitTestResult) -> Option<usize> {
        if hit.surface_id != ASSETS_CATALOG_SURFACE_ID {
            return None;
        }
        self.entry_index_from_node_id(&hit.node_id).or_else(|| {
            hit.action_id
                .as_deref()
                .and_then(|id| self.entry_index_from_action_id_and_node(id, &hit.node_id))
        })
    }

    pub(crate) fn entry_index_from_action(&self, action: &UiActionDispatch) -> Option<usize> {
        self.entry_index_from_node_id(&action.node_id).or_else(|| {
            self.entry_index_from_action_id_and_node(&action.action_id, &action.node_id)
        })
    }

    pub(crate) fn entry_index_from_action_id_and_node(
        &self,
        action_id: &str,
        node_id: &str,
    ) -> Option<usize> {
        match action_id {
            "asset_browser.details.inspect" => Some(self.selected_index),
            "asset_browser.asset.select"
            | "asset_browser.folder.open"
            | "asset_browser.sidebar.select" => self.entry_index_from_node_id(node_id),
            _ => None,
        }
    }

    pub(crate) fn entry_index_from_node_id(&self, node_id: &str) -> Option<usize> {
        if let Some(value) = node_id.strip_prefix("asset_browser.asset_card.") {
            return value.parse::<usize>().ok();
        }
        if let Some(value) = node_id.strip_prefix("asset_browser.folder_card.") {
            return value.parse::<usize>().ok();
        }
        if let Some(value) = node_id.strip_prefix("asset_browser.sidebar.folder.") {
            let ordinal = value.parse::<usize>().ok()?;
            return self.directory_entry_by_visible_ordinal(ordinal, 18);
        }
        None
    }

    pub(crate) fn directory_entry_by_visible_ordinal(
        &self,
        ordinal: usize,
        take: usize,
    ) -> Option<usize> {
        let snapshot = self.cached_snapshot.as_ref()?;
        filtered_entry_indices(
            snapshot,
            self.view_mode,
            &self.search_query,
            &self.collapsed_paths,
        )
        .into_iter()
        .filter(|entry_index| {
            snapshot
                .entries
                .get(*entry_index)
                .map(AssetsCatalogEntry::is_directory)
                .unwrap_or(false)
        })
        .take(take)
        .nth(ordinal)
    }

    pub(crate) fn open_asset_as_entry_directory(
        &mut self,
        asset_path: &str,
        frame_index: u64,
        surface_size_px: [u32; 2],
    ) -> bool {
        let normalized = normalize_catalog_path(asset_path);
        if normalized.is_empty() || normalized.contains('@') {
            return false;
        }
        match snapshot_from_list_file(&mut self.state, &normalized) {
            Ok(snapshot) => {
                self.current_path = normalized;
                self.selected_index = 0;
                self.view_mode = CatalogViewMode::Grid;
                self.focus_scope = CatalogFocusScope::Grid;
                self.cached_snapshot = Some(snapshot);
                self.cached_node = None;
                self.last_refresh_frame = 0;
                self.refresh_cache(frame_index, surface_size_px);
                newengine_ulog_api::ulog::info!(
                    "asset browser UI: opened NEF8/ListFile as entry directory path='{}'",
                    display_path(&self.current_path)
                );
                true
            }
            Err(error) => {
                newengine_ulog_api::ulog::debug!(
                    "asset browser UI: asset is not an entry directory path='{}' reason='{}'",
                    display_path(&normalized),
                    error
                );
                false
            }
        }
    }

    pub(crate) fn handle_text_input(&mut self, input: &UiInputFrame) -> bool {
        let mut changed = false;
        if self.focus_scope == CatalogFocusScope::Search {
            if input.is_key_pressed(key_code::BACKSPACE) {
                changed |= self.search_query.pop().is_some();
            }
            if input.is_key_pressed(key_code::ESCAPE) && !self.search_query.is_empty() {
                self.search_query.clear();
                changed = true;
            }
        }
        if !input.text.is_empty() {
            for ch in input.text.chars().filter(|ch| !ch.is_control()) {
                self.search_query.push(ch);
                changed = true;
            }
            self.focus_scope = CatalogFocusScope::Search;
        }
        if changed {
            if let Some(snapshot) = self.cached_snapshot.as_ref() {
                self.selected_index = filtered_entry_indices(
                    snapshot,
                    self.view_mode,
                    &self.search_query,
                    &self.collapsed_paths,
                )
                .first()
                .copied()
                .unwrap_or(0);
            } else {
                self.selected_index = 0;
            }
            self.invalidate_node();
        }
        changed
    }

    pub(crate) fn handle_navigation_input(
        &mut self,
        actions: &InputActionFrame,
        frame_index: u64,
        surface_size_px: [u32; 2],
    ) {
        let visible_indices = self
            .cached_snapshot
            .as_ref()
            .map(|snapshot| {
                filtered_entry_indices(
                    snapshot,
                    self.view_mode,
                    &self.search_query,
                    &self.collapsed_paths,
                )
            })
            .unwrap_or_default();
        let mut changed = false;

        if actions.ui_nav[0] < 0
            || action_frame_contains(actions, engine_action::UI_NAVIGATION_LEFT)
        {
            self.view_mode = self.view_mode.previous();
            self.focus_scope = match self.view_mode {
                CatalogViewMode::Tree => CatalogFocusScope::Tree,
                CatalogViewMode::List | CatalogViewMode::Grid => CatalogFocusScope::Grid,
                CatalogViewMode::Inspector => CatalogFocusScope::Inspector,
            };
            changed = true;
        }

        if actions.ui_nav[0] > 0
            || action_frame_contains(actions, engine_action::UI_NAVIGATION_RIGHT)
        {
            self.view_mode = self.view_mode.next();
            self.focus_scope = match self.view_mode {
                CatalogViewMode::Tree => CatalogFocusScope::Tree,
                CatalogViewMode::List | CatalogViewMode::Grid => CatalogFocusScope::Grid,
                CatalogViewMode::Inspector => CatalogFocusScope::Inspector,
            };
            changed = true;
        }

        if !visible_indices.is_empty() {
            let slot = visible_indices
                .iter()
                .position(|idx| *idx == self.selected_index)
                .unwrap_or(0);
            if actions.ui_nav[1] < 0
                || action_frame_contains(actions, engine_action::UI_NAVIGATION_UP)
            {
                self.selected_index = visible_indices[slot.saturating_sub(1)];
                self.focus_scope = CatalogFocusScope::Grid;
                changed = true;
            }
            if actions.ui_nav[1] > 0
                || action_frame_contains(actions, engine_action::UI_NAVIGATION_DOWN)
            {
                self.selected_index =
                    visible_indices[(slot + 1).min(visible_indices.len().saturating_sub(1))];
                self.focus_scope = CatalogFocusScope::Grid;
                changed = true;
            }
        }
        if actions.ui_back || action_frame_contains(actions, engine_action::UI_NAVIGATION_BACK) {
            if self.focus_scope == CatalogFocusScope::Search && !self.search_query.is_empty() {
                self.search_query.clear();
                changed = true;
            } else {
                let parent = parent_path(&self.current_path);
                if parent != self.current_path {
                    self.current_path = parent;
                    self.selected_index = 0;
                    self.view_mode = CatalogViewMode::Grid;
                    self.focus_scope = CatalogFocusScope::Breadcrumb;
                    self.cached_snapshot = None;
                    changed = true;
                    newengine_ulog_api::ulog::info!(
                        "asset browser UI: navigate parent path='{}'",
                        display_path(&self.current_path)
                    );
                } else {
                    self.view_mode = CatalogViewMode::Grid;
                    self.focus_scope = CatalogFocusScope::Grid;
                    changed = true;
                }
            }
        }
        if actions.ui_accept || action_frame_contains(actions, engine_action::UI_NAVIGATION_ACCEPT)
        {
            if let Some(entry) = self
                .cached_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.entries.get(self.selected_index))
                .cloned()
            {
                if entry.is_directory() {
                    self.current_path = normalize_catalog_path(&entry.logical_path);
                    self.selected_index = 0;
                    self.cached_snapshot = None;
                    self.view_mode = CatalogViewMode::Grid;
                    self.focus_scope = CatalogFocusScope::Grid;
                    changed = true;
                    newengine_ulog_api::ulog::info!(
                        "asset browser UI: open directory path='{}'",
                        display_path(&self.current_path)
                    );
                } else if self.open_asset_as_entry_directory(
                    &entry.logical_path,
                    frame_index,
                    surface_size_px,
                ) {
                    changed = false;
                } else {
                    self.view_mode = CatalogViewMode::Inspector;
                    self.focus_scope = CatalogFocusScope::Inspector;
                    changed = true;
                    newengine_ulog_api::ulog::info!(
                        "asset browser UI: selected asset path='{}' kind='{}' gateway='{}'",
                        entry.logical_path,
                        entry.asset_kind,
                        entry.semantic_gateway
                    );
                }
            }
        }

        if changed {
            self.invalidate_node();
            if self.cached_snapshot.is_none() {
                self.refresh_cache(frame_index, surface_size_px);
            }
        }
    }
}
