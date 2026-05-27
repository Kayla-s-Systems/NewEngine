#![forbid(unsafe_op_in_unsafe_fn)]

//! Optional Content Browser retained UI node runtime.
//!
//! Content Browser is not a byte owner, semantic parser, or reusable engine feature.
//! It is a tool-side consumer that composes:
//! - mounted VFS directory listings from AssetManager;
//! - provider-declared FileTypeRegistry descriptors;
//! - common ListFile manifests for `file@entry` dictionary browsing;
//! - a retained `engine.ui` surface.
//!
//! Reusable engine/runtime code provides assets and UI routing only. It does not
//! know, render, or special-case this browser.

use abi_stable::std_types::RString;
use newengine_assets::{AssetService, AssetServiceClient};
mod content_model;
use content_model::*;
use newengine_assets_api::{
    file_type_method,
    AssetDecodeRequest,
    AssetEntryManifest,
    AssetFileManifest,
    AssetFileTypeDescriptor,
    AssetFileTypeManifest,
    ASSET_LIST_FILE_MANIFEST_OUTPUT,
    ENGINE_ASSET_FILE_TYPES_SERVICE_ID,
};
use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_input_actions_api::{
    engine_action, InputActionDefinition, InputActionDispatchMode, InputActionFrame,
    InputActionListenerRegistration, InputFrameSource,
};
use newengine_input_api::{engine_default_keybind, key_identity};
use newengine_input_bindings_api::{InputBinding, InputBindingRegistration, InputKeyRegistration};
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_ui_api::{
    UiComponentNode, UiInputCaptureState, UiInputFrame, UiNodeTone, UiSurfaceNode, UiSurfaceStyle,
    ENGINE_UI_SERVICE_ID, UI_COMPONENT_PANEL, UI_SERVICE_METHOD_SURFACE_NODE_V1,
    UI_SURFACE_EDITOR_CONTENT_BROWSER,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const CONTENT_BROWSER_UI_OWNER: &str = "engine.ui.content_browser";
const CONTENT_BROWSER_DOCUMENT_REF: &str = "assets/ui/editor/content_browser.neui@surface";
const CONTENT_BROWSER_STYLE_REF: &str = "assets/ui/editor/content_browser.neui@layout.main";
const CONTENT_BROWSER_THEME_REF: &str = "assets/ui/themes/take_some_default.neui@take_some.default";
const CONTENT_BROWSER_INPUT_OWNER: &str = "engine.ui.content_browser";
const CONTENT_BROWSER_INPUT_LISTENER: &str = "content-browser-node";

#[derive(Clone)]
pub struct ContentBrowserRuntimeState {
    host: HostApiV1,
    client: AssetServiceClient,
}

impl ContentBrowserRuntimeState {
    #[inline]
    pub fn new(host: HostApiV1, client: AssetServiceClient) -> Self { Self { host, client } }
}

/// Tool-owned retained UI module for the Content Browser surface.
///
/// The engine does not know that this surface is a content browser. The module
/// samples semantic input every frame, publishes a generic `UiInputCaptureState`,
/// and sends a retained `UiSurfaceNode` through `engine.ui` when the tool is open.
pub struct ContentBrowserUiRuntimeModule {
    state: ContentBrowserRuntimeState,
    open: bool,
    last_refresh_frame: u64,
    last_toggle_frame: u64,
    last_published_open: bool,
    input_registered: bool,
    last_input_registration_frame: Option<u64>,
    cached_node: Option<UiSurfaceNode>,
}

impl ContentBrowserUiRuntimeModule {
    #[inline]
    pub fn new() -> Self {
        let host = newengine_plugin_host::default_host_api();
        Self::with_host(host)
    }

    #[inline]
    pub fn with_host(host: HostApiV1) -> Self {
        let client = AssetServiceClient::new(host.clone());
        Self {
            state: ContentBrowserRuntimeState::new(host, client),
            open: false,
            last_refresh_frame: 0,
            last_toggle_frame: u64::MAX,
            last_published_open: false,
            input_registered: false,
            last_input_registration_frame: None,
            cached_node: None,
        }
    }

    fn publish_surface(&mut self, node: UiSurfaceNode) {
        let payload = match serde_json::to_vec(&node) {
            Ok(payload) => payload,
            Err(error) => {
                log::warn!("content browser UI node: surface serialization failed: {error}");
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
                log::debug!("content browser UI node: engine.ui route unavailable; surface skipped");
            }
            Err(error) => {
                log::warn!("content browser UI node: publish surface failed: {error}");
            }
        }
    }

    fn refresh_node(&mut self, frame_index: u64) -> UiSurfaceNode {
        match snapshot(&mut self.state) {
            Ok(snapshot) => content_browser_workspace_node(frame_index, &snapshot),
            Err(error) => content_browser_error_node(frame_index, error),
        }
    }
}

impl Default for ContentBrowserUiRuntimeModule {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Send + 'static> Module<E> for ContentBrowserUiRuntimeModule {
    fn id(&self) -> &'static str {
        "engine.ui.content_browser.node"
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
                self.input_registered = ensure_content_browser_input_registration();
            }
        }

        let input = ctx.resources().get::<UiInputFrame>().cloned().unwrap_or_default();
        let actions = resolve_content_browser_actions(&input);
        let toggled = action_frame_contains(&actions, engine_action::CONTENT_BROWSER_TOGGLE);

        if toggled && self.last_toggle_frame != frame_index {
            self.open = !self.open;
            self.last_toggle_frame = frame_index;
            self.cached_node = None;
            log::info!("content browser UI node: visibility changed open={}", self.open);
        }

        if self.open {
            let stale = frame_index.saturating_sub(self.last_refresh_frame) >= 30;
            if stale || self.cached_node.is_none() || self.last_toggle_frame == frame_index {
                let node = self.refresh_node(frame_index);
                self.last_refresh_frame = frame_index;
                self.cached_node = Some(node);
            }
            // Retained UI providers are allowed to rebuild/replace their draw packet
            // cache at any time. While a tool is open, publish the current state
            // every frame so the browser cannot vanish until the next toggle.
            if let Some(node) = self.cached_node.clone() {
                self.publish_surface(node);
            }
            ctx.resources_mut().insert(UiInputCaptureState::modal(
                UI_SURFACE_EDITOR_CONTENT_BROWSER,
                "content browser UI node modal capture",
            ));
        } else {
            if self.last_published_open || self.last_toggle_frame == frame_index {
                self.publish_surface(UiSurfaceNode::hidden(
                    UI_SURFACE_EDITOR_CONTENT_BROWSER,
                    CONTENT_BROWSER_UI_OWNER,
                ));
                let mut release = UiInputCaptureState::none();
                release.draw_refresh_requested = true;
                release.surfaces.push(UI_SURFACE_EDITOR_CONTENT_BROWSER.to_owned());
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

fn resolve_content_browser_actions(input: &UiInputFrame) -> InputActionFrame {
    newengine_input_bindings_runtime::resolve_input_actions(&UiInputSource(input))
}

fn action_frame_contains(actions: &InputActionFrame, action: &str) -> bool {
    actions.actions.iter().any(|it| it == action)
        || actions.events.iter().any(|event| event.action == action)
}


fn ensure_content_browser_input_registration() -> bool {
    let mut ok = true;
    if let Err(error) = newengine_input_bindings_runtime::register_input_key(
        InputKeyRegistration::new(
            engine_default_keybind::CONTENT_BROWSER_TOGGLE,
            key_identity::F1,
            "F1",
        ),
    ) {
        log::warn!("content browser UI node: input key registration failed: {error}");
        ok = false;
    }

    if let Err(error) = newengine_input_bindings_runtime::register_input_action(
        InputActionDefinition::new(engine_action::CONTENT_BROWSER_TOGGLE)
            .with_dispatch(InputActionDispatchMode::ConsumeFirst)
            .with_label("Toggle Content Browser"),
    ) {
        log::warn!("content browser UI node: input action registration failed: {error}");
        ok = false;
    }

    let profile = newengine_input_bindings_runtime::input_bindings_profile_snapshot();
    let has_binding = profile
        .bindings
        .iter()
        .any(|binding| binding.action == engine_action::CONTENT_BROWSER_TOGGLE);
    if !has_binding {
        let registration = InputBindingRegistration::new(InputBinding::keyboard_pressed(
            engine_action::CONTENT_BROWSER_TOGGLE,
            engine_default_keybind::CONTENT_BROWSER_TOGGLE,
        ));
        if let Err(error) = newengine_input_bindings_runtime::register_input_binding(registration) {
            log::warn!("content browser UI node: input binding registration failed: {error}");
            ok = false;
        }
    }

    if let Err(error) = newengine_input_bindings_runtime::register_input_listener(
        InputActionListenerRegistration::new(CONTENT_BROWSER_INPUT_OWNER, CONTENT_BROWSER_INPUT_LISTENER)
            .with_actions([engine_action::CONTENT_BROWSER_TOGGLE])
            .with_priority(90)
            .consuming(),
    ) {
        log::warn!("content browser UI node: input listener registration failed: {error}");
        ok = false;
    }

    if ok {
        log::info!(
            "content browser UI node: input listener registered owner='{}' listener='{}' action='{}' key='F1'",
            CONTENT_BROWSER_INPUT_OWNER,
            CONTENT_BROWSER_INPUT_LISTENER,
            engine_action::CONTENT_BROWSER_TOGGLE,
        );
    }
    ok
}

fn content_browser_workspace_node(
    frame_index: u64,
    snapshot: &ContentBrowserSnapshotResponse,
) -> UiSurfaceNode {
    let root = &snapshot.root;
    let logical = root.location.logical_path.trim_matches('/');
    let logical_label = if logical.is_empty() { "/".to_owned() } else { format!("/Content/{logical}") };

    let mut lines = Vec::new();
    lines.push("Content Browser".to_owned());
    lines.push(format!("Path: {logical_label}"));
    lines.push(format!(
        "{} folders · {} assets · {} ListFile entries · {} mounted sources",
        root.folders.len(),
        root.assets.len(),
        root.entries.len(),
        snapshot.sources.len(),
    ));
    lines.push("Folders".to_owned());
    for folder in root.folders.iter().take(12) {
        lines.push(format!("  {}", folder.name));
    }
    lines.push("Assets".to_owned());
    for asset in root.assets.iter().take(32) {
        let ext = asset.extension.as_deref().unwrap_or("-");
        lines.push(format!("  {}  .{}  {}", asset.name, ext, asset.asset_kind));
    }
    if !root.entries.is_empty() {
        lines.push("ListFile Entries".to_owned());
        for entry in root.entries.iter().take(18) {
            lines.push(format!("  {}", entry.entry_ref.as_deref().unwrap_or(&entry.name)));
        }
    }

    let mut components = Vec::new();
    components.push(
        UiComponentNode::row("ab.header", "Content Browser")
            .with_tone(UiNodeTone::Accent)
            .tagged("hero"),
    );
    components.push(
        UiComponentNode::row("ab.toolbar", "Add  Import  Save All  Refresh  Filters  Settings")
            .tagged("toolbar"),
    );
    components.push(
        UiComponentNode::row("ab.path", logical_label.clone())
            .tagged("breadcrumb")
            .with_detail("engine.assets / VFS / ListFile"),
    );

    for (idx, folder) in root.folders.iter().take(24).enumerate() {
        let mut component = UiComponentNode::row(format!("ab.folder.{idx:03}"), folder.name.clone())
            .tagged("folder")
            .with_detail(folder.logical_path.clone());
        if idx == 0 {
            component = component.tagged("selected");
        }
        components.push(component);
    }
    if root.folders.is_empty() {
        components.push(
            UiComponentNode::row("ab.folder.empty", "No folders")
                .tagged("folder")
                .with_tone(UiNodeTone::Disabled),
        );
    }

    for (idx, asset) in root.assets.iter().take(64).enumerate() {
        let ext = asset.extension.as_deref().unwrap_or("-");
        let gateway = asset.semantic_gateway.as_deref().unwrap_or("engine.assets");
        components.push(
            UiComponentNode::row(format!("ab.asset.{idx:03}"), asset.name.clone())
                .tagged("asset-card")
                .with_value(format!("{} · .{}", asset.asset_kind, ext))
                .with_detail(gateway.to_owned())
                .with_tone(if idx == 0 { UiNodeTone::Accent } else { UiNodeTone::Normal })
                .with_icon(icon_for_extension(ext)),
        );
    }
    if root.assets.is_empty() {
        components.push(
            UiComponentNode::row("ab.asset.empty", "No assets in this location")
                .tagged("asset-card")
                .with_tone(UiNodeTone::Disabled),
        );
    }

    for (idx, entry) in root.entries.iter().take(32).enumerate() {
        components.push(
            UiComponentNode::row(format!("ab.entry.{idx:03}"), entry.name.clone())
                .tagged("entry-card")
                .with_value(entry.asset_kind.clone())
                .with_detail(entry.entry_ref.clone().unwrap_or_default())
                .with_icon("@"),
        );
    }

    if !snapshot.warnings.is_empty() || !root.warnings.is_empty() {
        for (idx, warning) in snapshot.warnings.iter().chain(root.warnings.iter()).take(6).enumerate() {
            components.push(
                UiComponentNode::row(format!("ab.warn.{idx}"), warning.clone())
                    .tagged("warning")
                    .with_tone(UiNodeTone::Danger),
            );
        }
    }

    let mut node = UiSurfaceNode::new(UI_SURFACE_EDITOR_CONTENT_BROWSER, CONTENT_BROWSER_UI_OWNER)
        .with_title("Content Browser")
        .with_subtitle("retained UI node over engine.assets")
        .with_body_lines(lines)
        .with_footer_lines(vec![
            format!("{} folders  ·  {} assets  ·  {} ListFile entries", root.folders.len(), root.assets.len(), root.entries.len()),
            "F1 toggles Content Browser  ·  engine.ui retained node".to_owned(),
        ])
        .with_theme(CONTENT_BROWSER_THEME_REF)
        .with_style_ref(CONTENT_BROWSER_STYLE_REF)
        .with_style(content_browser_surface_style())
        .with_component(UI_COMPONENT_PANEL)
        .with_components(components)
        .with_metric("frame_index", json!(frame_index))
        .with_metric("folder_count", json!(root.folders.len()))
        .with_metric("asset_count", json!(root.assets.len()))
        .with_metric("entry_count", json!(root.entries.len()))
        .with_metric("logical_path", json!(logical_label))
        .with_metric("document_ref", json!(CONTENT_BROWSER_DOCUMENT_REF))
        .with_metric("style_ref", json!(CONTENT_BROWSER_STYLE_REF));
    node.modal = true;
    node.z_order = 970;
    node.style_tags = vec![
        "tool".to_owned(),
        "workspace".to_owned(),
        "content-browser".to_owned(),
        "modern".to_owned(),
        "rounded".to_owned(),
    ];
    node
}

fn content_browser_error_node(frame_index: u64, error: String) -> UiSurfaceNode {
    let mut node = UiSurfaceNode::new(UI_SURFACE_EDITOR_CONTENT_BROWSER, CONTENT_BROWSER_UI_OWNER)
        .with_title("Content Browser")
        .with_subtitle("UI node over engine.assets")
        .with_body_lines(vec![
            "Content Browser UI node failed to build its snapshot.".to_owned(),
            error,
            "The engine still provides asset data; this UI node owns only presentation.".to_owned(),
        ])
        .with_footer_lines(vec!["listener alive = invariant | engine.ui owns rendering".to_owned()])
        .with_theme(CONTENT_BROWSER_THEME_REF)
        .with_style_ref(CONTENT_BROWSER_STYLE_REF)
        .with_style(content_browser_surface_style())
        .with_component(UI_COMPONENT_PANEL)
        .with_metric("frame_index", json!(frame_index))
        .with_metric("document_ref", json!(CONTENT_BROWSER_DOCUMENT_REF))
        .with_metric("style_ref", json!(CONTENT_BROWSER_STYLE_REF));
    node.modal = true;
    node.z_order = 970;
    node.style_tags = vec!["tool".to_owned(), "workspace".to_owned(), "content-browser".to_owned(), "error".to_owned()];
    node
}

fn content_browser_surface_style() -> UiSurfaceStyle {
    let mut style = UiSurfaceStyle::default();
    style.min_size_px = [1140.0, 690.0];
    style.max_size_px = [4096.0, 4096.0];
    style.margin_px = [26.0, 26.0];
    style.padding_px = [34.0, 110.0, 34.0, 62.0];
    style.row_pitch_px = 26.0;
    style.panel_rgba = [5, 8, 15, 246];
    style.panel_header_rgba = [13, 18, 32, 246];
    style.accent_rgba = [102, 204, 255, 255];
    style.text_rgba = [236, 244, 255, 255];
    style.text_muted_rgba = [158, 176, 202, 255];
    style.danger_rgba = [255, 142, 110, 255];
    style.border_rgba = [105, 182, 255, 72];
    style.backdrop_rgba = [0, 0, 0, 36];
    style.shadow_alpha = 0;
    style.corner_radius_px = 22.0;
    style.border_px = 1.0;
    style.font.stack = vec!["AureliaSans".to_owned(), "Inter".to_owned(), "Segoe UI".to_owned(), "NotoSans".to_owned()];
    style.font.title_px = 28.0;
    style.font.body_px = 16.0;
    style.font.secondary_px = 14.0;
    style.row_even_alpha = 12;
    style.row_odd_alpha = 5;
    style.normalized()
}

fn icon_for_extension(ext: &str) -> &'static str {
    match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
        "neui" => "UI",
        "ytd" => "TX",
        "ydd" => "MD",
        "ytyp" => "SC",
        "nemat" => "MT",
        "nepak" => "PK",
        "rs" => "RS",
        "json" => "JS",
        _ => "AS",
    }
}


fn snapshot(state: &mut ContentBrowserRuntimeState) -> Result<ContentBrowserSnapshotResponse, String> {
    let mut root = list_vfs(state, ContentBrowserListRequest::default())?;
    root.sources = sources_array(state);
    let (file_type_manifest, file_type_warnings) = file_type_manifest_value(state);
    let (formats, format_warnings) = formats_value(state);
    let sources = root.sources.clone();
    let mut warnings = Vec::new();
    warnings.extend(file_type_warnings);
    warnings.extend(format_warnings);
    Ok(ContentBrowserSnapshotResponse {
        ok: true,
        root,
        sources,
        file_type_manifest,
        formats,
        warnings,
        ..Default::default()
    })
}

fn list_vfs(
    state: &mut ContentBrowserRuntimeState,
    request: ContentBrowserListRequest,
) -> Result<ContentBrowserListResponse, String> {
    if let Some(entry) = request.entry.as_deref().map(str::trim).filter(|it| !it.is_empty()) {
        return open_entry(state, &request.logical_path, entry);
    }

    let logical_path = normalize_path(&request.logical_path);
    let listing = match state.client.vfs_list_json_v1(&logical_path) {
        Ok(value) => value,
        Err(error) => return Ok(fallback_catalog_response(state, &logical_path, error, &request)),
    };
    let descriptors = descriptor_map(state);
    let sources = sources_array(state);
    let mut response = ContentBrowserListResponse {
        ok: true,
        location: ContentBrowserLocation { logical_path: logical_path.clone(), entry: None, location_kind: "vfs_directory".to_owned() },
        breadcrumbs: breadcrumbs(&logical_path),
        sources,
        warnings: value_warnings(&listing),
        ..Default::default()
    };

    for entry in listing.get("entries").and_then(|v| v.as_array()).into_iter().flatten() {
        let mut node = node_from_vfs_entry(entry, &descriptors);
        if !request.include_hidden && node.name.starts_with('.') {
            continue;
        }
        if !request.query.trim().is_empty() && !node.name.to_ascii_lowercase().contains(&request.query.to_ascii_lowercase()) {
            continue;
        }
        if node.node_kind == "directory" {
            response.folders.push(node);
        } else {
            annotate_listfile_entry_count(state, &mut node, request.include_listfile_entries);
            response.assets.push(node);
        }
    }
    Ok(response)
}

fn fallback_catalog_response(
    state: &mut ContentBrowserRuntimeState,
    logical_path: &str,
    error: String,
    request: &ContentBrowserListRequest,
) -> ContentBrowserListResponse {
    let mut response = ContentBrowserListResponse {
        ok: true,
        location: ContentBrowserLocation {
            logical_path: normalize_path(logical_path),
            entry: None,
            location_kind: "asset_catalog_fallback".to_owned(),
        },
        breadcrumbs: breadcrumbs(logical_path),
        sources: sources_array(state),
        warnings: vec![format!(
            "VFS listing unavailable; showing registered asset catalog descriptors instead: {error}"
        )],
        ..Default::default()
    };

    let query = request.query.trim().to_ascii_lowercase();
    for desc in descriptor_map(state).into_values() {
        let logical = format!("catalog/*.{}", desc.extension);
        let node = node_from_descriptor_file(&logical, &desc);
        if query.is_empty()
            || node.name.to_ascii_lowercase().contains(&query)
            || node.asset_kind.to_ascii_lowercase().contains(&query)
            || desc.semantic_gateway.to_ascii_lowercase().contains(&query)
        {
            response.assets.push(node);
        }
    }
    response.assets.sort_by(|a, b| a.name.cmp(&b.name));
    response
}

fn open_target(
    state: &mut ContentBrowserRuntimeState,
    request: ContentBrowserOpenRequest,
) -> Result<ContentBrowserListResponse, String> {
    let target = normalize_path(&request.target_ref);
    let location = ContentBrowserLocation::from_ref(&target);
    if let Some(entry) = location.entry.as_deref() {
        return open_entry(state, &location.logical_path, entry);
    }

    let descriptors = descriptor_map(state);
    let descriptor = extension_of(&location.logical_path).and_then(|ext| descriptors.get(&ext).cloned());
    let mode = request.mode.trim();
    let should_open_as_listfile = mode == "listFile"
        || mode == "listfile"
        || descriptor.as_ref().map(is_listfile_descriptor).unwrap_or(false);

    if should_open_as_listfile {
        match open_listfile_manifest(state, &location.logical_path) {
            Ok(response) => return Ok(response),
            Err(e) if mode.eq_ignore_ascii_case("auto") || mode.is_empty() => {
                let mut request = ContentBrowserListRequest::default();
                request.logical_path = location.logical_path;
                let mut response = list_vfs(state, request)?;
                response.warnings.push(format!("open as ListFile failed: {e}"));
                return Ok(response);
            }
            Err(e) => return Err(e),
        }
    }

    let mut request = ContentBrowserListRequest::default();
    request.logical_path = location.logical_path;
    list_vfs(state, request)
}

fn open_entry(
    state: &mut ContentBrowserRuntimeState,
    logical_path: &str,
    entry: &str,
) -> Result<ContentBrowserListResponse, String> {
    let parent = open_listfile_manifest(state, logical_path)?;
    let entry_ref = format!("{}@{}", normalize_path(logical_path), entry.trim());
    let mut filtered = parent;
    filtered.location = ContentBrowserLocation { logical_path: normalize_path(logical_path), entry: Some(entry.trim().to_owned()), location_kind: "listfile_entry".to_owned() };
    filtered.entries.retain(|node| node.entry_ref.as_deref() == Some(entry_ref.as_str()) || node.name == entry.trim());
    if filtered.entries.is_empty() {
        filtered.warnings.push(format!("ListFile entry not found: {entry_ref}"));
    }
    Ok(filtered)
}

fn open_listfile_manifest(
    state: &mut ContentBrowserRuntimeState,
    logical_path: &str,
) -> Result<ContentBrowserListResponse, String> {
    let logical_path = normalize_path(logical_path);
    let request = AssetDecodeRequest {
        logical_path: logical_path.clone(),
        output_kind: ASSET_LIST_FILE_MANIFEST_OUTPUT.to_owned(),
        selector: Value::Null,
    };
    let bytes = state.client.decode_v1(&request)?;
    let manifest: AssetFileManifest = serde_json::from_slice(&bytes)
        .map_err(|e| format!("ListFile manifest parse failed for '{}': {e}", logical_path))?;
    Ok(response_from_manifest(state, logical_path, manifest))
}

fn response_from_manifest(
    state: &mut ContentBrowserRuntimeState,
    logical_path: String,
    manifest: AssetFileManifest,
) -> ContentBrowserListResponse {
    let descriptors = descriptor_map(state);
    let mut response = ContentBrowserListResponse {
        ok: true,
        location: ContentBrowserLocation { logical_path: logical_path.clone(), entry: None, location_kind: "listfile".to_owned() },
        breadcrumbs: breadcrumbs(&logical_path),
        sources: sources_array(state),
        warnings: manifest.warnings.clone(),
        ..Default::default()
    };
    if let Some(desc) = extension_of(&logical_path).and_then(|ext| descriptors.get(&ext).cloned()) {
        response.assets.push(node_from_descriptor_file(&logical_path, &desc));
    }
    for entry in manifest.entries {
        response.entries.push(node_from_manifest_entry(&logical_path, entry));
    }
    response
}

fn node_from_vfs_entry(
    value: &Value,
    descriptors: &BTreeMap<String, AssetFileTypeDescriptor>,
) -> ContentBrowserNode {
    let name = value.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_owned();
    let logical_path = value.get("path").and_then(|v| v.as_str()).unwrap_or_default().to_owned();
    let kind = value.get("kind").and_then(|v| v.as_str()).unwrap_or("file");
    let mut node = ContentBrowserNode {
        name,
        logical_path: logical_path.clone(),
        node_kind: if kind == "directory" { "directory".to_owned() } else { "asset".to_owned() },
        asset_kind: if kind == "directory" { "folder".to_owned() } else { "asset".to_owned() },
        byte_len: value.get("byte_len").and_then(|v| v.as_u64()),
        source_kind: value.get("source_kind").and_then(|v| v.as_str()).map(str::to_owned),
        source_index: value.get("source_index").and_then(|v| v.as_u64()).map(|v| v as usize),
        mount: value.get("mount").and_then(|v| v.as_str()).map(str::to_owned),
        priority: value.get("priority").and_then(|v| v.as_i64()).map(|v| v as i32),
        has_children: kind == "directory",
        can_preview: kind != "directory",
        can_rebuild: false,
        ..Default::default()
    };
    if let Some(ext) = extension_of(&logical_path) {
        node.extension = Some(ext.clone());
        if let Some(desc) = descriptors.get(&ext) {
            apply_descriptor(&mut node, desc);
        }
    }
    node
}

fn node_from_descriptor_file(logical_path: &str, desc: &AssetFileTypeDescriptor) -> ContentBrowserNode {
    let name = logical_path.rsplit('/').next().unwrap_or(logical_path).to_owned();
    let mut node = ContentBrowserNode {
        name,
        logical_path: logical_path.to_owned(),
        node_kind: "asset".to_owned(),
        asset_kind: desc.asset_kind.clone(),
        extension: Some(desc.extension.clone()),
        semantic_gateway: Some(desc.semantic_gateway.clone()),
        handler_service: Some(desc.handler_service.clone()),
        has_children: is_listfile_descriptor(desc) || desc.allow_nested_assets,
        can_open: true,
        can_preview: true,
        can_rebuild: is_listfile_descriptor(desc),
        ..Default::default()
    };
    node.metadata.insert("container".to_owned(), desc.container.clone());
    node.metadata.insert("codec_type".to_owned(), desc.codec_type.clone());
    node
}

fn node_from_manifest_entry(logical_path: &str, entry: AssetEntryManifest) -> ContentBrowserNode {
    let entry_ref = if entry.entry_ref.trim().is_empty() {
        format!("{}@{}", logical_path, entry.name)
    } else {
        entry.entry_ref.clone()
    };
    let mut metadata = entry.metadata.clone();
    if !entry.stable_id.trim().is_empty() {
        metadata.insert("stable_id".to_owned(), entry.stable_id.clone());
    }
    if !entry.dependencies.is_empty() {
        metadata.insert("dependency_count".to_owned(), entry.dependencies.len().to_string());
    }
    ContentBrowserNode {
        name: entry.name,
        logical_path: logical_path.to_owned(),
        entry_ref: Some(entry_ref),
        node_kind: "listfile_entry".to_owned(),
        asset_kind: entry.asset_kind,
        route_gateway: Some(entry.route.gateway),
        route_method: Some(entry.route.method),
        semantic_gateway: Some(entry.route.semantic_owner),
        has_children: false,
        can_open: true,
        can_preview: true,
        can_rename: true,
        can_delete: true,
        can_update: true,
        can_rebuild: true,
        metadata,
        warnings: vec!["entry mutation routes to AssetManager NEF8 ListFile repack/write-back; read-only container-backed sources reject destructive writes until a package writer is active".to_owned()],
        ..Default::default()
    }
}

fn apply_descriptor(node: &mut ContentBrowserNode, desc: &AssetFileTypeDescriptor) {
    node.asset_kind = desc.asset_kind.clone();
    node.semantic_gateway = Some(desc.semantic_gateway.clone());
    node.handler_service = Some(desc.handler_service.clone());
    node.has_children = node.has_children || is_listfile_descriptor(desc) || desc.allow_nested_assets;
    node.can_rebuild = is_listfile_descriptor(desc);
    node.metadata.insert("container".to_owned(), desc.container.clone());
    node.metadata.insert("codec_type".to_owned(), desc.codec_type.clone());
    if let Some(selector) = desc.selector_syntax.as_ref() {
        node.metadata.insert("selector_syntax".to_owned(), selector.clone());
    }
}

fn is_listfile_descriptor(desc: &AssetFileTypeDescriptor) -> bool {
    desc.codec_type == newengine_assets_api::codec_type::LIST_FILE
        || desc.codec_type == newengine_assets_api::codec_type::LIST
        || desc.selector_syntax.as_deref().map(|it| it.contains('@')).unwrap_or(false)
}

fn annotate_listfile_entry_count(
    state: &mut ContentBrowserRuntimeState,
    node: &mut ContentBrowserNode,
    include_entries: bool,
) {
    if !include_entries || !node.has_children || node.node_kind == "directory" {
        return;
    }
    if let Ok(manifest) = decode_listfile_manifest_value(state, &node.logical_path) {
        let count = manifest.entries.len();
        node.metadata.insert("entry_count".to_owned(), count.to_string());
    }
}

fn decode_listfile_manifest_value(
    state: &mut ContentBrowserRuntimeState,
    logical_path: &str,
) -> Result<AssetFileManifest, String> {
    let request = AssetDecodeRequest {
        logical_path: normalize_path(logical_path),
        output_kind: ASSET_LIST_FILE_MANIFEST_OUTPUT.to_owned(),
        selector: Value::Null,
    };
    let bytes = state.client.decode_v1(&request)?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

fn descriptor_map(state: &mut ContentBrowserRuntimeState) -> BTreeMap<String, AssetFileTypeDescriptor> {
    let (manifest, _) = file_type_manifest_value(state);
    serde_json::from_value::<AssetFileTypeManifest>(manifest)
        .map(|manifest| {
            manifest
                .formats
                .into_iter()
                .map(|desc| (desc.extension.clone(), desc))
                .collect()
        })
        .unwrap_or_default()
}

fn file_type_manifest_value(state: &mut ContentBrowserRuntimeState) -> (Value, Vec<String>) {
    let result = (state.host.call_service_v1)(
        RString::from(ENGINE_ASSET_FILE_TYPES_SERVICE_ID),
        MethodName::from(file_type_method::MANIFEST_JSON_V1),
        Blob::from(Vec::<u8>::new()),
    );
    match result.into_result() {
        Ok(blob) => match serde_json::from_slice::<Value>(blob.as_slice()) {
            Ok(value) => (value, Vec::new()),
            Err(e) => (Value::Null, vec![format!("file type manifest parse failed: {e}")]),
        },
        Err(e) => (Value::Null, vec![format!("file type manifest unavailable: {e}")]),
    }
}

fn formats_value(state: &mut ContentBrowserRuntimeState) -> (Value, Vec<String>) {
    match state.client.formats_json_v1() {
        Ok(value) => (value, Vec::new()),
        Err(e) => (Value::Null, vec![format!("asset formats unavailable: {e}")]),
    }
}

fn sources_array(state: &mut ContentBrowserRuntimeState) -> Vec<Value> {
    state
        .client
        .sources_json_v1()
        .ok()
        .and_then(|value| value.get("sources").and_then(|v| v.as_array()).cloned())
        .unwrap_or_default()
}

fn value_warnings(value: &Value) -> Vec<String> {
    value
        .get("warnings")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .map(|v| v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string()))
        .collect()
}

fn entry_update(state: &mut ContentBrowserRuntimeState, mut request: ContentBrowserEntryMutationRequest) -> ContentBrowserMutationResponse {
    if request.operation.trim().is_empty() {
        request.operation = "update".to_owned();
    }
    repack_via_asset_manager(state, serde_json::to_value(request).unwrap_or_default(), "update")
}

fn entry_delete(state: &mut ContentBrowserRuntimeState, mut request: ContentBrowserEntryMutationRequest) -> ContentBrowserMutationResponse {
    if request.operation.trim().is_empty() {
        request.operation = "delete".to_owned();
    }
    repack_via_asset_manager(state, serde_json::to_value(request).unwrap_or_default(), "delete")
}

fn rebuild_listfile(state: &mut ContentBrowserRuntimeState, request: ContentBrowserRebuildRequest) -> ContentBrowserMutationResponse {
    let mut value = serde_json::to_value(request).unwrap_or_default();
    if let Some(obj) = value.as_object_mut() {
        obj.insert("operation".to_owned(), Value::String("rebuild".to_owned()));
    }
    repack_via_asset_manager(state, value, "rebuild")
}

fn repack_via_asset_manager(
    state: &mut ContentBrowserRuntimeState,
    payload: Value,
    default_operation: &str,
) -> ContentBrowserMutationResponse {
    match state.client.list_file_repack_json_v1(payload.clone()) {
        Ok(value) => serde_json::from_value::<ContentBrowserMutationResponse>(value).unwrap_or_else(|e| {
            mutation_error_response(
                payload_target_ref(&payload),
                default_operation,
                format!("AssetManager repack response did not match ContentBrowserMutationResponse: {e}"),
            )
        }),
        Err(e) => mutation_error_response(payload_target_ref(&payload), default_operation, e),
    }
}

fn mutation_error_response(target_ref: String, operation: &str, error: String) -> ContentBrowserMutationResponse {
    let target_ref = normalize_path(&target_ref);
    ContentBrowserMutationResponse {
        ok: false,
        accepted: true,
        applied: false,
        target_ref: target_ref.clone(),
        logical_path: target_ref.split('@').next().unwrap_or_default().to_owned(),
        entry: target_ref.split_once('@').map(|(_, entry)| entry.to_owned()),
        operation: operation.to_owned(),
        transaction_id: format!("content-browser-repack-error:{}", stable_hash(&target_ref)),
        message: error,
        warnings: vec!["Content Browser did not mutate local UI state; source bytes remain authoritative in AssetManager VFS.".to_owned()],
        ..Default::default()
    }
}

fn payload_target_ref(value: &Value) -> String {
    value
        .get("target_ref")
        .or_else(|| value.get("logical_path"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_owned()
}

fn breadcrumbs(path: &str) -> Vec<ContentBrowserLocation> {
    let path = normalize_path(path);
    let mut out = vec![ContentBrowserLocation::default()];
    let mut cursor = String::new();
    for part in path.split('/').filter(|it| !it.is_empty()) {
        if !cursor.is_empty() {
            cursor.push('/');
        }
        cursor.push_str(part);
        out.push(ContentBrowserLocation { logical_path: cursor.clone(), entry: None, location_kind: "vfs_directory".to_owned() });
    }
    out
}

fn extension_of(path: &str) -> Option<String> {
    path
        .split('@')
        .next()
        .unwrap_or(path)
        .rsplit('.')
        .next()
        .map(str::trim)
        .filter(|ext| !ext.is_empty() && !ext.contains('/'))
        .map(|ext| ext.to_ascii_lowercase())
}

fn normalize_path(value: &str) -> String {
    let mut out = value.trim().replace('\\', "/");
    while let Some(rest) = out.strip_prefix("./") {
        out = rest.to_owned();
    }
    out = out.trim_start_matches('/').to_owned();
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    out
}

fn stable_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
