#![forbid(unsafe_op_in_unsafe_fn)]

//! Asset Browser retained UI projection over engine.assets data.
//!
//! This crate is deliberately not a backend domain, gateway or capability. It is
//! a product/profile UI composition module: it reads reusable backend data from
//! `engine.assets` and publishes a generic `UiSurfaceNode` through `engine.ui`.
//! Rendering remains owned by the selected `engine.ui` provider.

use newengine_assets_api::{
    AssetDecodeRequest, AssetDocumentAction, AssetFileManifest, AssetPatchResult, AssetService, AssetServiceClient,
    ASSET_LIST_FILE_MANIFEST_OUTPUT,
};
use newengine_core::{EngineResult, Module, ModuleCtx, Resources};
use newengine_core::host_events::WindowInitSize;
use newengine_core::lifecycle_events::EngineReadinessKey;
use newengine_input_actions_api::{
    engine_action, InputActionDefinition, InputActionDispatchMode, InputActionEffect,
    InputActionFrame, InputFrameSource,
};
use newengine_input_api::{engine_default_keybind, key_code, key_identity};
use newengine_input_bindings_api::{
    InputBinding, InputBindingRegistration, InputKeyRegistration,
};
use newengine_plugin_api::HostApiV1;
use std::collections::BTreeSet;

use newengine_ui_api::{
    ui_surface_node_layout, EditorSelectionContext, UiActionDispatch, UiComponentNode, UiEventDispatchFrame,
    UiHitTestResult, UiInputCaptureState, UiDockLayoutState, UiInputCaptureStateManager,
    UiInputFrame, UiNodeEventTrigger, UiNodeMessage, UiNodeMessageSeverity, UiNodeTone,
    UiScreenProfile, UiScreenProfileState, UiSurfaceAnchor, UiSurfaceNode, UiSurfaceStyle,
    ENGINE_UI_SERVICE_ID, UI_COMPONENT_ACTION, UI_COMPONENT_GRID, UI_COMPONENT_INPUT,
    UI_COMPONENT_LIST, UI_COMPONENT_PANEL, UI_COMPONENT_TREE, UI_FONT_ASSET_EDITOR_SANS,
    UI_THEME_ASSET_NORTHSTAR_EDITOR, UI_THEME_NORTHSTAR_EDITOR, UI_SERVICE_METHOD_SURFACE_NODE_V1,
};
use serde_json::{json, Value};

mod entry_presentation;
mod path;
mod pipeline_status;
mod value_helpers;

use entry_presentation::*;
use path::*;
use pipeline_status::*;
use value_helpers::*;

pub const ASSETS_CATALOG_UI_OWNER: &str = "app.asset_browser";
const ASSETS_CATALOG_SURFACE_ID: &str = "ui.assets.catalog";
const ASSETS_CATALOG_INPUT_LISTENER: &str = "asset-browser-ui";
const ASSETS_CATALOG_THEME_ID: &str = UI_THEME_NORTHSTAR_EDITOR;
pub(crate) const ASSET_BROWSER_ICON_FOLDER: &str = "ui/icons/assetBrowser.ytd@folder";
pub(crate) const ASSET_BROWSER_ICON_TEXTURE: &str = "ui/icons/assetBrowser.ytd@texture";
pub(crate) const ASSET_BROWSER_ICON_MATERIAL: &str = "ui/icons/assetBrowser.ytd@material";
pub(crate) const ASSET_BROWSER_ICON_MODEL: &str = "ui/icons/assetBrowser.ytd@model";
pub(crate) const ASSET_BROWSER_ICON_WORLD: &str = "ui/icons/assetBrowser.ytd@world";
pub(crate) const ASSET_BROWSER_ICON_UI: &str = "ui/icons/assetBrowser.ytd@ui";
pub(crate) const ASSET_BROWSER_ICON_PACKAGE: &str = "ui/icons/assetBrowser.ytd@package";
pub(crate) const ASSET_BROWSER_ICON_SCRIPT: &str = "ui/icons/assetBrowser.ytd@script";
pub(crate) const ASSET_BROWSER_ICON_SHADER: &str = "ui/icons/assetBrowser.ytd@shader";
pub(crate) const ASSET_BROWSER_ICON_AUDIO: &str = "ui/icons/assetBrowser.ytd@audio";
pub(crate) const ASSET_BROWSER_ICON_GENERIC: &str = "ui/icons/assetBrowser.ytd@generic";
pub(crate) const MAX_VISIBLE_ENTRIES: usize = 64;
const UI_SCROLLBAR_DRAG_ACTION: &str = "ui.scrollbar.drag";
const UI_SCROLL_WHEEL_ACTION: &str = "ui.scroll.wheel";
const DEFAULT_SURFACE_SIZE_PX: [u32; 2] = [1600, 900];

#[derive(Clone)]
pub struct AssetsCatalogRuntimeState {
    pub(crate) client: AssetServiceClient,
}

impl AssetsCatalogRuntimeState {
    #[inline]
    pub fn new(client: AssetServiceClient) -> Self {
        Self { client }
    }
}

/// Profile-owned UI projection over `engine.assets`.
///
/// This module does not register a service and does not extend the backend API.
/// If `engine.ui` is unavailable, it only emits a warning and skips drawing.
pub struct AssetsCatalogUiRuntimeModule {
    state: AssetsCatalogRuntimeState,
    open: bool,
    current_path: String,
    selected_index: usize,
    last_refresh_frame: u64,
    last_toggle_frame: u64,
    last_published_open: bool,
    last_published_visible: bool,
    last_pointer_frame: u64,
    input_registered: bool,
    cached_snapshot: Option<AssetsCatalogSnapshot>,
    cached_node: Option<UiSurfaceNode>,
    view_mode: CatalogViewMode,
    search_query: String,
    collapsed_paths: BTreeSet<String>,
    hovered_entry_index: Option<usize>,
    focus_scope: CatalogFocusScope,
    cached_document_actions: Vec<AssetDocumentAction>,
    cached_document_action_ref: String,
    cached_document_action_error: Option<String>,
    last_action_result: Option<AssetPatchResult>,
    context_menu_open: bool,
    main_scrollbar_dragging: bool,
}

impl AssetsCatalogUiRuntimeModule {
    #[inline]
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

    fn publish_surface(&self, node: UiSurfaceNode) {
        let payload = match serde_json::to_vec(&node) {
            Ok(payload) => payload,
            Err(error) => {
                log::warn!("asset browser UI: surface serialization failed: {error}");
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
                log::warn!(
                    "asset browser UI: engine.ui is unavailable; surface='{}' skipped instead of using a native/special renderer",
                    node.surface_id,
                );
            }
            Err(error) => {
                log::warn!("asset browser UI: engine.ui surface publish failed: {error}");
            }
        }
    }

    fn invalidate_node(&mut self) {
        self.cached_node = None;
    }

    fn refresh_cache(&mut self, frame_index: u64, surface_size_px: [u32; 2]) {
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

    fn publish_selected_asset_context(&self, resources: &mut Resources) {
        let Some(snapshot) = self.cached_snapshot.as_ref() else { return; };
        let Some(entry) = snapshot
            .entries
            .get(self.selected_index)
            .or_else(|| snapshot.entries.iter().find(|entry| !entry.is_directory()))
        else { return; };
        if entry.is_directory() {
            return;
        }
        let selection = EditorSelectionContext::asset(
            entry.logical_path.clone(),
            entry.name.clone(),
            ASSETS_CATALOG_SURFACE_ID,
            entry.semantic_gateway.clone(),
        );
        resources.insert(selection);
    }

    fn document_actions_for_snapshot(&mut self, snapshot: &AssetsCatalogSnapshot) -> Vec<AssetDocumentAction> {
        let Some(entry) = snapshot
            .entries
            .get(self.selected_index)
            .or_else(|| snapshot.entries.iter().find(|entry| !entry.is_directory()))
        else { return Vec::new(); };
        if entry.is_directory() {
            return Vec::new();
        }

        if self.cached_document_action_ref == entry.logical_path {
            return self.cached_document_actions.clone();
        }

        self.cached_document_action_ref = entry.logical_path.clone();
        match self.state.client.inspect_document_json_v1(newengine_assets_api::AssetDocumentRequest {
            asset_ref: entry.logical_path.clone(),
            requester: ASSETS_CATALOG_UI_OWNER.to_owned(),
            ..newengine_assets_api::AssetDocumentRequest::default()
        }) {
            Ok(document) => {
                self.cached_document_action_error = None;
                self.cached_document_actions = document.actions;
                self.cached_document_actions.clone()
            }
            Err(error) => {
                let should_log = self.cached_document_action_error.as_deref() != Some(error.as_str());
                if should_log {
                    log::warn!(
                        "asset browser UI: asset document actions unavailable path='{}' err='{}'",
                        entry.logical_path,
                        error,
                    );
                }
                self.cached_document_action_error = Some(error);
                self.cached_document_actions.clear();
                Vec::new()
            }
        }
    }

    fn dispatch_asset_document_action(&mut self, action_id: &str, frame_index: u64, surface_size_px: [u32; 2]) {
        let Some(action) = self.cached_document_actions.iter().find(|action| action.id == action_id).cloned() else {
            log::warn!("asset browser UI: unknown document action id='{}'", action_id);
            return;
        };

        if !action.enabled {
            self.last_action_result = Some(AssetPatchResult {
                accepted: false,
                written: false,
                dirty: false,
                diagnostics: vec![newengine_assets_api::AssetDocumentDiagnostic::warn(
                    "asset.action.disabled",
                    if action.disabled_reason.trim().is_empty() {
                        format!("action '{}' is disabled by provider policy", action.label)
                    } else {
                        action.disabled_reason.clone()
                    },
                )],
                ..AssetPatchResult::default()
            });
            self.invalidate_node();
            return;
        }

        if action.requires_input {
            self.last_action_result = Some(AssetPatchResult {
                accepted: false,
                written: false,
                dirty: false,
                diagnostics: vec![newengine_assets_api::AssetDocumentDiagnostic::info(
                    "asset.action.requires_input",
                    format!("action '{}' needs a schema/dialog payload before emitting AssetPatch", action.label),
                )],
                ..AssetPatchResult::default()
            });
            self.invalidate_node();
            return;
        }

        let Some(patch) = action.patch_template.clone() else {
            self.last_action_result = Some(AssetPatchResult {
                accepted: false,
                written: false,
                dirty: false,
                diagnostics: vec![newengine_assets_api::AssetDocumentDiagnostic::warn(
                    "asset.action.no_patch",
                    format!("action '{}' has no provider-declared AssetPatch template", action.label),
                )],
                ..AssetPatchResult::default()
            });
            self.invalidate_node();
            return;
        };

        match self.state.client.apply_patch_json_v1(patch) {
            Ok(result) => {
                log::info!(
                    "asset browser UI: document action dispatched id='{}' accepted={} written={}",
                    action.id,
                    result.accepted,
                    result.written,
                );
                self.last_action_result = Some(result);
                self.cached_snapshot = None;
                self.invalidate_node();
                self.refresh_cache(frame_index, surface_size_px);
            }
            Err(error) => {
                self.last_action_result = Some(AssetPatchResult {
                    accepted: false,
                    written: false,
                    dirty: true,
                    diagnostics: vec![newengine_assets_api::AssetDocumentDiagnostic::error(
                        "asset.action.dispatch_failed",
                        error,
                    )],
                    ..AssetPatchResult::default()
                });
                self.invalidate_node();
            }
        }
    }

    fn handle_ui_dispatch_frame(
        &mut self,
        dispatch: Option<&UiEventDispatchFrame>,
        _input: &UiInputFrame,
        surface_size_px: [u32; 2],
        frame_index: u64,
    ) {
        let Some(dispatch) = dispatch else { return; };
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
        for action in dispatch.actions.iter().filter(|action| action.surface_id == ASSETS_CATALOG_SURFACE_ID) {
            consumed |= self.handle_ui_action(action, surface_size_px, frame_index);
        }

        if self.main_scrollbar_dragging {
            if let Some(action) = dispatch
                .actions
                .iter()
                .find(|action| action.action_id == UI_SCROLLBAR_DRAG_ACTION && action.trigger == UiNodeEventTrigger::DragMove)
            {
                consumed |= self.handle_scrollbar_action(action);
            }
        }

        if !consumed
            && self.context_menu_open
            && dispatch.actions.iter().any(|action| matches!(action.trigger, UiNodeEventTrigger::Press | UiNodeEventTrigger::ContextMenu | UiNodeEventTrigger::Click))
            && hovered.is_none()
        {
            self.context_menu_open = false;
            self.invalidate_node();
        }
    }

    fn handle_ui_action(&mut self, action: &UiActionDispatch, surface_size_px: [u32; 2], frame_index: u64) -> bool {
        if self.last_pointer_frame == frame_index && matches!(action.trigger, UiNodeEventTrigger::Click | UiNodeEventTrigger::ContextMenu) {
            return false;
        }

        if action.action_id == UI_SCROLLBAR_DRAG_ACTION {
            if matches!(action.trigger, UiNodeEventTrigger::Press | UiNodeEventTrigger::DragStart) {
                self.main_scrollbar_dragging = true;
                return self.handle_scrollbar_action(action);
            }
            if action.trigger == UiNodeEventTrigger::DragMove {
                return self.handle_scrollbar_action(action);
            }
            if matches!(action.trigger, UiNodeEventTrigger::Release | UiNodeEventTrigger::DragEnd) {
                self.main_scrollbar_dragging = false;
                return true;
            }
            return false;
        }

        if action.action_id == UI_SCROLL_WHEEL_ACTION && action.trigger == UiNodeEventTrigger::ValueChanged {
            return action_payload_array_f32(action, "wheel", 1)
                .map(|wheel_y| self.select_by_wheel(wheel_y))
                .unwrap_or(false);
        }

        if action.trigger == UiNodeEventTrigger::ContextMenu {
            let Some(entry_index) = self.entry_index_from_action(action) else { return false; };
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
                let Some(snapshot) = self.cached_snapshot.as_ref() else { return false; };
                let path = breadcrumb_path_from_action(action, snapshot);
                self.current_path = normalize_catalog_path(&path);
                self.selected_index = 0;
                self.focus_scope = CatalogFocusScope::Breadcrumb;
                self.cached_snapshot = None;
                self.context_menu_open = false;
                self.invalidate_node();
                self.refresh_cache(frame_index, surface_size_px);
                log::info!("asset browser UI: breadcrumb open path='{}' via ui.dispatch_input_v1", display_path(&self.current_path));
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
                log::info!("asset browser UI: root opened via ui.dispatch_input_v1");
                true
            }
            "asset_browser.folder.open" | "asset_browser.sidebar.select" => {
                let Some(entry_index) = self.entry_index_from_action(action) else { return false; };
                self.open_folder_entry(entry_index, frame_index, surface_size_px)
            }
            "asset_browser.asset.select" | "asset_browser.details.inspect" => {
                let Some(entry_index) = self.entry_index_from_action(action) else { return false; };
                self.select_asset_entry(entry_index, frame_index, surface_size_px)
            }
            id if self.cached_document_actions.iter().any(|action| action.id == id) => {
                self.context_menu_open = false;
                self.dispatch_asset_document_action(id, frame_index, surface_size_px);
                true
            }
            _ => false,
        }
    }

    fn set_view_mode(&mut self, view_mode: CatalogViewMode) -> bool {
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

    fn select_by_wheel(&mut self, wheel_y: f32) -> bool {
        let Some(snapshot) = self.cached_snapshot.as_ref() else { return false; };
        let visible = filtered_entry_indices(snapshot, self.view_mode, &self.search_query, &self.collapsed_paths);
        if visible.is_empty() {
            return false;
        }
        let slot = visible.iter().position(|idx| *idx == self.selected_index).unwrap_or(0);
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

    fn handle_scrollbar_action(&mut self, action: &UiActionDispatch) -> bool {
        let Some(snapshot) = self.cached_snapshot.as_ref() else { return false; };
        let visible = filtered_entry_indices(snapshot, self.view_mode, &self.search_query, &self.collapsed_paths);
        if visible.is_empty() {
            return false;
        }
        let Some(local_y) = action_payload_f32(action, "local_pos", 1) else { return false; };
        let Some(height) = action_payload_f32(action, "global_rect", 3).filter(|height| *height > 0.0) else { return false; };
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

    fn open_folder_entry(&mut self, entry_index: usize, frame_index: u64, surface_size_px: [u32; 2]) -> bool {
        let Some(snapshot) = self.cached_snapshot.clone() else { return false; };
        let Some(entry) = snapshot.entries.get(entry_index).filter(|entry| entry.is_directory()) else { return false; };
        self.current_path = normalize_catalog_path(&entry.logical_path);
        self.selected_index = 0;
        self.view_mode = CatalogViewMode::Grid;
        self.focus_scope = CatalogFocusScope::Grid;
        self.cached_snapshot = None;
        self.context_menu_open = false;
        self.invalidate_node();
        self.refresh_cache(frame_index, surface_size_px);
        log::info!("asset browser UI: directory opened path='{}' via ui.dispatch_input_v1", display_path(&self.current_path));
        true
    }

    fn select_asset_entry(&mut self, entry_index: usize, frame_index: u64, surface_size_px: [u32; 2]) -> bool {
        let Some(snapshot) = self.cached_snapshot.clone() else { return false; };
        if entry_index >= snapshot.entries.len() {
            return false;
        }
        let was_selected = self.selected_index == entry_index;
        self.selected_index = entry_index;
        self.focus_scope = CatalogFocusScope::Grid;
        self.context_menu_open = false;
        self.cached_document_actions = self.document_actions_for_snapshot(&snapshot);
        self.invalidate_node();
        let Some(entry) = snapshot.entries.get(entry_index) else { return true; };
        if was_selected && self.open_asset_as_entry_directory(&entry.logical_path, frame_index, surface_size_px) {
            return true;
        }
        log::info!(
            "asset browser UI: selected asset path='{}' kind='{}' gateway='{}' via ui.dispatch_input_v1",
            entry.logical_path,
            entry.asset_kind,
            entry.semantic_gateway,
        );
        true
    }

    fn entry_index_from_dispatch_hit(&self, hit: &UiHitTestResult) -> Option<usize> {
        if hit.surface_id != ASSETS_CATALOG_SURFACE_ID {
            return None;
        }
        self.entry_index_from_node_id(&hit.node_id)
            .or_else(|| hit.action_id.as_deref().and_then(|id| self.entry_index_from_action_id_and_node(id, &hit.node_id)))
    }

    fn entry_index_from_action(&self, action: &UiActionDispatch) -> Option<usize> {
        self.entry_index_from_node_id(&action.node_id)
            .or_else(|| self.entry_index_from_action_id_and_node(&action.action_id, &action.node_id))
    }

    fn entry_index_from_action_id_and_node(&self, action_id: &str, node_id: &str) -> Option<usize> {
        match action_id {
            "asset_browser.details.inspect" => Some(self.selected_index),
            "asset_browser.asset.select" | "asset_browser.folder.open" | "asset_browser.sidebar.select" => self.entry_index_from_node_id(node_id),
            _ => None,
        }
    }

    fn entry_index_from_node_id(&self, node_id: &str) -> Option<usize> {
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

    fn directory_entry_by_visible_ordinal(&self, ordinal: usize, take: usize) -> Option<usize> {
        let snapshot = self.cached_snapshot.as_ref()?;
        filtered_entry_indices(snapshot, self.view_mode, &self.search_query, &self.collapsed_paths)
            .into_iter()
            .filter(|entry_index| snapshot.entries.get(*entry_index).map(AssetsCatalogEntry::is_directory).unwrap_or(false))
            .take(take)
            .nth(ordinal)
    }

    fn open_asset_as_entry_directory(&mut self, asset_path: &str, frame_index: u64, surface_size_px: [u32; 2]) -> bool {
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
                log::info!("asset browser UI: opened NEF8/ListFile as entry directory path='{}'", display_path(&self.current_path));
                true
            }
            Err(error) => {
                log::debug!("asset browser UI: asset is not an entry directory path='{}' reason='{}'", display_path(&normalized), error);
                false
            }
        }
    }

    fn handle_text_input(&mut self, input: &UiInputFrame) -> bool {
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
                self.selected_index = filtered_entry_indices(snapshot, self.view_mode, &self.search_query, &self.collapsed_paths)
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

    fn handle_navigation_input(&mut self, actions: &InputActionFrame, frame_index: u64, surface_size_px: [u32; 2]) {
        let visible_indices = self
            .cached_snapshot
            .as_ref()
            .map(|snapshot| filtered_entry_indices(snapshot, self.view_mode, &self.search_query, &self.collapsed_paths))
            .unwrap_or_default();
        let mut changed = false;

        if actions.ui_nav[0] < 0 || action_frame_contains(actions, engine_action::UI_NAVIGATION_LEFT) {
            self.view_mode = self.view_mode.previous();
            self.focus_scope = match self.view_mode {
                CatalogViewMode::Tree => CatalogFocusScope::Tree,
                CatalogViewMode::List | CatalogViewMode::Grid => CatalogFocusScope::Grid,
                CatalogViewMode::Inspector => CatalogFocusScope::Inspector,
            };
            changed = true;
        }

        if actions.ui_nav[0] > 0 || action_frame_contains(actions, engine_action::UI_NAVIGATION_RIGHT) {
            self.view_mode = self.view_mode.next();
            self.focus_scope = match self.view_mode {
                CatalogViewMode::Tree => CatalogFocusScope::Tree,
                CatalogViewMode::List | CatalogViewMode::Grid => CatalogFocusScope::Grid,
                CatalogViewMode::Inspector => CatalogFocusScope::Inspector,
            };
            changed = true;
        }

        if !visible_indices.is_empty() {
            let slot = visible_indices.iter().position(|idx| *idx == self.selected_index).unwrap_or(0);
            if actions.ui_nav[1] < 0 || action_frame_contains(actions, engine_action::UI_NAVIGATION_UP) {
                self.selected_index = visible_indices[slot.saturating_sub(1)];
                self.focus_scope = CatalogFocusScope::Grid;
                changed = true;
            }
            if actions.ui_nav[1] > 0 || action_frame_contains(actions, engine_action::UI_NAVIGATION_DOWN) {
                self.selected_index = visible_indices[(slot + 1).min(visible_indices.len().saturating_sub(1))];
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
                    log::info!("asset browser UI: navigate parent path='{}'", display_path(&self.current_path));
                } else {
                    self.view_mode = CatalogViewMode::Grid;
                    self.focus_scope = CatalogFocusScope::Grid;
                    changed = true;
                }
            }
        }
        if actions.ui_accept || action_frame_contains(actions, engine_action::UI_NAVIGATION_ACCEPT) {
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
                    log::info!("asset browser UI: open directory path='{}'", display_path(&self.current_path));
                } else if self.open_asset_as_entry_directory(&entry.logical_path, frame_index, surface_size_px) {
                    changed = false;
                } else {
                    self.view_mode = CatalogViewMode::Inspector;
                    self.focus_scope = CatalogFocusScope::Inspector;
                    changed = true;
                    log::info!(
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

impl Default for AssetsCatalogUiRuntimeModule {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Send + 'static> Module<E> for AssetsCatalogUiRuntimeModule {
    fn id(&self) -> &'static str {
        "app.asset_browser.ui_node"
    }

    fn startup_requires(&self) -> &'static [EngineReadinessKey] {
        const REQUIRES: &[EngineReadinessKey] = &[EngineReadinessKey::EnginePluginsReady];
        REQUIRES
    }

    fn start(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        self.input_registered = ensure_assets_catalog_input_registration();
        if !self.input_registered {
            log::warn!(
                "asset browser UI: semantic input listener registration incomplete; will continue through engine.input snapshot but F1 may be unavailable"
            );
        }
        Ok(())
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let frame_index = ctx.frame().map(|frame| frame.frame_index).unwrap_or(0);

        let input = ctx.resources().get::<UiInputFrame>().cloned().unwrap_or_default();
        let actions = resolve_actions(&input);
        let surface_size_px = ctx.resources()
            .get::<WindowInitSize>()
            .map(|size| [size.width.max(1), size.height.max(1)])
            .unwrap_or(DEFAULT_SURFACE_SIZE_PX);
        let toggled = action_frame_contains(&actions, engine_action::ASSET_CATALOG_UI_TOGGLE);
        let editor_profile_active = is_editor_screen_profile(ctx.resources());

        if toggled && self.last_toggle_frame != frame_index {
            self.last_toggle_frame = frame_index;
            if editor_profile_active {
                log::info!(
                    "asset browser UI: toggle consumed by editor dock surface; profile='editor' visible=true modal=false"
                );
            } else {
                self.open = !self.open;
                self.cached_node = None;
                if self.open && self.cached_snapshot.is_none() {
                    self.current_path.clear();
                    self.selected_index = 0;
                }
                log::info!("asset browser UI: visibility changed open={}", self.open);
            }
        }

        let docked_browser_visible = ctx
            .resources()
            .get::<UiDockLayoutState>()
            .map(|layout| layout.panel_visible("bottom.content_browser"))
            .unwrap_or(true);
        let visible = (editor_profile_active && docked_browser_visible) || self.open;
        if visible {
            let stale = frame_index.saturating_sub(self.last_refresh_frame) >= 30;
            if stale || self.cached_node.is_none() || self.last_toggle_frame == frame_index {
                self.refresh_cache(frame_index, surface_size_px);
            }
            let dispatch_frame = ctx.resources().get::<UiEventDispatchFrame>().cloned();
            self.handle_text_input(&input);
            self.handle_ui_dispatch_frame(dispatch_frame.as_ref(), &input, surface_size_px, frame_index);
            self.handle_navigation_input(&actions, frame_index, surface_size_px);
            if self.cached_node.is_none() {
                self.refresh_cache(frame_index, surface_size_px);
            }
            self.publish_selected_asset_context(ctx.resources_mut());
            if let Some(node) = self.cached_node.clone() {
                self.publish_surface(node);
            }
            if editor_profile_active {
                // In Editor profile the Content Browser is a docked editor panel.
                // The global screen profile capture already gates gameplay input;
                // the browser must not become a second modal owner and fight the editor shell.
                remove_input_capture_contribution(ctx.resources_mut(), ASSETS_CATALOG_UI_OWNER, None);
            } else {
                set_input_capture_contribution(
                    ctx.resources_mut(),
                    ASSETS_CATALOG_UI_OWNER,
                    UiInputCaptureState::modal(
                        ASSETS_CATALOG_SURFACE_ID,
                        "asset browser UI modal capture",
                    ),
                );
            }
        } else if self.last_published_visible || self.last_toggle_frame == frame_index {
            self.publish_surface(UiSurfaceNode::hidden(
                ASSETS_CATALOG_SURFACE_ID,
                ASSETS_CATALOG_UI_OWNER,
            ));
            remove_input_capture_contribution(
                ctx.resources_mut(),
                ASSETS_CATALOG_UI_OWNER,
                Some(ASSETS_CATALOG_SURFACE_ID),
            );
        } else {
            remove_input_capture_contribution(ctx.resources_mut(), ASSETS_CATALOG_UI_OWNER, None);
        }

        self.last_published_open = self.open;
        self.last_published_visible = visible;
        Ok(())
    }
}

fn action_payload_f32(action: &UiActionDispatch, field: &str, index: usize) -> Option<f32> {
    action_payload_array_f32(action, field, index)
}

fn action_payload_array_f32(action: &UiActionDispatch, field: &str, index: usize) -> Option<f32> {
    action.payload.get(field)?.as_array()?.get(index)?.as_f64().map(|value| value as f32)
}

fn dispatch_wheel_y(dispatch: &UiEventDispatchFrame, surface_id: &str) -> Option<f32> {
    dispatch
        .actions
        .iter()
        .find(|action| {
            action.surface_id == surface_id
                && action.trigger == UiNodeEventTrigger::ValueChanged
                && action.action_id == UI_SCROLL_WHEEL_ACTION
        })
        .and_then(|action| action_payload_array_f32(action, "wheel", 1))
}

fn breadcrumb_path_from_action(action: &UiActionDispatch, snapshot: &AssetsCatalogSnapshot) -> String {
    let Some(local_x) = action_payload_f32(action, "local_pos", 0) else {
        return parent_path(&snapshot.logical_path);
    };
    let Some(width) = action_payload_f32(action, "global_rect", 2).filter(|width| *width > 0.0) else {
        return parent_path(&snapshot.logical_path);
    };
    hit_breadcrumb_path(snapshot, local_x, 0.0, width)
}

fn is_editor_screen_profile(resources: &Resources) -> bool {
    resources
        .get::<UiScreenProfileState>()
        .map(|state| state.descriptor.profile == UiScreenProfile::Editor)
        .unwrap_or(false)
}

fn set_input_capture_contribution(resources: &mut Resources, owner: &str, capture: UiInputCaptureState) {
    let mut manager = resources.remove::<UiInputCaptureStateManager>().unwrap_or_default();
    manager.add_capture(owner.to_owned(), capture);
    let resolved = manager.resolve_final_capture();
    resources.insert(manager);
    resources.insert(resolved);
}

fn remove_input_capture_contribution(resources: &mut Resources, owner: &str, refresh_surface: Option<&str>) {
    let mut manager = resources.remove::<UiInputCaptureStateManager>().unwrap_or_default();
    manager.remove_capture(owner);
    let mut resolved = manager.resolve_final_capture();
    if let Some(surface) = refresh_surface {
        resolved.draw_refresh_requested = true;
        if !resolved.surfaces.iter().any(|it| it == surface) {
            resolved.surfaces.push(surface.to_owned());
        }
    }
    resources.insert(manager);
    resources.insert(resolved);
}

struct UiInputSource<'a>(&'a UiInputFrame);

impl InputFrameSource for UiInputSource<'_> {
    #[inline]
    fn is_key_down(&self, key: u32) -> bool { self.0.keys_down.contains(&key) }
    #[inline]
    fn is_key_pressed(&self, key: u32) -> bool { self.0.keys_pressed.contains(&key) }
    #[inline]
    fn is_key_released(&self, key: u32) -> bool { self.0.keys_released.contains(&key) }
    #[inline]
    fn is_mouse_down(&self, button: u32) -> bool { self.0.mouse_down.contains(&button) }
    #[inline]
    fn is_mouse_pressed(&self, button: u32) -> bool { self.0.mouse_pressed.contains(&button) }
    #[inline]
    fn is_mouse_released(&self, button: u32) -> bool { self.0.mouse_released.contains(&button) }
    #[inline]
    fn has_gamepad_connected(&self) -> bool { self.0.gamepad_connected > 0 }
    #[inline]
    fn is_gamepad_button_down(&self, button: &str) -> bool { self.0.is_gamepad_button_down(button) }
    #[inline]
    fn is_gamepad_button_pressed(&self, button: &str) -> bool { self.0.gamepad_buttons_pressed.contains(button) }
    #[inline]
    fn is_gamepad_button_released(&self, button: &str) -> bool { self.0.gamepad_buttons_released.contains(button) }
    #[inline]
    fn gamepad_axis(&self, axis: &str) -> f32 { self.0.gamepad_axes.get(axis).copied().unwrap_or(0.0) }
}

fn resolve_actions(input: &UiInputFrame) -> InputActionFrame {
    newengine_input_bindings_runtime::resolve_input_actions(&UiInputSource(input))
}

fn action_frame_contains(actions: &InputActionFrame, action: &str) -> bool {
    actions.actions.iter().any(|it| it == action)
        || actions.events.iter().any(|event| event.action == action)
}

fn ensure_assets_catalog_input_registration() -> bool {
    let mut ok = true;
    for (code, identity, label) in [
        (engine_default_keybind::ASSET_CATALOG_UI_TOGGLE, key_identity::F1, "F1"),
        (key_code::ARROW_UP, key_identity::ARROW_UP, "Arrow Up"),
        (key_code::ARROW_DOWN, key_identity::ARROW_DOWN, "Arrow Down"),
        (key_code::ARROW_LEFT, key_identity::ARROW_LEFT, "Arrow Left"),
        (key_code::ARROW_RIGHT, key_identity::ARROW_RIGHT, "Arrow Right"),
        (key_code::ENTER, key_identity::ENTER, "Enter"),
        (key_code::BACKSPACE, key_identity::BACKSPACE, "Backspace"),
    ] {
        if let Err(error) = newengine_input_bindings_runtime::register_input_key(
            InputKeyRegistration::new(code, identity, label),
        ) {
            log::warn!("asset browser UI: key registration failed key='{label}': {error}");
            ok = false;
        }
    }

    for action in [
        InputActionDefinition::new(engine_action::ASSET_CATALOG_UI_TOGGLE)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Toggle Asset Browser"),
        InputActionDefinition::new(engine_action::UI_NAVIGATION_ACCEPT)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Asset catalog accept")
            .with_effect(InputActionEffect::UiAccept),
        InputActionDefinition::new(engine_action::UI_NAVIGATION_BACK)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Asset catalog back")
            .with_effect(InputActionEffect::UiBack),
        InputActionDefinition::new(engine_action::UI_NAVIGATION_UP)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Asset catalog up")
            .with_effect(InputActionEffect::UiNav { x: 0, y: -1 }),
        InputActionDefinition::new(engine_action::UI_NAVIGATION_DOWN)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Asset catalog down")
            .with_effect(InputActionEffect::UiNav { x: 0, y: 1 }),
        InputActionDefinition::new(engine_action::UI_NAVIGATION_LEFT)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Asset catalog previous view")
            .with_effect(InputActionEffect::UiNav { x: -1, y: 0 }),
        InputActionDefinition::new(engine_action::UI_NAVIGATION_RIGHT)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Asset catalog next view")
            .with_effect(InputActionEffect::UiNav { x: 1, y: 0 }),
    ] {
        if let Err(error) = newengine_input_bindings_runtime::register_input_action(action) {
            log::warn!("asset browser UI: action registration failed: {error}");
            ok = false;
        }
    }

    for registration in [
        InputBindingRegistration::new(InputBinding::keyboard_pressed(
            engine_action::ASSET_CATALOG_UI_TOGGLE,
            engine_default_keybind::ASSET_CATALOG_UI_TOGGLE,
        )),
        InputBindingRegistration::new(InputBinding::keyboard_pressed(engine_action::UI_NAVIGATION_ACCEPT, key_code::ENTER)),
        InputBindingRegistration::new(InputBinding::keyboard_pressed(engine_action::UI_NAVIGATION_BACK, key_code::BACKSPACE)),
        InputBindingRegistration::new(InputBinding::keyboard_pressed(engine_action::UI_NAVIGATION_UP, key_code::ARROW_UP)),
        InputBindingRegistration::new(InputBinding::keyboard_pressed(engine_action::UI_NAVIGATION_DOWN, key_code::ARROW_DOWN)),
        InputBindingRegistration::new(InputBinding::keyboard_pressed(engine_action::UI_NAVIGATION_LEFT, key_code::ARROW_LEFT)),
        InputBindingRegistration::new(InputBinding::keyboard_pressed(engine_action::UI_NAVIGATION_RIGHT, key_code::ARROW_RIGHT)),
    ] {
        if let Err(error) = newengine_input_bindings_runtime::register_input_binding(registration) {
            log::warn!("asset browser UI: binding registration failed: {error}");
            ok = false;
        }
    }

    if let Err(error) = newengine_input_bindings_runtime::register_input_listener(
        newengine_input_actions_api::InputActionListenerRegistration::new(
            ASSETS_CATALOG_UI_OWNER,
            ASSETS_CATALOG_INPUT_LISTENER,
        )
        .with_actions([engine_action::ASSET_CATALOG_UI_TOGGLE])
        .with_priority(110)
        .consuming(),
    ) {
        log::warn!("asset browser UI: toggle listener registration failed: {error}");
        ok = false;
    }

    if let Err(error) = newengine_input_bindings_runtime::register_input_listener(
        newengine_input_actions_api::InputActionListenerRegistration::new(
            ASSETS_CATALOG_UI_OWNER,
            "assets-browser-navigation",
        )
        .with_actions([
            engine_action::UI_NAVIGATION_ACCEPT,
            engine_action::UI_NAVIGATION_BACK,
            engine_action::UI_NAVIGATION_UP,
            engine_action::UI_NAVIGATION_DOWN,
            engine_action::UI_NAVIGATION_LEFT,
            engine_action::UI_NAVIGATION_RIGHT,
        ])
        .with_priority(110),
    ) {
        log::warn!("asset browser UI: navigation listener registration failed: {error}");
        ok = false;
    }

    if ok {
        log::info!(
            "asset browser UI: input listeners registered owner='{}' toggle_listener='{}' nav_listener='assets-browser-navigation'",
            ASSETS_CATALOG_UI_OWNER,
            ASSETS_CATALOG_INPUT_LISTENER,
        );
    }
    ok
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CatalogToolbarAction {
    Refresh,
    Tree,
    List,
    Grid,
}

impl CatalogToolbarAction {
    fn label(self) -> &'static str {
        match self {
            Self::Refresh => "Refresh",
            Self::Tree => "Tree",
            Self::List => "List",
            Self::Grid => "Grid",
        }
    }
}

#[derive(Clone, Debug)]
enum CatalogToolbarItem {
    DocumentAction { id: String, label: String, enabled: bool },
    ViewAction { action: CatalogToolbarAction, label: &'static str },
}

fn catalog_toolbar_items(document_actions: &[AssetDocumentAction]) -> Vec<CatalogToolbarItem> {
    let mut items = document_actions
        .iter()
        .map(|action| CatalogToolbarItem::DocumentAction {
            id: action.id.clone(),
            label: action.label.clone(),
            enabled: action.enabled,
        })
        .collect::<Vec<_>>();
    items.extend([
        CatalogToolbarItem::ViewAction { action: CatalogToolbarAction::Tree, label: CatalogToolbarAction::Tree.label() },
        CatalogToolbarItem::ViewAction { action: CatalogToolbarAction::List, label: CatalogToolbarAction::List.label() },
        CatalogToolbarItem::ViewAction { action: CatalogToolbarAction::Grid, label: CatalogToolbarAction::Grid.label() },
        CatalogToolbarItem::ViewAction { action: CatalogToolbarAction::Refresh, label: CatalogToolbarAction::Refresh.label() },
    ]);
    items
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CatalogFocusScope {
    Tree,
    Breadcrumb,
    Search,
    Grid,
    Inspector,
}

impl CatalogFocusScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::Breadcrumb => "breadcrumb",
            Self::Search => "search",
            Self::Grid => "grid",
            Self::Inspector => "inspector",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct CatalogWorkspaceGeometry {
    panel_x: f32,
    panel_y: f32,
    panel_w: f32,
    panel_h: f32,
    sidebar_x: f32,
    sidebar_w: f32,
    main_x: f32,
    main_w: f32,
    details_x: f32,
    details_w: f32,
    content_top: f32,
    content_h: f32,
    tab_h: f32,
    toolbar_h: f32,
}

fn catalog_workspace_geometry(surface_size_px: [u32; 2]) -> CatalogWorkspaceGeometry {
    let style = assets_catalog_surface_style();
    let style_tags = vec![
        "asset-catalog".to_owned(),
        "docked-panel".to_owned(),
        "dock-bottom".to_owned(),
        "engine-ui-node".to_owned(),
    ];
    let layout = ui_surface_node_layout(surface_size_px, &style_tags, &style, 5, 2);
    let panel_x = layout.panel_x;
    let panel_y = layout.panel_y;
    let panel_w = layout.panel_w;
    let panel_h = layout.panel_h;
    let tab_h = 34.0;
    let toolbar_h = 40.0;
    let breadcrumb_h = 34.0;
    let inner_gap = 8.0;
    let content_top = panel_y + tab_h + toolbar_h + breadcrumb_h + inner_gap;
    let content_bottom = panel_y + panel_h - 30.0;
    let content_h = (content_bottom - content_top).max(96.0);
    let sidebar_w = (panel_w * 0.18).clamp(210.0, 286.0);
    let details_w = (panel_w * 0.20).clamp(240.0, 330.0);
    let sidebar_x = panel_x + inner_gap;
    let details_x = panel_x + panel_w - details_w - inner_gap;
    let main_x = sidebar_x + sidebar_w + inner_gap;
    let main_w = (details_x - main_x - inner_gap).max(320.0);
    CatalogWorkspaceGeometry { panel_x, panel_y, panel_w, panel_h, sidebar_x, sidebar_w, main_x, main_w, details_x, details_w, content_top, content_h, tab_h, toolbar_h }
}

fn push_catalog_regions(components: &mut Vec<UiComponentNode>, geometry: &CatalogWorkspaceGeometry) {
    components.push(catalog_region("asset_browser.region.tabs", geometry.panel_x, geometry.panel_y, geometry.panel_w, geometry.tab_h, [7, 12, 19, 252]));
    components.push(catalog_region("asset_browser.region.toolbar", geometry.panel_x, geometry.panel_y + geometry.tab_h, geometry.panel_w, geometry.toolbar_h, [9, 14, 22, 252]));
    components.push(catalog_region("asset_browser.region.breadcrumb", geometry.panel_x, geometry.panel_y + geometry.tab_h + geometry.toolbar_h, geometry.panel_w, 34.0, [7, 11, 18, 252]));
    components.push(catalog_region("asset_browser.region.sidebar", geometry.sidebar_x, geometry.content_top, geometry.sidebar_w, geometry.content_h, [8, 13, 20, 248]));
    components.push(catalog_region("asset_browser.region.main", geometry.main_x, geometry.content_top, geometry.main_w, geometry.content_h, [5, 9, 15, 248]));
    components.push(catalog_region("asset_browser.region.details", geometry.details_x, geometry.content_top, geometry.details_w, geometry.content_h, [8, 13, 20, 248]));
    components.push(catalog_region("asset_browser.region.status", geometry.panel_x, geometry.panel_y + geometry.panel_h - 30.0, geometry.panel_w, 30.0, [7, 11, 18, 252]));
}

fn catalog_region(id: &str, x: f32, y: f32, w: f32, h: f32, fill: [u8; 4]) -> UiComponentNode {
    let mut component = UiComponentNode::row(id, "")
        .tagged("region")
        .tagged("panel-region")
        .tagged("asset-browser-region");
    component.component_id = UI_COMPONENT_PANEL.to_owned();
    component.props.insert("interactive".to_owned(), json!(false));
    component.props.insert("draw_panel".to_owned(), json!(true));
    component.props.insert("fill_rgba".to_owned(), json!(fill));
    component.props.insert("border_rgba".to_owned(), json!([54, 70, 92, 150]));
    component.props.insert("radius_px".to_owned(), json!(if h <= 40.0 { 0.0 } else { 7.0 }));
    set_component_rect(&mut component, x, y, w, h);
    component
}

fn apply_catalog_component_layout(components: &mut [UiComponentNode], geometry: &CatalogWorkspaceGeometry) {
    let mut tab_x = geometry.panel_x + 14.0;
    let mut action_x = geometry.panel_x + 300.0;
    let mut sidebar_y = geometry.content_top;
    let mut details_y = geometry.content_top;
    let mut status_y = geometry.panel_y + geometry.panel_h - 34.0;
    let mut context_y = geometry.content_top + 42.0;

    for component in components.iter_mut() {
        let id = component.id.as_str();
        if id.starts_with("asset_browser.tab.") {
            set_component_rect(component, tab_x, geometry.panel_y + 6.0, 104.0, 26.0);
            tab_x += 112.0;
        } else if id == "asset_browser.toolbar" {
            set_component_rect(component, geometry.panel_x + 14.0, geometry.panel_y + geometry.tab_h + 6.0, 270.0, 28.0);
        } else if id.starts_with("asset_browser.action.") {
            set_component_rect(component, action_x, geometry.panel_y + geometry.tab_h + 6.0, 118.0, 28.0);
            action_x += 124.0;
        } else if id == "asset_browser.breadcrumb" {
            set_component_rect(component, geometry.panel_x + 14.0, geometry.panel_y + geometry.tab_h + geometry.toolbar_h + 6.0, geometry.panel_w * 0.55, 28.0);
        } else if id == "asset_browser.search" {
            let x = geometry.panel_x + geometry.panel_w * 0.62;
            set_component_rect(component, x, geometry.panel_y + geometry.tab_h + geometry.toolbar_h + 6.0, (geometry.panel_x + geometry.panel_w - x - 14.0).max(180.0), 28.0);
        } else if id.starts_with("asset_browser.sidebar.") {
            set_component_rect(component, geometry.sidebar_x, sidebar_y, geometry.sidebar_w, 24.0);
            sidebar_y += 26.0;
        } else if id == "asset_browser.main_scroll" {
            component.props.insert("h_px".to_owned(), json!(geometry.content_h));
            component.props.insert("w_px".to_owned(), json!(geometry.main_w));
            set_component_rect(component, geometry.main_x, geometry.content_top, geometry.main_w, geometry.content_h);
        } else if id.starts_with("asset_browser.details.") || id == "asset_browser.selection.bridge" {
            set_component_rect(component, geometry.details_x, details_y, geometry.details_w, 24.0);
            details_y += 27.0;
        } else if id.starts_with("asset_browser.context_menu") {
            let w = 240.0_f32.min(geometry.main_w.max(160.0));
            set_component_rect(component, (geometry.details_x - w - 10.0).max(geometry.main_x), context_y, w, 28.0);
            context_y += 31.0;
        } else if id == "asset_browser.action_result" || id == "asset_browser.status" || id.starts_with("asset_browser.warning.") {
            set_component_rect(component, geometry.panel_x + 14.0, status_y, geometry.panel_w - 28.0, 24.0);
            status_y += 26.0;
        }
    }

    components.sort_by(|a, b| {
        let ay = component_rect_number(a, "y_px").unwrap_or(f32::MAX);
        let by = component_rect_number(b, "y_px").unwrap_or(f32::MAX);
        let ax = component_rect_number(a, "x_px").unwrap_or(f32::MAX);
        let bx = component_rect_number(b, "x_px").unwrap_or(f32::MAX);
        ay.partial_cmp(&by)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| component_paint_rank(a).cmp(&component_paint_rank(b)))
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn component_paint_rank(component: &UiComponentNode) -> u8 {
    if component.state_tags.iter().any(|tag| tag == "region" || tag == "panel-region") { 0 } else { 1 }
}

fn set_component_rect(component: &mut UiComponentNode, x: f32, y: f32, w: f32, h: f32) {
    component.props.insert("x_px".to_owned(), json!(x.max(0.0)));
    component.props.insert("y_px".to_owned(), json!(y.max(0.0)));
    component.props.insert("w_px".to_owned(), json!(w.max(1.0)));
    component.props.insert("h_px".to_owned(), json!(h.max(1.0)));
}

fn component_rect_number(component: &UiComponentNode, key: &str) -> Option<f32> {
    component.props.get(key).and_then(|value| value.as_f64()).map(|value| value as f32)
}

fn hit_breadcrumb_path(snapshot: &AssetsCatalogSnapshot, mx: f32, start_x: f32, max_w: f32) -> String {
    let normalized = normalize_catalog_path(&snapshot.logical_path);
    if normalized.is_empty() {
        return String::new();
    }
    let mut x = start_x;
    let mut path = String::new();
    for segment in normalized.split('/') {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(segment);
        let w = (segment.chars().count() as f32 * 8.0 + 24.0).clamp(34.0, 160.0);
        if mx >= x && mx <= x + w && x - start_x < max_w {
            return path.clone();
        }
        x += w + 6.0;
    }
    normalized
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CatalogViewMode {
    Tree,
    List,
    Grid,
    Inspector,
}

impl CatalogViewMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::List => "list",
            Self::Grid => "grid",
            Self::Inspector => "inspector",
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Tree => Self::Inspector,
            Self::List => Self::Tree,
            Self::Grid => Self::List,
            Self::Inspector => Self::Grid,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Tree => Self::List,
            Self::List => Self::Grid,
            Self::Grid => Self::Inspector,
            Self::Inspector => Self::Tree,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct AssetsCatalogSnapshot {
    pub(crate) logical_path: String,
    entries: Vec<AssetsCatalogEntry>,
    sources: Vec<String>,
    formats: Vec<String>,
    warnings: Vec<String>,
    import_summary: String,
    import_queue_summary: String,
    package_writer_summary: String,
    route_diagnostics: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AssetsCatalogEntry {
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) logical_path: String,
    pub(crate) extension: String,
    pub(crate) semantic_gateway: String,
    pub(crate) asset_kind: String,
    pub(crate) import_stage: String,
    pub(crate) import_action: String,
    pub(crate) dirty: bool,
    pub(crate) uid: String,
    pub(crate) thumbnail: String,
}

impl AssetsCatalogEntry {
    pub(crate) fn is_directory(&self) -> bool {
        let kind = self.kind.trim().to_ascii_lowercase();
        kind == "directory" || kind == "dir" || kind == "folder" || kind == "mount"
    }
}

fn snapshot(state: &mut AssetsCatalogRuntimeState, logical_path: &str, _selected_index: usize) -> Result<AssetsCatalogSnapshot, String> {
    let logical_path = normalize_catalog_path(logical_path);
    let listing = match state.client.vfs_list_json_v1(&logical_path) {
        Ok(listing) => listing,
        Err(vfs_error) => {
            return snapshot_from_list_file(state, &logical_path).map_err(|entry_error| {
                format!("engine.assets catalog path unavailable: vfs='{vfs_error}' listFile='{entry_error}'")
            });
        }
    };
    let mut warnings = value_warnings(&listing);
    let mut entries = listing
        .get("entries")
        .and_then(|value| value.as_array())
        .map(|items| items.iter().map(entry_from_vfs_value).collect::<Vec<_>>())
        .unwrap_or_default();
    entries.sort_by(|a, b| {
        b.is_directory()
            .cmp(&a.is_directory())
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
    });

    apply_import_lifecycle_rows(state, &logical_path, &mut entries, &mut warnings);
    hydrate_preview_plans_for_entries(state, &mut entries, &mut warnings);

    let sources = match state.client.sources_json_v1() {
        Ok(value) => source_labels(&value),
        Err(error) => {
            warnings.push(format!("engine.assets sources unavailable: {error}"));
            Vec::new()
        }
    };
    let formats = match state.client.formats_json_v1() {
        Ok(value) => format_labels(&value),
        Err(error) => {
            warnings.push(format!("engine.assets formats unavailable: {error}"));
            Vec::new()
        }
    };

    let package_writer_summary = package_writer_summary(state).unwrap_or_else(|error| {
        warnings.push(format!("engine.assets.package_writer unavailable: {error}"));
        "package writer unavailable".to_owned()
    });
    let import_queue_summary = import_queue_summary(state).unwrap_or_else(|error| {
        warnings.push(format!("engine.assets.import_queue unavailable: {error}"));
        "import queue unavailable".to_owned()
    });
    let import_summary = import_summary_for_entries(&entries);
    let route_diagnostics = "routes: engine.assets · engine.assets.types · engine.assets.inspect · engine.assets.edit · engine.ui surface node".to_owned();

    Ok(AssetsCatalogSnapshot {
        logical_path,
        entries,
        sources,
        formats,
        warnings,
        import_summary,
        import_queue_summary,
        package_writer_summary,
        route_diagnostics,
    })
}

fn snapshot_from_list_file(state: &mut AssetsCatalogRuntimeState, logical_path: &str) -> Result<AssetsCatalogSnapshot, String> {
    let logical_path = normalize_catalog_path(logical_path);
    if logical_path.is_empty() || logical_path.contains('@') {
        return Err("entry directory requires a concrete ListFile path without @entry selector".to_owned());
    }
    let request = AssetDecodeRequest {
        logical_path: logical_path.clone(),
        output_kind: ASSET_LIST_FILE_MANIFEST_OUTPUT.to_owned(),
        selector: json!({}),
    };
    let bytes = state.client.decode_v1(&request)?;
    let manifest = serde_json::from_slice::<AssetFileManifest>(&bytes)
        .map_err(|error| format!("provider returned invalid AssetFileManifest: {error}"))?;
    if manifest.entries.is_empty() {
        return Err("provider manifest contains no addressable entries".to_owned());
    }

    let source_extension = path_extension_from_ref(&logical_path);
    let mut entries = manifest
        .entries
        .iter()
        .map(|entry| AssetsCatalogEntry {
            name: entry.name.clone(),
            kind: "asset_entry".to_owned(),
            logical_path: entry.entry_ref.clone(),
            extension: source_extension.clone(),
            semantic_gateway: if entry.route.gateway.trim().is_empty() { "engine.assets.inspect".to_owned() } else { entry.route.gateway.clone() },
            asset_kind: if entry.asset_kind.trim().is_empty() { manifest.file_kind.clone() } else { entry.asset_kind.clone() },
            import_stage: "listfile_entry".to_owned(),
            import_action: "inspect/edit".to_owned(),
            dirty: false,
            uid: entry.stable_id.clone(),
            thumbnail: String::new(),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));

    let mut warnings = manifest.warnings.clone();
    warnings.extend(manifest.policy.iter().map(|policy| format!("policy: {policy}")));
    let sources = match state.client.sources_json_v1() {
        Ok(value) => source_labels(&value),
        Err(error) => {
            warnings.push(format!("engine.assets sources unavailable: {error}"));
            Vec::new()
        }
    };
    let formats = match state.client.formats_json_v1() {
        Ok(value) => format_labels(&value),
        Err(error) => {
            warnings.push(format!("engine.assets formats unavailable: {error}"));
            Vec::new()
        }
    };
    let package_writer_summary = package_writer_summary(state).unwrap_or_else(|error| {
        warnings.push(format!("engine.assets.package_writer unavailable: {error}"));
        "package writer unavailable".to_owned()
    });
    let import_queue_summary = import_queue_summary(state).unwrap_or_else(|error| {
        warnings.push(format!("engine.assets.import_queue unavailable: {error}"));
        "import queue unavailable".to_owned()
    });
    let import_summary = format!("{} addressable entries from {}", entries.len(), manifest.file_kind);
    let route_diagnostics = format!(
        "ListFile directory: {} -> entries as file@entry refs · inspect=engine.assets.inspect · edit=engine.assets.edit",
        logical_path
    );

    Ok(AssetsCatalogSnapshot {
        logical_path,
        entries,
        sources,
        formats,
        warnings,
        import_summary,
        import_queue_summary,
        package_writer_summary,
        route_diagnostics,
    })
}


fn path_extension_from_ref(path: &str) -> String {
    path.split('@')
        .next()
        .unwrap_or(path)
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

fn entry_from_vfs_value(value: &Value) -> AssetsCatalogEntry {
    let name = string_field(value, &["name", "file_name", "display_name"])
        .unwrap_or_else(|| "<unnamed>".to_owned());
    let logical_path = string_field(value, &["logical_path", "path", "id", "reference"])
        .unwrap_or_else(|| name.clone());
    let kind = string_field(value, &["kind", "node_kind", "entry_kind"])
        .unwrap_or_else(|| {
            (if bool_field(value, &["is_dir", "directory", "is_directory"]) { "directory" } else { "asset" }).to_owned()
        });
    let extension = extension_from(&name, value);
    AssetsCatalogEntry {
        name,
        kind,
        logical_path: normalize_catalog_path(&logical_path),
        extension,
        semantic_gateway: string_field(value, &["semantic_gateway", "gateway"])
            .unwrap_or_else(|| "engine.assets".to_owned()),
        asset_kind: string_field(value, &["asset_kind", "content_kind", "type"])
            .unwrap_or_else(|| "asset".to_owned()),
        import_stage: "unknown".to_owned(),
        import_action: "scan".to_owned(),
        dirty: false,
        uid: String::new(),
        thumbnail: String::new(),
    }
}


fn component_id_fragment(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').to_owned();
    if trimmed.is_empty() { "node".to_owned() } else { trimmed }
}

fn assets_catalog_node(
    frame_index: u64,
    surface_size_px: [u32; 2],
    snapshot: &AssetsCatalogSnapshot,
    selected_index: usize,
    hovered_entry_index: Option<usize>,
    view_mode: CatalogViewMode,
    search_query: &str,
    collapsed_paths: &BTreeSet<String>,
    focus_scope: CatalogFocusScope,
    document_actions: &[AssetDocumentAction],
    last_action_result: Option<&AssetPatchResult>,
    context_menu_open: bool,
) -> UiSurfaceNode {
    let folder_count = snapshot.entries.iter().filter(|entry| entry.is_directory()).count();
    let asset_count = snapshot.entries.len().saturating_sub(folder_count);
    let visible_indices = filtered_entry_indices(snapshot, view_mode, search_query, collapsed_paths);
    let selected_entry = snapshot
        .entries
        .get(selected_index)
        .or_else(|| visible_indices.first().and_then(|idx| snapshot.entries.get(*idx)))
        .or_else(|| snapshot.entries.first());
    let geometry = catalog_workspace_geometry(surface_size_px);

    let mut body_lines = Vec::new();
    body_lines.push(format!(
        "{} folders · {} assets · {} mounted sources · {} declared formats",
        folder_count,
        asset_count,
        snapshot.sources.len(),
        snapshot.formats.len(),
    ));
    body_lines.push(format!("Path: {}", display_path(&snapshot.logical_path)));
    body_lines.push("Content Browser panel · selection publisher · provider DTO consumer.".to_owned());
    body_lines.push(snapshot.import_summary.clone());
    body_lines.push(format!(
        "UI focus={} · query='{}' · visible={}",
        focus_scope.as_str(),
        search_query,
        visible_indices.len()
    ));

    let mut components = Vec::new();
    push_catalog_regions(&mut components, &geometry);
    for (id, label, icon, mode, detail) in [
        ("tree", "Tree", ASSET_BROWSER_ICON_FOLDER, CatalogViewMode::Tree, "hierarchy"),
        ("list", "List", ASSET_BROWSER_ICON_GENERIC, CatalogViewMode::List, "dense rows"),
        ("grid", "Grid", ASSET_BROWSER_ICON_TEXTURE, CatalogViewMode::Grid, "previews"),
        ("inspector", "Inspector", ASSET_BROWSER_ICON_GENERIC, CatalogViewMode::Inspector, "schema DTO · providers"),
    ] {
        let tab = UiComponentNode::action(format!("asset_browser.tab.{id}"), label, format!("asset_browser.view.{id}"))
            .with_icon(icon)
            .with_detail(detail)
            .with_tone(if view_mode == mode { UiNodeTone::Accent } else { UiNodeTone::Normal })
            .tagged("tab")
            .tagged(if view_mode == mode { "active" } else { "inactive" });
        components.push(tab);
    }
    let toolbar_labels = catalog_toolbar_items(document_actions)
        .into_iter()
        .map(|item| match item {
            CatalogToolbarItem::DocumentAction { label, enabled, .. } => {
                if enabled { label } else { format!("{} · disabled", label) }
            }
            CatalogToolbarItem::ViewAction { label, .. } => label.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("    ");
    components.push(
        UiComponentNode::row("asset_browser.toolbar", toolbar_labels)
            .with_detail("Document actions are provider-declared AssetPatch templates; view actions stay local UI state")
            .with_tone(UiNodeTone::Normal)
            .tagged("toolbar")
            .tagged("asset-patch-dispatcher"),
    );
    for action in document_actions.iter() {
        let action_row = UiComponentNode::action(format!("asset_browser.action.{}", component_id_fragment(&action.id)), action.label.clone(), action.id.clone())
            .with_detail(if action.enabled { action.tooltip.clone() } else { action.disabled_reason.clone() })
            .with_tone(if action.enabled { UiNodeTone::Normal } else { UiNodeTone::Disabled })
            .tagged("action")
            .tagged("asset-patch")
            .tagged("toolbar")
            .tagged(if action.enabled { "enabled" } else { "disabled" });
        components.push(action_row);
    }

    let mut breadcrumb = UiComponentNode::row("asset_browser.breadcrumb", format!("Content  /  {}", display_path(&snapshot.logical_path)))
        .with_detail("engine.assets.vfs_list_json_v1")
        .with_tone(UiNodeTone::Accent)
        .tagged("breadcrumb");
    breadcrumb.action_id = Some("asset_browser.breadcrumb.open".to_owned());
    components.push(breadcrumb);
    let mut search = UiComponentNode::action(
        "asset_browser.search",
        "Search Content",
        "asset_browser.search.focus",
    )
    .with_value(if search_query.is_empty() {
        format!("Search {}...", browser_folder_label(&snapshot.logical_path))
    } else {
        search_query.to_owned()
    })
    .with_detail("Search/filter is local UI state; backend remains engine.assets")
    .with_tone(if focus_scope == CatalogFocusScope::Search { UiNodeTone::Accent } else { UiNodeTone::Normal })
    .tagged("search")
    .tagged(if focus_scope == CatalogFocusScope::Search { "focused" } else { "idle" });
    search.component_id = UI_COMPONENT_INPUT.to_owned();
    components.push(search);

    components.push(
        UiComponentNode::row("asset_browser.sidebar.favorites", "Favorites")
            .with_tone(UiNodeTone::Normal)
            .tagged("sidebar"),
    );
    components.push(
        {
            UiComponentNode::action("asset_browser.sidebar.root", "All Content", "asset_browser.root.open")
                .with_icon(ASSET_BROWSER_ICON_FOLDER)
                .with_detail("root")
                .with_tone(if snapshot.logical_path.is_empty() { UiNodeTone::Accent } else { UiNodeTone::Normal })
                .tagged("sidebar")
                .tagged("folder")
        },
    );
    for entry_index in visible_indices
        .iter()
        .copied()
        .filter(|entry_index| snapshot.entries.get(*entry_index).map(AssetsCatalogEntry::is_directory).unwrap_or(false))
        .take(18)
    {
        let Some(entry) = snapshot.entries.get(entry_index) else { continue; };
        let depth = entry.logical_path.split('/').count().saturating_sub(1).min(3);
        let collapsed = collapsed_paths.contains(&normalize_catalog_path(&entry.logical_path));
        let label = format!("{}{} {}", "  ".repeat(depth), if collapsed { "▸" } else { "▾" }, entry.name);
        let mut row = UiComponentNode::action(format!("asset_browser.sidebar.folder.{entry_index:03}"), label, "asset_browser.folder.open")
            .with_icon(ASSET_BROWSER_ICON_FOLDER)
            .with_detail(display_path(&entry.logical_path))
            .with_tone(if snapshot.logical_path == entry.logical_path { UiNodeTone::Accent } else { UiNodeTone::Normal })
            .tagged("sidebar")
            .tagged("folder");
        if hovered_entry_index == Some(entry_index) { row = row.tagged("hovered"); }
        components.push(row);
    }

    let selected_slot = visible_indices.iter().position(|idx| *idx == selected_index).unwrap_or(0);
    let window_size = main_visible_window_size(&geometry, view_mode)
        .min(MAX_VISIBLE_ENTRIES)
        .min(visible_indices.len())
        .max(1);
    let window_start = visible_window_start(visible_indices.len(), selected_slot, window_size);
    let scroll_page_01 = (window_size as f32 / visible_indices.len().max(1) as f32).clamp(0.05, 1.0);
    let scroll_offset_01 = if visible_indices.len() <= window_size {
        0.0
    } else {
        window_start as f32 / visible_indices.len().saturating_sub(window_size).max(1) as f32
    };

    let mut main_scroll_children: Vec<UiComponentNode> = Vec::new();
    for entry_index in visible_indices
        .iter()
        .copied()
        .filter(|entry_index| snapshot.entries.get(*entry_index).map(AssetsCatalogEntry::is_directory).unwrap_or(false))
        .skip(window_start)
        .take(10)
    {
        let Some(entry) = snapshot.entries.get(entry_index) else { continue; };
        let mut card = UiComponentNode::action(format!("asset_browser.folder_card.{entry_index:03}"), entry.name.clone(), "asset_browser.folder.open")
            .with_icon(ASSET_BROWSER_ICON_FOLDER)
            .with_value("Folder")
            .with_detail(entry.logical_path.clone())
            .with_tone(UiNodeTone::Accent)
            .tagged("folder-card");
        if hovered_entry_index == Some(entry_index) { card = card.tagged("hovered"); }
        main_scroll_children.push(card);
    }

    for visible_idx in visible_indices
        .iter()
        .copied()
        .skip(window_start)
        .filter(|entry_index| snapshot.entries.get(*entry_index).map(|entry| !entry.is_directory()).unwrap_or(false))
        .take(36)
    {
        let Some(entry) = snapshot.entries.get(visible_idx) else { continue; };
        let selected = visible_idx == selected_index;
        let hovered = hovered_entry_index == Some(visible_idx);
        let mut card = UiComponentNode::action(format!("asset_browser.asset_card.{visible_idx:03}"), entry.name.clone(), "asset_browser.asset.select")
            .with_icon(icon_for_extension(&entry.extension))
            .with_value(asset_type_label(entry))
            .with_detail(format!("{} · {}", entry.import_stage, entry.import_action))
            .with_tone(if selected { UiNodeTone::Accent } else { UiNodeTone::Normal })
            .tagged("asset-card")
            .tagged(entry.kind.clone())
            .tagged(match view_mode { CatalogViewMode::List => "list-row", CatalogViewMode::Tree => "tree-row", _ => "grid-card" });
        if selected { card = card.tagged("selected"); }
        if hovered { card = card.tagged("hovered"); }
        main_scroll_children.push(card);
    }

    let mut main_scroll = UiComponentNode::row("asset_browser.main_scroll", format!("{} visible entries", visible_indices.len()))
        .with_detail("generic ScrollContainer: wheel/drag goes through ui.dispatch_input_v1")
        .with_tone(UiNodeTone::Normal)
        .with_prop("overflow", json!("auto"))
        .with_prop("h_px", json!(154.0))
        .with_prop("row_h_px", json!(if view_mode == CatalogViewMode::Grid { 34.0 } else { 26.0 }))
        .with_prop("scrollbar_w_px", json!(8.0))
        .with_prop("scroll_offset_01", json!(scroll_offset_01))
        .with_prop("scroll_page_01", json!(scroll_page_01))
        .with_prop("scrollbar_always", json!(visible_indices.len() > window_size))
        .tagged("scroll-container")
        .tagged("asset-browser-main");
    main_scroll.component_id = match view_mode {
        CatalogViewMode::Grid => UI_COMPONENT_GRID,
        CatalogViewMode::Tree => UI_COMPONENT_TREE,
        CatalogViewMode::List | CatalogViewMode::Inspector => UI_COMPONENT_LIST,
    }
    .to_owned();
    main_scroll.props.insert("item_w_px".to_owned(), json!(132.0));
    main_scroll.props.insert("item_h_px".to_owned(), json!(88.0));
    main_scroll.props.insert("draw_panel".to_owned(), json!(true));
    main_scroll.children = main_scroll_children;
    components.push(main_scroll);

    if let Some(entry) = selected_entry {
        components.push(
            {
                let mut details = UiComponentNode::row("asset_browser.details.title", entry.name.clone())
                    .with_icon(icon_for_entry(entry))
                    .with_value(asset_type_label(entry))
                    .with_tone(UiNodeTone::Accent)
                    .tagged("details")
                    .tagged("details-title");
                details.action_id = Some("asset_browser.details.inspect".to_owned());
                details
            },
        );
        for (id, label, value) in [
            ("path", "Path", display_path(&entry.logical_path)),
            ("type", "Type", asset_type_label(entry)),
            ("extension", "Extension", if entry.extension.is_empty() { "directory".to_owned() } else { entry.extension.clone() }),
            ("gateway", "Gateway", entry.semantic_gateway.clone()),
            ("uid", "UID", if entry.uid.is_empty() { "pending".to_owned() } else { entry.uid.clone() }),
            ("import", "Import", format!("{} / {}", entry.import_stage, entry.import_action)),
            ("thumbnail", "Preview", if entry.thumbnail.is_empty() { preview_plan_label(entry).to_owned() } else { entry.thumbnail.clone() }),
            ("providers", "Providers", snapshot.route_diagnostics.clone()),
            ("package_writer", "Package Writer", snapshot.package_writer_summary.clone()),
            ("ownership", "UI Role", "selection publisher; no local right inspector".to_owned()),
            ("focus", "Focus Graph", format!("scope={} modal=false z=970", focus_scope.as_str())),
        ] {
            components.push(
                UiComponentNode::row(format!("asset_browser.details.{id}"), label)
                    .with_value(value)
                    .with_tone(UiNodeTone::Normal)
                    .tagged("details"),
            );
        }
        components.push(
            UiComponentNode::row("asset_browser.selection.bridge", "Published Selection")
                .with_value(entry.logical_path.clone())
                .with_detail("global Right Edit Window consumes EditorSelectionContext and calls engine.assets.inspect")
                .with_tone(UiNodeTone::Accent)
                .tagged("details")
                .tagged("selection-context"),
        );
        if context_menu_open {
            components.push(
                UiComponentNode::row("asset_browser.context_menu.title", "Asset Actions")
                    .with_detail("provider-declared actions; dispatch emits AssetPatch DTO through engine.assets.edit")
                    .with_tone(UiNodeTone::Accent)
                    .tagged("context-menu"),
            );
            for action in document_actions.iter() {
                let mut row = UiComponentNode::row(format!("asset_browser.context_menu.{}", component_id_fragment(&action.id)), action.label.clone())
                    .with_detail(if action.enabled { action.tooltip.clone() } else { action.disabled_reason.clone() })
                    .with_tone(if action.enabled { UiNodeTone::Normal } else { UiNodeTone::Disabled })
                    .tagged("context-menu")
                    .tagged("asset-patch");
                row.action_id = Some(action.id.clone());
                components.push(row);
            }
        }
    }

    if let Some(result) = last_action_result {
        let diagnostic = result
            .diagnostics
            .last()
            .map(|diag| diag.message.clone())
            .unwrap_or_else(|| "Asset action completed without diagnostics".to_owned());
        components.push(
            UiComponentNode::row("asset_browser.action_result", if result.written { "Asset write complete" } else if result.accepted { "Asset patch accepted" } else { "Asset action blocked" })
                .with_detail(diagnostic)
                .with_tone(if result.accepted { UiNodeTone::Accent } else { UiNodeTone::Danger })
                .tagged("status")
                .tagged("asset-patch-result"),
        );
    }

    components.push(
        UiComponentNode::row("asset_browser.status", format!("Showing {} of {} assets", asset_count.min(36), asset_count))
            .with_detail(format!("{} folders · {} · {} · F1 close · arrows navigate", folder_count, snapshot.import_queue_summary, snapshot.package_writer_summary))
            .with_tone(UiNodeTone::Accent)
            .tagged("status"),
    );
    for (idx, warning) in snapshot.warnings.iter().take(4).enumerate() {
        components.push(
            UiComponentNode::row(format!("asset_browser.warning.{idx}"), warning.clone())
                .with_icon(ASSET_BROWSER_ICON_GENERIC)
                .with_tone(UiNodeTone::Danger)
                .tagged("status")
                .tagged("warning"),
        );
    }

    apply_catalog_component_layout(&mut components, &geometry);

    let mut node = UiSurfaceNode::new(ASSETS_CATALOG_SURFACE_ID, ASSETS_CATALOG_UI_OWNER)
        .with_title("Content Browser")
        .with_subtitle("docked Content Browser panel over engine.assets")
        .with_body_lines(body_lines)
        .with_footer_lines(vec![
            "Docked panel · mouse hover/click/wheel · type to search · arrows/gamepad navigate · Enter Open/Inspect".to_owned(),
            "Content Browser publishes selection; global Right Edit Window renders AssetDocument DTOs".to_owned(),
        ])
        .with_theme(ASSETS_CATALOG_THEME_ID)
        .with_style_ref(UI_THEME_ASSET_NORTHSTAR_EDITOR)
        .with_style(assets_catalog_surface_style())
        .with_component(UI_COMPONENT_PANEL)
        .with_components(components)
        .with_metric("frame_index", json!(frame_index))
        .with_metric("current_path", json!(snapshot.logical_path.as_str()))
        .with_metric("selected_index", json!(selected_index))
        .with_metric("view_mode", json!(view_mode.as_str()))
        .with_metric("scroll_window_start", json!(window_start))
        .with_metric("scroll_offset_01", json!(scroll_offset_01))
        .with_metric("scroll_page_01", json!(scroll_page_01))
        .with_metric("search_query", json!(search_query))
        .with_metric("focus_scope", json!(focus_scope.as_str()))
        .with_metric("modal_stack", json!([ASSETS_CATALOG_SURFACE_ID]))
        .with_metric("hovered_entry_index", json!(hovered_entry_index))
        .with_metric("import_summary", json!(snapshot.import_summary.as_str()))
        .with_metric("package_writer", json!(snapshot.package_writer_summary.as_str()))
        .with_metric("folder_count", json!(folder_count))
        .with_metric("asset_count", json!(asset_count))
        .with_metric("source_count", json!(snapshot.sources.len()))
        .with_metric("format_count", json!(snapshot.formats.len()))
        .with_metric("document_action_count", json!(document_actions.len()))
        .with_metric("context_menu_open", json!(context_menu_open))
        .with_metric("last_action_written", json!(last_action_result.map(|result| result.written)));
    node.modal = false;
    node.z_order = 220;
    node.style_tags = vec![
        "workspace".to_owned(),
        "explorer-grid".to_owned(),
        "asset-catalog".to_owned(),
        "docked-panel".to_owned(),
        "dock-bottom".to_owned(),
        "engine-ui-node".to_owned(),
        "noir-editor".to_owned(),
    ];
    node
}

fn assets_catalog_error_node(frame_index: u64, error: String) -> UiSurfaceNode {
    let mut node = UiSurfaceNode::new(ASSETS_CATALOG_SURFACE_ID, ASSETS_CATALOG_UI_OWNER)
        .with_title("Content Browser")
        .with_subtitle("engine.assets data unavailable")
        .with_body_lines(vec![
            "The UI projection could not read backend asset data.".to_owned(),
            error.clone(),
            "Nothing is rendered outside engine.ui; this is a normal retained node.".to_owned(),
        ])
        .with_footer_lines(vec!["Backend must expose data; UI decides presentation.".to_owned()])
        .with_style(assets_catalog_surface_style())
        .with_component(UI_COMPONENT_PANEL)
        .with_message(UiNodeMessage::new(
            "Assets data unavailable",
            error,
            UiNodeMessageSeverity::Warning,
        ))
        .with_metric("frame_index", json!(frame_index));
    node.modal = true;
    node.z_order = 220;
    node.style_tags = vec!["workspace".to_owned(), "explorer-grid".to_owned(), "asset-catalog".to_owned(), "warning".to_owned()];
    node
}

fn filtered_entry_indices(
    snapshot: &AssetsCatalogSnapshot,
    view_mode: CatalogViewMode,
    search_query: &str,
    collapsed_paths: &BTreeSet<String>,
) -> Vec<usize> {
    let query = search_query.trim().to_ascii_lowercase();
    snapshot
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            if view_mode == CatalogViewMode::Tree && !entry.is_directory() {
                return false;
            }
            if entry.is_directory() && is_hidden_by_collapsed_parent(entry, collapsed_paths) {
                return false;
            }
            if query.is_empty() {
                return true;
            }
            entry.name.to_ascii_lowercase().contains(&query)
                || entry.logical_path.to_ascii_lowercase().contains(&query)
                || entry.asset_kind.to_ascii_lowercase().contains(&query)
                || entry.semantic_gateway.to_ascii_lowercase().contains(&query)
                || entry.extension.to_ascii_lowercase().contains(&query)
        })
        .map(|(index, _)| index)
        .collect()
}

fn is_hidden_by_collapsed_parent(entry: &AssetsCatalogEntry, collapsed_paths: &BTreeSet<String>) -> bool {
    let path = normalize_catalog_path(&entry.logical_path);
    collapsed_paths.iter().any(|collapsed| {
        let collapsed = normalize_catalog_path(collapsed);
        !collapsed.is_empty() && path != collapsed && path.starts_with(&(collapsed + "/"))
    })
}

fn main_visible_window_size(geometry: &CatalogWorkspaceGeometry, view_mode: CatalogViewMode) -> usize {
    let row_h = match view_mode {
        CatalogViewMode::Grid => 106.0,
        CatalogViewMode::Inspector => 30.0,
        CatalogViewMode::Tree | CatalogViewMode::List => 28.0,
    };
    let rows = (geometry.content_h / row_h).floor().max(1.0) as usize;
    let cols = match view_mode {
        CatalogViewMode::Grid => (geometry.main_w / 138.0).floor().max(1.0) as usize,
        CatalogViewMode::Inspector | CatalogViewMode::Tree | CatalogViewMode::List => 1,
    };
    rows.saturating_mul(cols).max(1)
}

fn visible_window_start(total: usize, selected_index: usize, window: usize) -> usize {
    if total <= window {
        return 0;
    }
    let half = window / 2;
    selected_index.saturating_sub(half).min(total.saturating_sub(window))
}

fn assets_catalog_surface_style() -> UiSurfaceStyle {
    let mut style = UiSurfaceStyle::default();
    style.anchor = UiSurfaceAnchor::BottomLeft;
    style.min_size_px = [960.0, 248.0];
    style.max_size_px = [4096.0, 320.0];
    style.margin_px = [8.0, 30.0];
    style.padding_px = [14.0, 44.0, 14.0, 24.0];
    style.row_pitch_px = 20.0;
    style.panel_rgba = [6, 10, 16, 252];
    style.panel_header_rgba = [9, 15, 24, 252];
    style.accent_rgba = [89, 164, 255, 255];
    style.text_rgba = [225, 232, 242, 255];
    style.text_muted_rgba = [137, 150, 168, 255];
    style.danger_rgba = [238, 110, 88, 255];
    style.border_rgba = [72, 91, 116, 135];
    style.backdrop_rgba = [0, 0, 0, 36];
    style.shadow_alpha = 82;
    style.corner_radius_px = 7.0;
    style.border_px = 1.0;
    style.font.stack = vec![
        UI_FONT_ASSET_EDITOR_SANS.to_owned(),
        "Inter".to_owned(),
        "Segoe UI".to_owned(),
        "NotoSans".to_owned(),
    ];
    style.font.title_px = 14.0;
    style.font.body_px = 10.0;
    style.font.secondary_px = 9.0;
    style.row_even_alpha = 8;
    style.row_odd_alpha = 3;
    style.normalized()
}
