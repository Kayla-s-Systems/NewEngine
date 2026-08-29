use super::*;

impl AssetsCatalogUiRuntimeModule {
    pub fn new() -> Self {
        let host = newengine_plugin_host::default_host_api();
        Self::with_host(host)
    }

    #[inline]
    pub fn with_host(host: HostApiV1) -> Self {
        let client = AssetServiceClient::new(host.clone());
        Self {
            state: AssetsCatalogRuntimeState::new(client),
            open: false,
            current_path: String::new(),
            selected_index: 0,
            last_refresh_frame: 0,
            last_toggle_frame: u64::MAX,
            last_published_open: false,
            last_published_visible: false,
            last_pointer_frame: u64::MAX,
            input_registered: false,
            cached_snapshot: None,
            cached_node: None,
            view_mode: CatalogViewMode::Grid,
            search_query: String::new(),
            collapsed_paths: BTreeSet::new(),
            hovered_entry_index: None,
            focus_scope: CatalogFocusScope::Grid,
            cached_document_actions: Vec::new(),
            cached_document_action_ref: String::new(),
            cached_document_action_error: None,
            last_action_result: None,
            context_menu_open: false,
            main_scrollbar_dragging: false,
        }
    }

    pub(crate) fn publish_surface(&self, node: UiSurfaceNode) {
        let payload = match serde_json::to_vec(&node) {
            Ok(payload) => payload,
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "asset browser UI: surface serialization failed: {error}"
                );
                return;
            }
        };
        match newengine_core::call_service_v1_optional(
            ENGINE_UI_SERVICE_ID,
            UI_SERVICE_METHOD_SURFACE_NODE_V1,
            &payload,
        ) {
            Ok(Some(_)) => {}
            Ok(None) => {
                newengine_ulog_api::ulog::warn!(
                    "asset browser UI: engine.ui is unavailable; surface='{}' skipped instead of using a native/special renderer",
                    node.surface_id,
                );
            }
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "asset browser UI: engine.ui surface publish failed: {error}"
                );
            }
        }
    }

    pub(crate) fn invalidate_node(&mut self) {
        self.cached_node = None;
    }

    pub(crate) fn refresh_cache(&mut self, frame_index: u64, surface_size_px: [u32; 2]) {
        let snapshot_result = snapshot(&mut self.state, &self.current_path, self.selected_index);
        match snapshot_result {
            Ok(snapshot) => {
                if snapshot.entries.is_empty() {
                    self.selected_index = 0;
                } else if self.selected_index >= snapshot.entries.len() {
                    self.selected_index = snapshot.entries.len().saturating_sub(1);
                }
                self.cached_document_actions = self.document_actions_for_snapshot(&snapshot);
                let node = assets_catalog_node(
                    frame_index,
                    surface_size_px,
                    &snapshot,
                    self.selected_index,
                    self.hovered_entry_index,
                    self.view_mode,
                    &self.search_query,
                    &self.collapsed_paths,
                    self.focus_scope,
                    &self.cached_document_actions,
                    self.last_action_result.as_ref(),
                    self.context_menu_open,
                );
                self.cached_snapshot = Some(snapshot);
                self.cached_node = Some(node);
            }
            Err(error) => {
                self.cached_snapshot = None;
                self.cached_node = Some(assets_catalog_error_node(frame_index, error));
            }
        }
        self.last_refresh_frame = frame_index;
    }
}
