#![forbid(unsafe_op_in_unsafe_fn)]

//! Asset Browser retained UI projection over engine.assets data.
//!
//! This crate is deliberately not a backend domain, gateway or capability. It is
//! a product/profile UI composition module: it reads reusable backend data from
//! `engine.assets` and publishes a generic `UiSurfaceNode` through `engine.ui`.
//! Rendering remains owned by the selected `engine.ui` provider.

use newengine_assets::{AssetService, AssetServiceClient};
use newengine_core::{EngineResult, Module, ModuleCtx};
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
use newengine_ui_api::{
    ui_surface_node_layout, UiComponentNode, UiInputCaptureState, UiInputFrame, UiNodeMessage,
    UiNodeMessageSeverity, UiNodeTone, UiSurfaceAnchor, UiSurfaceNode, UiSurfaceStyle,
    ENGINE_UI_SERVICE_ID, UI_COMPONENT_PANEL, UI_SERVICE_METHOD_SURFACE_NODE_V1,
};
use serde_json::{json, Value};

pub const ASSETS_CATALOG_UI_OWNER: &str = "app.asset_browser";
const ASSETS_CATALOG_SURFACE_ID: &str = "ui.assets.catalog";
const ASSETS_CATALOG_INPUT_LISTENER: &str = "asset-browser-ui";
const ASSETS_CATALOG_THEME_ID: &str = "northstar.assets.browser.editor_dark";
const ASSET_BROWSER_ICON_FOLDER: &str = "ui/icons/assetBrowser.ytd@folder";
const ASSET_BROWSER_ICON_TEXTURE: &str = "ui/icons/assetBrowser.ytd@texture";
const ASSET_BROWSER_ICON_MATERIAL: &str = "ui/icons/assetBrowser.ytd@material";
const ASSET_BROWSER_ICON_MODEL: &str = "ui/icons/assetBrowser.ytd@model";
const ASSET_BROWSER_ICON_WORLD: &str = "ui/icons/assetBrowser.ytd@world";
const ASSET_BROWSER_ICON_UI: &str = "ui/icons/assetBrowser.ytd@ui";
const ASSET_BROWSER_ICON_PACKAGE: &str = "ui/icons/assetBrowser.ytd@package";
const ASSET_BROWSER_ICON_SCRIPT: &str = "ui/icons/assetBrowser.ytd@script";
const ASSET_BROWSER_ICON_SHADER: &str = "ui/icons/assetBrowser.ytd@shader";
const ASSET_BROWSER_ICON_AUDIO: &str = "ui/icons/assetBrowser.ytd@audio";
const ASSET_BROWSER_ICON_GENERIC: &str = "ui/icons/assetBrowser.ytd@generic";
const MAX_VISIBLE_ENTRIES: usize = 64;
const MOUSE_PRIMARY_BUTTON: u32 = 1;
const DEFAULT_SURFACE_SIZE_PX: [u32; 2] = [1600, 900];

#[derive(Clone)]
pub struct AssetsCatalogRuntimeState {
    client: AssetServiceClient,
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
    last_pointer_frame: u64,
    input_registered: bool,
    cached_snapshot: Option<AssetsCatalogSnapshot>,
    cached_node: Option<UiSurfaceNode>,
    view_mode: CatalogViewMode,
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
            last_pointer_frame: u64::MAX,
            input_registered: false,
            cached_snapshot: None,
            cached_node: None,
            view_mode: CatalogViewMode::Grid,
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

    fn refresh_cache(&mut self, frame_index: u64) {
        let snapshot_result = snapshot(&mut self.state, &self.current_path);
        match snapshot_result {
            Ok(snapshot) => {
                if snapshot.entries.is_empty() {
                    self.selected_index = 0;
                } else if self.selected_index >= snapshot.entries.len() {
                    self.selected_index = snapshot.entries.len().saturating_sub(1);
                }
                let node = assets_catalog_node(frame_index, &snapshot, self.selected_index, self.view_mode);
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

    fn handle_pointer_input(&mut self, input: &UiInputFrame, surface_size_px: [u32; 2], frame_index: u64) {
        if input.mouse_wheel.1.abs() > f32::EPSILON {
            let entry_count = self.cached_snapshot.as_ref().map(|snapshot| snapshot.entries.len()).unwrap_or(0);
            if entry_count > 0 {
                if input.mouse_wheel.1 > 0.0 {
                    self.selected_index = self.selected_index.saturating_sub(1);
                } else {
                    self.selected_index = (self.selected_index + 1).min(entry_count.saturating_sub(1));
                }
                self.invalidate_node();
            }
        }

        if !input.is_mouse_pressed(MOUSE_PRIMARY_BUTTON) || self.last_pointer_frame == frame_index {
            return;
        }
        self.last_pointer_frame = frame_index;

        let Some(snapshot) = self.cached_snapshot.clone() else { return; };
        let Some(target) = catalog_hit_test(&snapshot, self.selected_index, surface_size_px, input.mouse_pos) else { return; };

        match target {
            CatalogPointerTarget::Root => {
                self.current_path.clear();
                self.selected_index = 0;
                self.view_mode = CatalogViewMode::Grid;
                self.cached_snapshot = None;
                self.invalidate_node();
                self.refresh_cache(frame_index);
                log::info!("asset browser UI: mouse open root");
            }
            CatalogPointerTarget::Folder(entry_index) => {
                if let Some(entry) = snapshot.entries.get(entry_index).filter(|entry| entry.is_directory()) {
                    self.current_path = normalize_catalog_path(&entry.logical_path);
                    self.selected_index = 0;
                    self.view_mode = CatalogViewMode::Grid;
                    self.cached_snapshot = None;
                    self.invalidate_node();
                    self.refresh_cache(frame_index);
                    log::info!("asset browser UI: mouse open directory path='{}'", display_path(&self.current_path));
                }
            }
            CatalogPointerTarget::Asset(entry_index) => {
                if entry_index < snapshot.entries.len() {
                    self.selected_index = entry_index;
                    self.view_mode = CatalogViewMode::Grid;
                    self.invalidate_node();
                    if let Some(entry) = snapshot.entries.get(entry_index) {
                        log::info!(
                            "asset browser UI: mouse selected asset path='{}' kind='{}' gateway='{}'",
                            entry.logical_path,
                            entry.asset_kind,
                            entry.semantic_gateway,
                        );
                    }
                }
            }
            CatalogPointerTarget::Inspector(entry_index) => {
                if entry_index < snapshot.entries.len() {
                    self.selected_index = entry_index;
                    self.view_mode = CatalogViewMode::Inspector;
                    self.invalidate_node();
                }
            }
            CatalogPointerTarget::Tab(view_mode) => {
                self.view_mode = view_mode;
                self.invalidate_node();
            }
            CatalogPointerTarget::Toolbar(action) => {
                match action {
                    CatalogToolbarAction::Refresh => {
                        self.cached_snapshot = None;
                        self.invalidate_node();
                        self.refresh_cache(frame_index);
                    }
                    CatalogToolbarAction::Add | CatalogToolbarAction::Import | CatalogToolbarAction::Reimport | CatalogToolbarAction::SaveAll => {
                        log::info!("asset browser UI: toolbar action '{}' routed as UI intent placeholder", action.as_str());
                    }
                }
            }
        }
    }

    fn handle_navigation_input(&mut self, actions: &InputActionFrame, frame_index: u64) {
        let entry_count = self.cached_snapshot.as_ref().map(|snapshot| snapshot.entries.len()).unwrap_or(0);
        let mut changed = false;

        if actions.ui_nav[0] < 0 || action_frame_contains(actions, engine_action::UI_NAVIGATION_LEFT) {
            self.view_mode = self.view_mode.previous();
            changed = true;
        }

        if actions.ui_nav[0] > 0 || action_frame_contains(actions, engine_action::UI_NAVIGATION_RIGHT) {
            self.view_mode = self.view_mode.next();
            changed = true;
        }

        if actions.ui_nav[1] < 0 || action_frame_contains(actions, engine_action::UI_NAVIGATION_UP) {
            self.selected_index = self.selected_index.saturating_sub(1);
            changed = true;
        }
        if (actions.ui_nav[1] > 0 || action_frame_contains(actions, engine_action::UI_NAVIGATION_DOWN)) && entry_count > 0 {
            self.selected_index = (self.selected_index + 1).min(entry_count.saturating_sub(1));
            changed = true;
        }
        if actions.ui_back || action_frame_contains(actions, engine_action::UI_NAVIGATION_BACK) {
            let parent = parent_path(&self.current_path);
            if parent != self.current_path {
                self.current_path = parent;
                self.selected_index = 0;
                self.view_mode = CatalogViewMode::Grid;
                self.cached_snapshot = None;
                changed = true;
                log::info!("asset browser UI: navigate parent path='{}'", display_path(&self.current_path));
            } else {
                self.view_mode = CatalogViewMode::Grid;
                changed = true;
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
                    changed = true;
                    log::info!("asset browser UI: open directory path='{}'", display_path(&self.current_path));
                } else {
                    self.view_mode = CatalogViewMode::Inspector;
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
                self.refresh_cache(frame_index);
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

        if toggled && self.last_toggle_frame != frame_index {
            self.open = !self.open;
            self.last_toggle_frame = frame_index;
            self.cached_node = None;
            if self.open && self.cached_snapshot.is_none() {
                self.current_path.clear();
                self.selected_index = 0;
            }
            log::info!("asset browser UI: visibility changed open={}", self.open);
        }

        if self.open {
            let stale = frame_index.saturating_sub(self.last_refresh_frame) >= 30;
            if stale || self.cached_node.is_none() || self.last_toggle_frame == frame_index {
                self.refresh_cache(frame_index);
            }
            self.handle_pointer_input(&input, surface_size_px, frame_index);
            self.handle_navigation_input(&actions, frame_index);
            if self.cached_node.is_none() {
                self.refresh_cache(frame_index);
            }
            if let Some(node) = self.cached_node.clone() {
                self.publish_surface(node);
            }
            ctx.resources_mut().insert(UiInputCaptureState::modal(
                ASSETS_CATALOG_SURFACE_ID,
                "asset browser UI modal capture",
            ));
        } else {
            if self.last_published_open || self.last_toggle_frame == frame_index {
                self.publish_surface(UiSurfaceNode::hidden(
                    ASSETS_CATALOG_SURFACE_ID,
                    ASSETS_CATALOG_UI_OWNER,
                ));
                let mut release = UiInputCaptureState::none();
                release.draw_refresh_requested = true;
                release.surfaces.push(ASSETS_CATALOG_SURFACE_ID.to_owned());
                ctx.resources_mut().insert(release);
            } else {
                ctx.resources_mut().insert(UiInputCaptureState::none());
            }
        }

        self.last_published_open = self.open;
        Ok(())
    }
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
enum CatalogPointerTarget {
    Root,
    Folder(usize),
    Asset(usize),
    Inspector(usize),
    Tab(CatalogViewMode),
    Toolbar(CatalogToolbarAction),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CatalogToolbarAction {
    Add,
    Import,
    Reimport,
    SaveAll,
    Refresh,
}

impl CatalogToolbarAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Import => "import",
            Self::Reimport => "reimport",
            Self::SaveAll => "save_all",
            Self::Refresh => "refresh",
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
    let style_tags = vec!["workspace".to_owned(), "explorer-grid".to_owned()];
    let layout = ui_surface_node_layout(surface_size_px, &style_tags, &style, 1, 0);
    let panel_x = layout.panel_x;
    let panel_y = layout.panel_y;
    let panel_w = layout.panel_w;
    let panel_h = layout.panel_h;
    let tab_h = 34.0;
    let toolbar_h = 44.0;
    let breadcrumb_h = 38.0;
    let status_h = 30.0;
    let sidebar_w = (panel_w * 0.165).clamp(210.0, 280.0);
    let details_w = (panel_w * 0.165).clamp(230.0, 300.0);
    let gap = 8.0;
    let content_top = panel_y + tab_h + toolbar_h + breadcrumb_h + gap;
    let content_bottom = panel_y + panel_h - status_h - gap;
    let content_h = (content_bottom - content_top).max(120.0);
    let sidebar_x = panel_x + gap;
    let main_x = sidebar_x + sidebar_w + gap;
    let details_x = panel_x + panel_w - details_w - gap;
    let main_w = (details_x - main_x - gap).max(320.0);
    CatalogWorkspaceGeometry { panel_x, panel_y, panel_w, panel_h, sidebar_x, sidebar_w, main_x, main_w, details_x, details_w, content_top, content_h, tab_h, toolbar_h }
}

fn catalog_hit_test(
    snapshot: &AssetsCatalogSnapshot,
    selected_index: usize,
    surface_size_px: [u32; 2],
    mouse_pos: Option<(f32, f32)>,
) -> Option<CatalogPointerTarget> {
    let (mx, my) = mouse_pos?;
    let g = catalog_workspace_geometry(surface_size_px);
    if !point_in_rect(mx, my, g.panel_x, g.panel_y, g.panel_w, g.panel_h) {
        return None;
    }

    if my >= g.panel_y && my <= g.panel_y + g.tab_h {
        let mut tx = g.panel_x + 10.0;
        let tabs = [("Asset Browser", CatalogViewMode::Grid), ("Inspector", CatalogViewMode::Inspector)];
        for (label, mode) in tabs {
            let tw = (label.chars().count() as f32 * 9.0 + 76.0).clamp(118.0, 220.0);
            if point_in_rect(mx, my, tx, g.panel_y + 4.0, tw, g.tab_h - 5.0) {
                return Some(CatalogPointerTarget::Tab(mode));
            }
            tx += tw + 4.0;
        }
    }

    let toolbar_y = g.panel_y + g.tab_h;
    if my >= toolbar_y && my <= toolbar_y + g.toolbar_h {
        let mut bx = g.panel_x + 18.0;
        for (label, action) in [
            ("+ Add", CatalogToolbarAction::Add),
            ("Import", CatalogToolbarAction::Import),
            ("Reimport", CatalogToolbarAction::Reimport),
            ("Save All", CatalogToolbarAction::SaveAll),
            ("Refresh", CatalogToolbarAction::Refresh),
        ] {
            let bw = (label.chars().count() as f32 * 9.0 + 32.0).clamp(58.0, 118.0);
            if point_in_rect(mx, my, bx, toolbar_y + 9.0, bw, g.toolbar_h - 18.0) {
                return Some(CatalogPointerTarget::Toolbar(action));
            }
            bx += bw + 8.0;
        }
    }

    if point_in_rect(mx, my, g.sidebar_x, g.content_top, g.sidebar_w, g.content_h) {
        let mut cy = g.content_top + 42.0;
        if point_in_rect(mx, my, g.sidebar_x + 8.0, cy - 5.0, g.sidebar_w - 16.0, 24.0) {
            return None;
        }
        cy += 23.0;
        if point_in_rect(mx, my, g.sidebar_x + 8.0, cy - 5.0, g.sidebar_w - 16.0, 24.0) {
            return Some(CatalogPointerTarget::Root);
        }
        cy += 23.0;
        for (entry_index, _entry) in snapshot.entries.iter().enumerate().filter(|(_, entry)| entry.is_directory()).take(18) {
            if point_in_rect(mx, my, g.sidebar_x + 8.0, cy - 5.0, g.sidebar_w - 16.0, 24.0) {
                return Some(CatalogPointerTarget::Folder(entry_index));
            }
            cy += 23.0;
            if cy > g.content_top + g.content_h - 22.0 { break; }
        }
    }

    if point_in_rect(mx, my, g.main_x, g.content_top, g.main_w, g.content_h) {
        if let Some(folder_index) = hit_folder_card(snapshot, mx, my, &g) {
            return Some(CatalogPointerTarget::Folder(folder_index));
        }
        if let Some(asset_index) = hit_asset_card(snapshot, selected_index, mx, my, &g) {
            return Some(CatalogPointerTarget::Asset(asset_index));
        }
    }

    if point_in_rect(mx, my, g.details_x, g.content_top, g.details_w, g.content_h) {
        if point_in_rect(mx, my, g.details_x + 14.0, g.content_top + 48.0, g.details_w - 28.0, 104.0) {
            return Some(CatalogPointerTarget::Inspector(selected_index));
        }
    }

    None
}

fn hit_folder_card(snapshot: &AssetsCatalogSnapshot, mx: f32, my: f32, g: &CatalogWorkspaceGeometry) -> Option<usize> {
    let cy = g.content_top + 14.0 + 30.0;
    let card_gap = 12.0;
    let folder_w = 126.0;
    let folder_h = 66.0;
    let columns = ((g.main_w - 28.0 + card_gap) / (folder_w + card_gap)).floor().max(1.0) as usize;
    for (slot, (entry_index, _entry)) in snapshot.entries.iter().enumerate().filter(|(_, entry)| entry.is_directory()).take(10).enumerate() {
        let col = slot % columns;
        let row = slot / columns;
        let cx = g.main_x + 16.0 + col as f32 * (folder_w + card_gap);
        let fy = cy + row as f32 * (folder_h + card_gap);
        if fy + folder_h > g.content_top + g.content_h * 0.42 { break; }
        if point_in_rect(mx, my, cx, fy, folder_w, folder_h) {
            return Some(entry_index);
        }
    }
    None
}

fn hit_asset_card(snapshot: &AssetsCatalogSnapshot, selected_index: usize, mx: f32, my: f32, g: &CatalogWorkspaceGeometry) -> Option<usize> {
    let folder_count = snapshot.entries.iter().filter(|entry| entry.is_directory()).count();
    let card_gap = 12.0;
    let folder_w = 126.0;
    let folder_h = 66.0;
    let folder_columns = ((g.main_w - 28.0 + card_gap) / (folder_w + card_gap)).floor().max(1.0) as usize;
    let folder_rows = ((folder_count.min(10) + folder_columns - 1) / folder_columns).max(1);
    let cy = g.content_top + 14.0 + 30.0 + folder_rows as f32 * (folder_h + card_gap) + 24.0 + 32.0;

    let asset_w = 124.0;
    let asset_h = 122.0;
    let asset_cols = ((g.main_w - 28.0 + card_gap) / (asset_w + card_gap)).floor().max(1.0) as usize;
    let window_start = visible_window_start(snapshot.entries.len(), selected_index, MAX_VISIBLE_ENTRIES);
    for (slot, (entry_index, _entry)) in snapshot
        .entries
        .iter()
        .enumerate()
        .skip(window_start)
        .filter(|(_, entry)| !entry.is_directory())
        .take(36)
        .enumerate()
    {
        let col = slot % asset_cols;
        let row = slot / asset_cols;
        let cx = g.main_x + 16.0 + col as f32 * (asset_w + card_gap);
        let ay = cy + row as f32 * (asset_h + card_gap);
        if ay + asset_h > g.content_top + g.content_h - 18.0 { break; }
        if point_in_rect(mx, my, cx, ay, asset_w, asset_h) {
            return Some(entry_index);
        }
    }
    None
}

#[inline]
fn point_in_rect(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32) -> bool {
    px >= x && px <= x + w && py >= y && py <= y + h
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
    logical_path: String,
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
struct AssetsCatalogEntry {
    name: String,
    kind: String,
    logical_path: String,
    extension: String,
    semantic_gateway: String,
    asset_kind: String,
    import_stage: String,
    import_action: String,
    dirty: bool,
    uid: String,
    thumbnail: String,
}

impl AssetsCatalogEntry {
    fn is_directory(&self) -> bool {
        let kind = self.kind.trim().to_ascii_lowercase();
        kind == "directory" || kind == "dir" || kind == "folder" || kind == "mount"
    }
}

fn snapshot(state: &mut AssetsCatalogRuntimeState, logical_path: &str) -> Result<AssetsCatalogSnapshot, String> {
    let logical_path = normalize_catalog_path(logical_path);
    let listing = state.client.vfs_list_json_v1(&logical_path)?;
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
    let route_diagnostics = format!(
        "routes: engine.assets.uid · dependencies · import_queue · package_writer · engine.ui surface node"
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

fn assets_catalog_node(frame_index: u64, snapshot: &AssetsCatalogSnapshot, selected_index: usize, view_mode: CatalogViewMode) -> UiSurfaceNode {
    let folder_count = snapshot.entries.iter().filter(|entry| entry.is_directory()).count();
    let asset_count = snapshot.entries.len().saturating_sub(folder_count);
    let selected_entry = snapshot.entries.get(selected_index).or_else(|| snapshot.entries.first());
    let _available_views = [CatalogViewMode::Tree, CatalogViewMode::List, CatalogViewMode::Grid, CatalogViewMode::Inspector];

    let mut body_lines = Vec::new();
    body_lines.push(format!(
        "{} folders · {} assets · {} mounted sources · {} declared formats",
        folder_count,
        asset_count,
        snapshot.sources.len(),
        snapshot.formats.len(),
    ));
    body_lines.push(format!("Path: {}", display_path(&snapshot.logical_path)));
    body_lines.push("Asset Browser is a retained engine.ui workspace over engine.assets data.".to_owned());
    body_lines.push(snapshot.import_summary.clone());

    let mut components = Vec::new();
    components.push(
        UiComponentNode::row("asset_browser.tab.browser", "Asset Browser")
            .with_icon(ASSET_BROWSER_ICON_FOLDER)
            .with_detail("Assets")
            .with_tone(if view_mode == CatalogViewMode::Grid { UiNodeTone::Accent } else { UiNodeTone::Normal })
            .tagged("tab")
            .tagged(if view_mode == CatalogViewMode::Grid { "active" } else { "inactive" }),
    );
    components.push(
        UiComponentNode::row("asset_browser.tab.inspector", "Inspector")
            .with_icon(ASSET_BROWSER_ICON_GENERIC)
            .with_detail("Schema DTO · settings · providers")
            .with_tone(if view_mode == CatalogViewMode::Inspector { UiNodeTone::Accent } else { UiNodeTone::Normal })
            .tagged("tab")
            .tagged(if view_mode == CatalogViewMode::Inspector { "active" } else { "inactive" }),
    );
    components.push(
        UiComponentNode::row("asset_browser.toolbar", "+ Add    Import    Reimport    Save All    Refresh")
            .with_detail(format!("Tree/List/Grid selection · active view={} · actions dispatch through engine.ui", view_mode.as_str()))
            .with_tone(UiNodeTone::Normal)
            .tagged("toolbar"),
    );
    components.push(
        UiComponentNode::row("asset_browser.breadcrumb", format!("Content  /  {}", display_path(&snapshot.logical_path)))
            .with_detail("engine.assets.vfs_list_json_v1")
            .with_tone(UiNodeTone::Accent)
            .tagged("breadcrumb"),
    );
    components.push(
        UiComponentNode::row("asset_browser.search", format!("Search {}...", browser_folder_label(&snapshot.logical_path)))
            .with_detail("Search UI is local to this node; backend remains engine.assets")
            .with_tone(UiNodeTone::Disabled)
            .tagged("search"),
    );

    components.push(
        UiComponentNode::row("asset_browser.sidebar.favorites", "Favorites")
            .with_tone(UiNodeTone::Normal)
            .tagged("sidebar"),
    );
    components.push(
        UiComponentNode::row("asset_browser.sidebar.root", "All Content")
            .with_icon(ASSET_BROWSER_ICON_FOLDER)
            .with_detail("root")
            .with_tone(if snapshot.logical_path.is_empty() { UiNodeTone::Accent } else { UiNodeTone::Normal })
            .tagged("sidebar")
            .tagged("folder"),
    );
    for (idx, entry) in snapshot.entries.iter().filter(|entry| entry.is_directory()).take(18).enumerate() {
        let depth = entry.logical_path.split('/').count().saturating_sub(1).min(3);
        let label = format!("{}{}", "  ".repeat(depth), entry.name);
        components.push(
            UiComponentNode::row(format!("asset_browser.sidebar.folder.{idx:02}"), label)
                .with_icon(ASSET_BROWSER_ICON_FOLDER)
                .with_detail(display_path(&entry.logical_path))
                .with_tone(if snapshot.logical_path == entry.logical_path { UiNodeTone::Accent } else { UiNodeTone::Normal })
                .tagged("sidebar")
                .tagged("folder"),
        );
    }

    for (idx, entry) in snapshot.entries.iter().filter(|entry| entry.is_directory()).take(10).enumerate() {
        components.push(
            UiComponentNode::row(format!("asset_browser.folder_card.{idx:02}"), entry.name.clone())
                .with_icon(ASSET_BROWSER_ICON_FOLDER)
                .with_value("Folder")
                .with_detail(entry.logical_path.clone())
                .with_tone(UiNodeTone::Accent)
                .tagged("folder-card"),
        );
    }

    let window_start = visible_window_start(snapshot.entries.len(), selected_index, MAX_VISIBLE_ENTRIES);
    for (visible_idx, entry) in snapshot
        .entries
        .iter()
        .enumerate()
        .skip(window_start)
        .filter(|(_, entry)| !entry.is_directory())
        .take(36)
    {
        let selected = visible_idx == selected_index;
        let mut card = UiComponentNode::row(format!("asset_browser.asset_card.{visible_idx:03}"), entry.name.clone())
            .with_icon(icon_for_extension(&entry.extension))
            .with_value(asset_type_label(entry))
            .with_detail(format!("{} · {}", entry.import_stage, entry.import_action))
            .with_tone(if selected { UiNodeTone::Accent } else { UiNodeTone::Normal })
            .tagged("asset-card")
            .tagged(entry.kind.clone());
        if selected {
            card = card.tagged("selected");
        }
        components.push(card);
    }

    if let Some(entry) = selected_entry {
        components.push(
            UiComponentNode::row("asset_browser.details.title", entry.name.clone())
                .with_icon(icon_for_entry(entry))
                .with_value(asset_type_label(entry))
                .with_tone(UiNodeTone::Accent)
                .tagged("details")
                .tagged("details-title"),
        );
        for (id, label, value) in [
            ("path", "Path", display_path(&entry.logical_path)),
            ("type", "Type", asset_type_label(entry)),
            ("extension", "Extension", if entry.extension.is_empty() { "directory".to_owned() } else { entry.extension.clone() }),
            ("gateway", "Gateway", entry.semantic_gateway.clone()),
            ("uid", "UID", if entry.uid.is_empty() { "pending".to_owned() } else { entry.uid.clone() }),
            ("import", "Import", format!("{} / {}", entry.import_stage, entry.import_action)),
            ("thumbnail", "Preview", if entry.thumbnail.is_empty() { preview_plan_label(entry).to_owned() } else { entry.thumbnail.clone() }),
            ("readonly_dto", "Readonly DTO", "available in details panel".to_owned()),
            ("settings", "Settings", "schema-driven editor placeholder".to_owned()),
            ("providers", "Providers", snapshot.route_diagnostics.clone()),
            ("package_writer", "Package Writer", snapshot.package_writer_summary.clone()),
            ("ownership", "UI Role", "visualization only".to_owned()),
        ] {
            components.push(
                UiComponentNode::row(format!("asset_browser.details.{id}"), label)
                    .with_value(value)
                    .with_tone(UiNodeTone::Normal)
                    .tagged("details"),
            );
        }
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

    let mut node = UiSurfaceNode::new(ASSETS_CATALOG_SURFACE_ID, ASSETS_CATALOG_UI_OWNER)
        .with_title("Asset Browser")
        .with_subtitle("clean editor workspace over engine.assets")
        .with_body_lines(body_lines)
        .with_footer_lines(vec![
            "F1 Close · mouse select/open · wheel select · arrows navigate · Enter Open/Inspect".to_owned(),
            "Every asset format declares a provider preview contract; UI only composes it".to_owned(),
        ])
        .with_theme(ASSETS_CATALOG_THEME_ID)
        .with_style(assets_catalog_surface_style())
        .with_component(UI_COMPONENT_PANEL)
        .with_components(components)
        .with_metric("frame_index", json!(frame_index))
        .with_metric("current_path", json!(snapshot.logical_path.as_str()))
        .with_metric("selected_index", json!(selected_index))
        .with_metric("view_mode", json!(view_mode.as_str()))
        .with_metric("import_summary", json!(snapshot.import_summary.as_str()))
        .with_metric("package_writer", json!(snapshot.package_writer_summary.as_str()))
        .with_metric("folder_count", json!(folder_count))
        .with_metric("asset_count", json!(asset_count))
        .with_metric("source_count", json!(snapshot.sources.len()))
        .with_metric("format_count", json!(snapshot.formats.len()));
    node.modal = false;
    node.z_order = 970;
    node.style_tags = vec![
        "workspace".to_owned(),
        "explorer-grid".to_owned(),
        "asset-catalog".to_owned(),
        "engine-ui-node".to_owned(),
        "noir-editor".to_owned(),
    ];
    node
}

fn assets_catalog_error_node(frame_index: u64, error: String) -> UiSurfaceNode {
    let mut node = UiSurfaceNode::new(ASSETS_CATALOG_SURFACE_ID, ASSETS_CATALOG_UI_OWNER)
        .with_title("Asset Browser")
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
    node.z_order = 970;
    node.style_tags = vec!["workspace".to_owned(), "explorer-grid".to_owned(), "asset-catalog".to_owned(), "warning".to_owned()];
    node
}

fn visible_window_start(total: usize, selected_index: usize, window: usize) -> usize {
    if total <= window {
        return 0;
    }
    let half = window / 2;
    selected_index.saturating_sub(half).min(total.saturating_sub(window))
}

fn parent_path(path: &str) -> String {
    let normalized = normalize_catalog_path(path);
    let trimmed = normalized.trim_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }
    trimmed
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_owned())
        .unwrap_or_default()
}

fn normalize_catalog_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim()
        .trim_start_matches("assets://")
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_owned()
}

fn display_path(path: &str) -> String {
    let path = normalize_catalog_path(path);
    if path.is_empty() { "/".to_owned() } else { format!("/{path}") }
}

fn assets_catalog_surface_style() -> UiSurfaceStyle {
    let mut style = UiSurfaceStyle::default();
    style.anchor = UiSurfaceAnchor::TopLeft;
    style.min_size_px = [1480.0, 850.0];
    style.max_size_px = [4096.0, 4096.0];
    style.margin_px = [8.0, 8.0];
    style.padding_px = [18.0, 84.0, 18.0, 36.0];
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
    style.font.stack = vec!["NorthStarSans".to_owned(), "Inter".to_owned(), "Segoe UI".to_owned(), "NotoSans".to_owned()];
    style.font.title_px = 14.0;
    style.font.body_px = 10.0;
    style.font.secondary_px = 9.0;
    style.row_even_alpha = 8;
    style.row_odd_alpha = 3;
    style.normalized()
}

fn browser_folder_label(path: &str) -> String {
    let path = normalize_catalog_path(path);
    path.rsplit('/').next().filter(|value| !value.is_empty()).unwrap_or("Content").to_owned()
}

fn asset_type_label(entry: &AssetsCatalogEntry) -> String {
    if entry.is_directory() {
        return "Folder".to_owned();
    }
    let kind = entry.asset_kind.trim();
    if kind.is_empty() || kind == "asset" {
        match entry.extension.as_str() {
            "neui" => "UI Dictionary".to_owned(),
            "nemat" => "Material Library".to_owned(),
            "ytd" => "Texture Dictionary".to_owned(),
            "ydd" | "ydr" | "obj" | "gltf" | "glb" => "Model / Drawable".to_owned(),
            "ytyp" => "Scene Definition".to_owned(),
            "ymap" => "Map".to_owned(),
            "wav" | "ogg" => "Audio".to_owned(),
            _ => "Asset".to_owned(),
        }
    } else {
        kind.to_owned()
    }
}

fn icon_for_entry(entry: &AssetsCatalogEntry) -> &'static str {
    if entry.is_directory() { ASSET_BROWSER_ICON_FOLDER } else { icon_for_extension(&entry.extension) }
}

fn preview_plan_label(entry: &AssetsCatalogEntry) -> &'static str {
    if entry.is_directory() {
        "folder preview"
    } else {
        match entry.extension.as_str() {
            "ytd" | "png" | "jpg" | "jpeg" | "dds" => "texture preview provider",
            "nemat" => "material preview provider",
            "ydd" | "ydr" | "obj" | "gltf" | "glb" => "model preview provider",
            "ytyp" | "ymap" => "world metadata preview provider",
            "neui" => "UI preview provider",
            _ => "metadata preview provider",
        }
    }
}

fn source_labels(value: &Value) -> Vec<String> {
    value
        .get("sources")
        .and_then(Value::as_array)
        .or_else(|| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|item| string_field(item, &["id", "name", "root", "logical_root"]))
        .take(64)
        .collect()
}

fn format_labels(value: &Value) -> Vec<String> {
    value
        .get("formats")
        .and_then(Value::as_array)
        .or_else(|| value.get("descriptors").and_then(Value::as_array))
        .or_else(|| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|item| string_field(item, &["extension", "id", "asset_kind", "content_kind"]))
        .take(64)
        .collect()
}


fn hydrate_preview_plans_for_entries(
    state: &mut AssetsCatalogRuntimeState,
    entries: &mut [AssetsCatalogEntry],
    warnings: &mut Vec<String>,
) {
    let mut failures = 0usize;
    for entry in entries.iter_mut().filter(|entry| !entry.is_directory()).take(MAX_VISIBLE_ENTRIES) {
        if !entry.thumbnail.trim().is_empty() {
            continue;
        }
        match state.client.thumbnail_json_v1(json!({ "logical_path": entry.logical_path.as_str() })) {
            Ok(value) => {
                entry.thumbnail = thumbnail_label_from_value(&value)
                    .unwrap_or_else(|| preview_plan_label(entry).to_owned());
            }
            Err(error) => {
                failures += 1;
                if failures <= 2 {
                    warnings.push(format!("engine.assets.thumbnail_v1 unavailable for '{}': {error}", entry.logical_path));
                }
                entry.thumbnail = preview_plan_label(entry).to_owned();
            }
        }
    }
    if failures > 2 {
        warnings.push(format!("engine.assets.thumbnail_v1 unavailable for {} additional assets", failures - 2));
    }
}

fn thumbnail_label_from_value(value: &Value) -> Option<String> {
    let thumbnail = value.get("thumbnail")?;
    let kind = string_field(thumbnail, &["kind", "strategy", "label"])?;
    let state = string_field(thumbnail, &["state"]).unwrap_or_else(|| "planned".to_owned());
    let icon = string_field(thumbnail, &["icon_ref", "icon", "asset_icon"]);
    let cache_key = string_field(thumbnail, &["cache_key"]);
    Some(match (icon, cache_key) {
        (Some(icon), Some(cache_key)) => format!("{kind} / {state} / {icon} / {cache_key}"),
        (Some(icon), None) => format!("{kind} / {state} / {icon}"),
        (None, Some(cache_key)) => format!("{kind} / {state} / {cache_key}"),
        (None, None) => format!("{kind} / {state}"),
    })
}

fn apply_import_lifecycle_rows(
    state: &mut AssetsCatalogRuntimeState,
    logical_path: &str,
    entries: &mut [AssetsCatalogEntry],
    warnings: &mut Vec<String>,
) {
    let response = match state.client.dirty_scan_json_v1(json!({
        "root": logical_path,
        "recursive": false,
        "max_entries": 256,
    })) {
        Ok(value) => value,
        Err(error) => {
            warnings.push(format!("engine.assets.dirty_scan_v1 unavailable: {error}"));
            return;
        }
    };
    let rows = response.get("rows").and_then(Value::as_array).cloned().unwrap_or_default();
    for row in rows {
        let Some(path) = string_field(&row, &["logical_path", "path"]) else { continue; };
        let normalized = normalize_catalog_path(&path);
        if let Some(entry) = entries.iter_mut().find(|entry| entry.logical_path == normalized) {
            entry.import_stage = string_field(&row, &["stage"]).unwrap_or_else(|| "unknown".to_owned());
            entry.import_action = string_field(&row, &["recommended_action"]).unwrap_or_else(|| "none".to_owned());
            entry.dirty = row.get("dirty").and_then(Value::as_bool).unwrap_or(false);
            entry.uid = string_field(&row, &["uid"]).unwrap_or_default();
            entry.thumbnail = row
                .get("thumbnail")
                .and_then(|thumbnail| string_field(thumbnail, &["kind", "strategy", "label"]))
                .unwrap_or_default();
        }
    }
}

fn package_writer_summary(state: &mut AssetsCatalogRuntimeState) -> Result<String, String> {
    let value = state.client.package_writer_info_json_v1(json!({}))?;
    let ops = value.get("operations").and_then(Value::as_object);
    let loose = ops.and_then(|o| o.get("loose_vfs_write_back")).and_then(Value::as_bool).unwrap_or(false);
    let listfile = ops.and_then(|o| o.get("nef8_listfile_repack")).and_then(Value::as_bool).unwrap_or(false);
    let nepak = ops.and_then(|o| o.get("nepak_container_write_back")).and_then(Value::as_bool).unwrap_or(false);
    Ok(format!("package writer: loose={} listfile={} nepak={}", loose, listfile, nepak))
}

fn import_queue_summary(state: &mut AssetsCatalogRuntimeState) -> Result<String, String> {
    let value = state.client.import_queue_json_v1(json!({}))?;
    if let Some(summary) = value.get("summary") {
        let queued = summary.get("queued").or_else(|| summary.get("queue_len")).and_then(Value::as_u64).unwrap_or(0);
        let active = summary.get("active").and_then(Value::as_u64).unwrap_or(0);
        return Ok(format!("import queue: queued={} active={}", queued, active));
    }
    let queued = value.get("queued").and_then(Value::as_array).map(|v| v.len()).unwrap_or(0);
    Ok(format!("import queue: queued={} active=0", queued))
}

fn import_summary_for_entries(entries: &[AssetsCatalogEntry]) -> String {
    let dirty = entries.iter().filter(|entry| entry.dirty).count();
    let reimport = entries.iter().filter(|entry| entry.import_action == "reimport").count();
    let import = entries.iter().filter(|entry| entry.import_action == "import").count();
    format!("Import status: {} dirty · {} reimport · {} new import", dirty, reimport, import)
}

fn value_warnings(value: &Value) -> Vec<String> {
    value
        .get("warnings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn bool_field(value: &Value, keys: &[&str]) -> bool {
    keys.iter().any(|key| value.get(*key).and_then(Value::as_bool).unwrap_or(false))
}

fn extension_from(name: &str, value: &Value) -> String {
    if let Some(ext) = string_field(value, &["extension", "ext"]) {
        return ext.trim_start_matches('.').to_ascii_lowercase();
    }
    name.rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

fn icon_for_extension(ext: &str) -> &'static str {
    match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
        "neui" => ASSET_BROWSER_ICON_UI,
        "ytd" | "png" | "jpg" | "jpeg" | "dds" => ASSET_BROWSER_ICON_TEXTURE,
        "ydd" | "ydr" | "obj" | "gltf" | "glb" => ASSET_BROWSER_ICON_MODEL,
        "ytyp" | "ymap" => ASSET_BROWSER_ICON_WORLD,
        "nemat" => ASSET_BROWSER_ICON_MATERIAL,
        "nepak" => ASSET_BROWSER_ICON_PACKAGE,
        "nepat" => ASSET_BROWSER_ICON_GENERIC,
        "lua" | "ron" | "json" | "toml" | "rs" | "py" | "bat" | "cmd" => ASSET_BROWSER_ICON_SCRIPT,
        "vert" | "frag" | "wgsl" | "glsl" => ASSET_BROWSER_ICON_SHADER,
        "wav" | "ogg" => ASSET_BROWSER_ICON_AUDIO,
        "" => ASSET_BROWSER_ICON_GENERIC,
        _ => ASSET_BROWSER_ICON_GENERIC,
    }
}
