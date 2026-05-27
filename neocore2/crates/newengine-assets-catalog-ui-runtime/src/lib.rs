#![forbid(unsafe_op_in_unsafe_fn)]

//! Content Browser retained UI projection over engine.assets data.
//!
//! This crate is deliberately not a backend domain, gateway or capability. It is
//! a product/profile UI composition module: it reads reusable backend data from
//! `engine.assets` and publishes a generic `UiSurfaceNode` through `engine.ui`.
//! Rendering remains owned by the selected `engine.ui` provider.

use newengine_assets::{AssetService, AssetServiceClient};
use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_input_actions_api::{
    engine_action, InputActionDispatchMode, InputActionFrame, InputFrameSource,
};
use newengine_input_api::{engine_default_keybind, key_code, key_identity};
use newengine_input_bindings_api::{
    InputBinding, InputBindingRegistration, InputKeyRegistration,
};
use newengine_plugin_api::HostApiV1;
use newengine_ui_api::{
    UiComponentNode, UiInputCaptureState, UiInputFrame, UiNodeMessage,
    UiNodeMessageSeverity, UiNodeTone, UiSurfaceAnchor, UiSurfaceNode, UiSurfaceStyle,
    ENGINE_UI_SERVICE_ID, UI_COMPONENT_PANEL, UI_SERVICE_METHOD_SURFACE_NODE_V1,
};
use serde_json::{json, Value};

pub const ASSETS_CATALOG_UI_OWNER: &str = "newengine.assets_catalog_ui";
const ASSETS_CATALOG_SURFACE_ID: &str = "ui.assets.catalog";
const ASSETS_CATALOG_INPUT_LISTENER: &str = "assets-catalog-ui";
const ASSETS_CATALOG_THEME_ID: &str = "northstar.assets.catalog";
const MAX_VISIBLE_ENTRIES: usize = 64;

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
    input_registered: bool,
    last_input_registration_frame: Option<u64>,
    cached_snapshot: Option<AssetsCatalogSnapshot>,
    cached_node: Option<UiSurfaceNode>,
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
            input_registered: false,
            last_input_registration_frame: None,
            cached_snapshot: None,
            cached_node: None,
        }
    }

    fn publish_surface(&self, node: UiSurfaceNode) {
        let payload = match serde_json::to_vec(&node) {
            Ok(payload) => payload,
            Err(error) => {
                log::warn!("assets catalog UI: surface serialization failed: {error}");
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
                    "assets catalog UI: engine.ui is unavailable; surface='{}' skipped instead of using a native/special renderer",
                    node.surface_id,
                );
            }
            Err(error) => {
                log::warn!("assets catalog UI: engine.ui surface publish failed: {error}");
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
                let node = assets_catalog_node(frame_index, &snapshot, self.selected_index);
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

    fn handle_navigation_input(&mut self, input: &UiInputFrame, frame_index: u64) {
        let entry_count = self.cached_snapshot.as_ref().map(|snapshot| snapshot.entries.len()).unwrap_or(0);
        let mut changed = false;

        if key_pressed(input, key_code::ARROW_UP) {
            self.selected_index = self.selected_index.saturating_sub(1);
            changed = true;
        }
        if key_pressed(input, key_code::ARROW_DOWN) && entry_count > 0 {
            self.selected_index = (self.selected_index + 1).min(entry_count.saturating_sub(1));
            changed = true;
        }
        if key_pressed(input, key_code::BACKSPACE) {
            let parent = parent_path(&self.current_path);
            if parent != self.current_path {
                self.current_path = parent;
                self.selected_index = 0;
                self.cached_snapshot = None;
                changed = true;
                log::info!("assets catalog UI: navigate parent path='{}'", display_path(&self.current_path));
            }
        }
        if key_pressed(input, key_code::ENTER) {
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
                    log::info!("assets catalog UI: open directory path='{}'", display_path(&self.current_path));
                } else {
                    log::info!(
                        "assets catalog UI: selected asset path='{}' kind='{}' gateway='{}'",
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
        "newengine.assets_catalog_ui.node"
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let frame_index = ctx.frame().map(|frame| frame.frame_index).unwrap_or(0);
        if !self.input_registered {
            let should_attempt = self
                .last_input_registration_frame
                .map(|last| frame_index.saturating_sub(last) >= 60)
                .unwrap_or(true);
            if should_attempt {
                self.last_input_registration_frame = Some(frame_index);
                self.input_registered = ensure_assets_catalog_input_registration();
            }
        }

        let input = ctx.resources().get::<UiInputFrame>().cloned().unwrap_or_default();
        let actions = resolve_actions(&input);
        let toggled = action_frame_contains(&actions, engine_action::ASSET_CATALOG_UI_TOGGLE);

        if toggled && self.last_toggle_frame != frame_index {
            self.open = !self.open;
            self.last_toggle_frame = frame_index;
            self.cached_node = None;
            if self.open && self.cached_snapshot.is_none() {
                self.current_path.clear();
                self.selected_index = 0;
            }
            log::info!("assets catalog UI: visibility changed open={}", self.open);
        }

        if self.open {
            let stale = frame_index.saturating_sub(self.last_refresh_frame) >= 30;
            if stale || self.cached_node.is_none() || self.last_toggle_frame == frame_index {
                self.refresh_cache(frame_index);
            }
            self.handle_navigation_input(&input, frame_index);
            if self.cached_node.is_none() {
                self.refresh_cache(frame_index);
            }
            if let Some(node) = self.cached_node.clone() {
                self.publish_surface(node);
            }
            ctx.resources_mut().insert(UiInputCaptureState::modal(
                ASSETS_CATALOG_SURFACE_ID,
                "assets catalog UI modal capture",
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

fn key_pressed(input: &UiInputFrame, key: u32) -> bool {
    input.keys_pressed.contains(&key)
}

fn ensure_assets_catalog_input_registration() -> bool {
    let mut ok = true;
    for (code, identity, label) in [
        (engine_default_keybind::ASSET_CATALOG_UI_TOGGLE, key_identity::F1, "F1"),
        (key_code::ARROW_UP, key_identity::ARROW_UP, "Arrow Up"),
        (key_code::ARROW_DOWN, key_identity::ARROW_DOWN, "Arrow Down"),
        (key_code::ENTER, key_identity::ENTER, "Enter"),
        (key_code::BACKSPACE, key_identity::BACKSPACE, "Backspace"),
    ] {
        if let Err(error) = newengine_input_bindings_runtime::register_input_key(
            InputKeyRegistration::new(code, identity, label),
        ) {
            log::warn!("assets catalog UI: key registration failed key='{label}': {error}");
            ok = false;
        }
    }
    if let Err(error) = newengine_input_bindings_runtime::register_input_action(
        newengine_input_actions_api::InputActionDefinition::new(engine_action::ASSET_CATALOG_UI_TOGGLE)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Toggle assets catalog UI"),
    ) {
        log::warn!("assets catalog UI: action registration failed: {error}");
        ok = false;
    }
    if let Err(error) = newengine_input_bindings_runtime::register_input_binding(
        InputBindingRegistration::new(InputBinding::keyboard_pressed(
            engine_action::ASSET_CATALOG_UI_TOGGLE,
            engine_default_keybind::ASSET_CATALOG_UI_TOGGLE,
        )),
    ) {
        log::warn!("assets catalog UI: binding registration failed: {error}");
        ok = false;
    }
    if let Err(error) = newengine_input_bindings_runtime::register_input_listener(
        newengine_input_actions_api::InputActionListenerRegistration::new(
            ASSETS_CATALOG_UI_OWNER,
            ASSETS_CATALOG_INPUT_LISTENER,
        )
        .with_actions([engine_action::ASSET_CATALOG_UI_TOGGLE])
        .with_priority(90)
        .consuming(),
    ) {
        log::warn!("assets catalog UI: listener registration failed: {error}");
        ok = false;
    }
    ok
}

#[derive(Clone, Debug, Default)]
struct AssetsCatalogSnapshot {
    logical_path: String,
    entries: Vec<AssetsCatalogEntry>,
    sources: Vec<String>,
    formats: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Default)]
struct AssetsCatalogEntry {
    name: String,
    kind: String,
    logical_path: String,
    extension: String,
    semantic_gateway: String,
    asset_kind: String,
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

    Ok(AssetsCatalogSnapshot {
        logical_path,
        entries,
        sources,
        formats,
        warnings,
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
    }
}

fn assets_catalog_node(frame_index: u64, snapshot: &AssetsCatalogSnapshot, selected_index: usize) -> UiSurfaceNode {
    let folder_count = snapshot.entries.iter().filter(|entry| entry.is_directory()).count();
    let asset_count = snapshot.entries.len().saturating_sub(folder_count);
    let selected_entry = snapshot.entries.get(selected_index).or_else(|| snapshot.entries.first());

    let mut body_lines = Vec::new();
    body_lines.push(format!(
        "{} folders · {} assets · {} mounted sources · {} declared formats",
        folder_count,
        asset_count,
        snapshot.sources.len(),
        snapshot.formats.len(),
    ));
    body_lines.push(format!("Path: {}", display_path(&snapshot.logical_path)));
    body_lines.push("Content Browser is an engine.ui application node over engine.assets data.".to_owned());

    let mut components = Vec::new();
    components.push(
        UiComponentNode::row("content_browser.tab.browser", "Content Browser")
            .with_icon("◈")
            .with_detail("Assets")
            .with_tone(UiNodeTone::Accent)
            .tagged("tab")
            .tagged("active"),
    );
    components.push(
        UiComponentNode::row("content_browser.tab.materials", "Materials")
            .with_icon("◇")
            .with_detail("Pinned workspace")
            .with_tone(UiNodeTone::Normal)
            .tagged("tab"),
    );
    components.push(
        UiComponentNode::row("content_browser.toolbar", "+ Add    Import    Save All    Refresh")
            .with_icon("TB")
            .with_detail("Filters · All Types · All Platforms · Grid View")
            .with_tone(UiNodeTone::Normal)
            .tagged("toolbar"),
    );
    components.push(
        UiComponentNode::row("content_browser.breadcrumb", format!("Content  ›  {}", display_path(&snapshot.logical_path)))
            .with_icon("PATH")
            .with_detail("engine.assets.vfs_list_json_v1")
            .with_tone(UiNodeTone::Accent)
            .tagged("breadcrumb"),
    );
    components.push(
        UiComponentNode::row("content_browser.search", format!("Search {}...", browser_folder_label(&snapshot.logical_path)))
            .with_icon("⌕")
            .with_detail("Search UI is local to this node; backend remains engine.assets")
            .with_tone(UiNodeTone::Disabled)
            .tagged("search"),
    );

    components.push(
        UiComponentNode::row("content_browser.sidebar.favorites", "Favorites")
            .with_icon("▸")
            .with_tone(UiNodeTone::Normal)
            .tagged("sidebar"),
    );
    components.push(
        UiComponentNode::row("content_browser.sidebar.root", "All Content")
            .with_icon("▾")
            .with_detail("root")
            .with_tone(if snapshot.logical_path.is_empty() { UiNodeTone::Accent } else { UiNodeTone::Normal })
            .tagged("sidebar")
            .tagged("folder"),
    );
    for (idx, entry) in snapshot.entries.iter().filter(|entry| entry.is_directory()).take(18).enumerate() {
        let depth = entry.logical_path.split('/').count().saturating_sub(1).min(3);
        let label = format!("{}{}", "  ".repeat(depth), entry.name);
        components.push(
            UiComponentNode::row(format!("content_browser.sidebar.folder.{idx:02}"), label)
                .with_icon("▸")
                .with_detail(display_path(&entry.logical_path))
                .with_tone(if snapshot.logical_path == entry.logical_path { UiNodeTone::Accent } else { UiNodeTone::Normal })
                .tagged("sidebar")
                .tagged("folder"),
        );
    }

    for (idx, entry) in snapshot.entries.iter().filter(|entry| entry.is_directory()).take(10).enumerate() {
        components.push(
            UiComponentNode::row(format!("content_browser.folder_card.{idx:02}"), entry.name.clone())
                .with_icon("▰")
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
        let mut card = UiComponentNode::row(format!("content_browser.asset_card.{visible_idx:03}"), entry.name.clone())
            .with_icon(icon_for_extension(&entry.extension))
            .with_value(asset_type_label(entry))
            .with_detail(entry.logical_path.clone())
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
            UiComponentNode::row("content_browser.details.title", entry.name.clone())
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
            ("ownership", "UI Role", "visualization only".to_owned()),
        ] {
            components.push(
                UiComponentNode::row(format!("content_browser.details.{id}"), label)
                    .with_value(value)
                    .with_tone(UiNodeTone::Normal)
                    .tagged("details"),
            );
        }
    }

    components.push(
        UiComponentNode::row("content_browser.status", format!("Showing {} of {} assets", asset_count.min(36), asset_count))
            .with_icon("●")
            .with_detail(format!("{} folders · Page 1 · F1 close · arrows navigate", folder_count))
            .with_tone(UiNodeTone::Accent)
            .tagged("status"),
    );
    for (idx, warning) in snapshot.warnings.iter().take(4).enumerate() {
        components.push(
            UiComponentNode::row(format!("content_browser.warning.{idx}"), warning.clone())
                .with_icon("WARN")
                .with_tone(UiNodeTone::Danger)
                .tagged("status")
                .tagged("warning"),
        );
    }

    let mut node = UiSurfaceNode::new(ASSETS_CATALOG_SURFACE_ID, ASSETS_CATALOG_UI_OWNER)
        .with_title("Content Browser")
        .with_subtitle("Explorer-style UI composition over engine.assets")
        .with_body_lines(body_lines)
        .with_footer_lines(vec![
            "F1 Close · ↑/↓ Select · Enter Open Folder · Backspace Parent".to_owned(),
            "This is not a backend domain; it consumes engine.assets and publishes engine.ui nodes".to_owned(),
        ])
        .with_theme(ASSETS_CATALOG_THEME_ID)
        .with_style(assets_catalog_surface_style())
        .with_component(UI_COMPONENT_PANEL)
        .with_components(components)
        .with_metric("frame_index", json!(frame_index))
        .with_metric("current_path", json!(snapshot.logical_path.as_str()))
        .with_metric("selected_index", json!(selected_index))
        .with_metric("folder_count", json!(folder_count))
        .with_metric("asset_count", json!(asset_count))
        .with_metric("source_count", json!(snapshot.sources.len()))
        .with_metric("format_count", json!(snapshot.formats.len()));
    node.modal = false;
    node.z_order = 970;
    node.style_tags = vec![
        "workspace".to_owned(),
        "content-browser".to_owned(),
        "explorer-grid".to_owned(),
        "assets-catalog".to_owned(),
        "engine-ui-node".to_owned(),
        "modern".to_owned(),
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
    node.z_order = 970;
    node.style_tags = vec!["tool".to_owned(), "assets-catalog".to_owned(), "warning".to_owned()];
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
    style.min_size_px = [1440.0, 860.0];
    style.max_size_px = [4096.0, 4096.0];
    style.margin_px = [8.0, 8.0];
    style.padding_px = [22.0, 108.0, 22.0, 48.0];
    style.row_pitch_px = 24.0;
    style.panel_rgba = [6, 11, 18, 252];
    style.panel_header_rgba = [11, 18, 30, 252];
    style.accent_rgba = [82, 154, 255, 255];
    style.text_rgba = [229, 236, 247, 255];
    style.text_muted_rgba = [150, 163, 184, 255];
    style.danger_rgba = [255, 142, 110, 255];
    style.border_rgba = [71, 87, 112, 140];
    style.backdrop_rgba = [0, 0, 0, 0];
    style.shadow_alpha = 0;
    style.corner_radius_px = 8.0;
    style.border_px = 1.0;
    style.font.title_px = 22.0;
    style.font.body_px = 14.0;
    style.font.secondary_px = 12.0;
    style.row_even_alpha = 8;
    style.row_odd_alpha = 4;
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
            "neui" => "Widget/UI".to_owned(),
            "nemat" => "Material".to_owned(),
            "ytd" => "Texture Dictionary".to_owned(),
            "ydd" => "Drawable Dictionary".to_owned(),
            "ytyp" => "Scene Definition".to_owned(),
            "ymap" => "Map".to_owned(),
            "wav" | "ogg" => "Sound Wave".to_owned(),
            _ => "Asset".to_owned(),
        }
    } else {
        kind.to_owned()
    }
}

fn icon_for_entry(entry: &AssetsCatalogEntry) -> &'static str {
    if entry.is_directory() { "DIR" } else { icon_for_extension(&entry.extension) }
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
        "neui" => "UI",
        "ytd" => "TX",
        "ydd" => "MD",
        "ytyp" => "SC",
        "nemat" => "MT",
        "nepak" => "PK",
        "nepat" => "AI",
        "lua" | "ron" | "json" | "toml" => "CFG",
        "png" | "jpg" | "jpeg" | "dds" => "IMG",
        "vert" | "frag" | "wgsl" | "glsl" => "SH",
        "rs" | "py" | "bat" | "cmd" => "SRC",
        "" => "AS",
        _ => "AS",
    }
}
