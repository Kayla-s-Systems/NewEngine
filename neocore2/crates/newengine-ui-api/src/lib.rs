#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

pub use newengine_ui_draw::{
    reserved, UiDrawCmd, UiDrawList, UiMesh, UiRect, UiTexId, UiTexture, UiTextureDelta,
    UiTexturePatch, UiVertex,
};
use std::collections::BTreeMap;

/// UI input snapshot consumed by engine UI surfaces and runtime overlays.
///
/// This type lives in `newengine-ui-api` so reusable runtime code can exchange
/// input with the UI domain without depending on a concrete UI implementation
/// crate or any provider package.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiInputFrame {
    pub keys_down: std::collections::BTreeSet<u32>,
    pub keys_pressed: std::collections::BTreeSet<u32>,
    pub keys_released: std::collections::BTreeSet<u32>,

    pub mouse_pos: Option<(f32, f32)>,
    pub mouse_delta: (f32, f32),
    pub mouse_wheel: (f32, f32),

    pub mouse_down: std::collections::BTreeSet<u32>,
    pub mouse_pressed: std::collections::BTreeSet<u32>,
    pub mouse_released: std::collections::BTreeSet<u32>,

    pub text: String,
    pub ime_preedit: String,
    pub ime_commit: String,

    pub gamepad_buttons: std::collections::BTreeMap<String, f32>,
    pub gamepad_buttons_pressed: std::collections::BTreeSet<String>,
    pub gamepad_buttons_released: std::collections::BTreeSet<String>,

    pub gamepad_axes: std::collections::BTreeMap<String, f32>,
    pub gamepad_connected: usize,
}

/// Tool/UI capture state published through Resources.
///
/// UI surfaces may gate camera navigation or gameplay movement, but they must not
/// stop raw input sampling/listeners. Runtime consumes this generic DTO without
/// knowing which tool or panel requested capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiInputCaptureState {
    pub sampling_alive: bool,
    pub camera_navigation_gated: bool,
    pub gameplay_movement_gated: bool,
    pub modal: bool,
    pub draw_refresh_requested: bool,
    pub reason: String,
    pub surfaces: Vec<String>,
}

impl Default for UiInputCaptureState {
    fn default() -> Self { Self::none() }
}

impl UiInputCaptureState {
    #[inline]
    pub fn none() -> Self {
        Self {
            sampling_alive: true,
            camera_navigation_gated: false,
            gameplay_movement_gated: false,
            modal: false,
            draw_refresh_requested: false,
            reason: "none".to_owned(),
            surfaces: Vec::new(),
        }
    }

    #[inline]
    pub fn modal(surface_id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            sampling_alive: true,
            camera_navigation_gated: true,
            gameplay_movement_gated: true,
            modal: true,
            draw_refresh_requested: true,
            reason: reason.into(),
            surfaces: vec![surface_id.into()],
        }
    }

    #[inline]
    pub fn requests_capture(&self) -> bool {
        self.modal || self.camera_navigation_gated || self.gameplay_movement_gated
    }

    #[inline]
    pub fn merged_with_primary_modal(mut self, primary_modal: bool) -> Self {
        self.sampling_alive = true;
        if primary_modal {
            self.modal = true;
            self.camera_navigation_gated = true;
            self.gameplay_movement_gated = true;
            if !self.surfaces.iter().any(|surface| surface == UI_SURFACE_ENGINE_PRIMARY) {
                self.surfaces.push(UI_SURFACE_ENGINE_PRIMARY.to_owned());
            }
            if self.reason == "none" || self.reason.trim().is_empty() {
                self.reason = "primary UI modal capture".to_owned();
            }
        }
        self
    }
}


impl UiInputFrame {
    #[inline]
    pub fn is_key_down(&self, key: u32) -> bool { self.keys_down.contains(&key) }

    #[inline]
    pub fn is_key_pressed(&self, key: u32) -> bool { self.keys_pressed.contains(&key) }

    #[inline]
    pub fn is_mouse_down(&self, btn: u32) -> bool { self.mouse_down.contains(&btn) }

    #[inline]
    pub fn is_mouse_pressed(&self, btn: u32) -> bool { self.mouse_pressed.contains(&btn) }

    #[inline]
    pub fn has_gamepad_connected(&self) -> bool { self.gamepad_connected > 0 }

    #[inline]
    pub fn has_gamepad_activity(&self) -> bool {
        self.gamepad_buttons.values().any(|v| v.abs() > 0.5)
            || self.gamepad_axes.values().any(|v| v.abs() > 0.05)
            || !self.gamepad_buttons_pressed.is_empty()
            || !self.gamepad_buttons_released.is_empty()
    }

    #[inline]
    pub fn is_gamepad_button_down(&self, button: &str) -> bool {
        self.gamepad_buttons.get(button).copied().unwrap_or(0.0) > 0.5
    }

    #[inline]
    pub fn is_gamepad_button_pressed(&self, button: &str) -> bool {
        self.gamepad_buttons_pressed.contains(button)
    }

    #[inline]
    pub fn is_gamepad_button_released(&self, button: &str) -> bool {
        self.gamepad_buttons_released.contains(button)
    }

    #[inline]
    pub fn gamepad_axis(&self, axis: &str) -> f32 {
        self.gamepad_axes.get(axis).copied().unwrap_or(0.0)
    }
}

/// Canonical keyboard ids consumed by editor/UI code.
pub mod keys {
    pub use newengine_input_api::key_code::*;
}

mod frame_binary;
pub use frame_binary::{
    decode_ui_frame_request_bin, decode_ui_frame_response_bin, encode_ui_frame_request_bin,
    encode_ui_frame_response_bin,
};

/// Engine-facing UI service gateway id. Runtime consumers call this facade;
/// the host resolves it to the active UI provider by descriptor metadata.
pub const ENGINE_UI_SERVICE_ID: &str = "engine.ui";

/// Default/first-party provider service id for UI backends.
pub const UI_SERVICE_ID: &str = "ui.api";
pub const UI_BACKEND_CAPABILITY_ID: &str = "ui.backend";

pub const UI_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const UI_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const UI_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const UI_SERVICE_METHOD_SURFACE_MANIFEST_V1: &str = "surface_manifest_v1";
pub const UI_SERVICE_METHOD_SURFACE_CATALOG_V1: &str = "surface_catalog_v1";
pub const UI_SERVICE_METHOD_LAYOUT_MANIFEST_V1: &str = "layout_manifest_v1";
pub const UI_SERVICE_METHOD_ACTION_MANIFEST_V1: &str = "action_manifest_v1";
pub const UI_SERVICE_METHOD_LOADING_SHELL_V1: &str = "loading_shell_v1";
pub const UI_SERVICE_METHOD_DEBUG_TELEMETRY_SCHEMA: &str = "debug_telemetry_schema";
/// Generic retained UI surface/node state. Runtime publishes state only; provider owns layout/rendering.
pub const UI_SERVICE_METHOD_SURFACE_NODE_V1: &str = "ui.surface_node_v1";
pub const UI_SERVICE_METHOD_DRAW_FRAME_V1: &str = "draw_frame_v1";
pub const UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1: &str = "draw_frame_bin_v1";

/// Provider-owned XML document export for diagnostics/import tooling only.
/// Runtime UI should mount compiled `.neui` documents through the methods below.
pub const UI_SERVICE_METHOD_DOCUMENT_XML_V1: &str = "document_xml_v1";

/// Canonical live runtime UI methods. `engine.assets.ui` compiles `.neui` entries;
/// `engine.ui` mounts, patches, routes input/actions and emits draw packets.
pub const UI_SERVICE_METHOD_REGISTRY_LOAD_V1: &str = "ui.registry_load_v1";
pub const UI_SERVICE_METHOD_MOUNT_SURFACE_V1: &str = "ui.mount_surface_v1";
pub const UI_SERVICE_METHOD_UNMOUNT_SURFACE_V1: &str = "ui.unmount_surface_v1";
pub const UI_SERVICE_METHOD_SET_SURFACE_VISIBLE_V1: &str = "ui.set_surface_visible_v1";
pub const UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1: &str = "ui.apply_state_patch_v1";
pub const UI_SERVICE_METHOD_DISPATCH_INPUT_V1: &str = "ui.dispatch_input_v1";
pub const UI_SERVICE_METHOD_DISPATCH_ACTION_V1: &str = "ui.dispatch_action_v1";
pub const UI_SERVICE_METHOD_NAVIGATE_V1: &str = "ui.navigate_v1";
pub const UI_SERVICE_METHOD_DEBUG_TREE_V1: &str = "ui.debug_tree_v1";
pub const UI_SERVICE_METHOD_DEBUG_BINDINGS_V1: &str = "ui.debug_bindings_v1";

pub const UI_SURFACE_ENGINE_LOADING: &str = "engine.loading";
pub const UI_SURFACE_ENGINE_ERROR_MODAL: &str = "engine.error_modal";
pub const UI_SURFACE_RUNTIME_OVERLAY: &str = "runtime.overlay";
pub const UI_SURFACE_RUNTIME_DEBUG_OVERLAY: &str = "runtime.debug_overlay";
pub const UI_SURFACE_ENGINE_PRIMARY: &str = "engine.ui.primary";
/// Canonical declarative UI theme id used by first-party runtime/editor surfaces.
/// The engine treats this as a token; the active UI provider resolves fonts,
/// metrics and colors from its own theme registry.
pub const UI_THEME_NORTHSTAR_DEFAULT: &str = "northstar.default";

/// Generic component primitives. These are not screen types: every interface is
/// the same retained `UiSurfaceNode` tree and may compose the same primitives.
pub const UI_COMPONENT_SURFACE: &str = "surface";
pub const UI_COMPONENT_PANEL: &str = "panel";
pub const UI_COMPONENT_STACK: &str = "stack";
pub const UI_COMPONENT_ROW: &str = "row";
pub const UI_COMPONENT_TEXT: &str = "text";
pub const UI_COMPONENT_ACTION: &str = "action";
pub const UI_COMPONENT_SPACER: &str = "spacer";

/// Generic backend-family declaration for UI providers.
pub const UI_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "ui",
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_ID,
        UI_BACKEND_CAPABILITY_ID,
    );

/// Startup validation contract for the engine-facing UI gateway.
pub const UI_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_UI_SERVICE_ID,
        "newengine.ui-api >= 0.1.x",
        newengine_service_api::JSON_CONTROL_SERVICE_METHODS_V1,
    );

/// Declarative startup requirement for UI. Missing UI degrades unless strict
/// runtime profiles opt in through the explicit env switch.
pub const UI_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        UI_RUNTIME_CONTRACT_SPEC,
        Some(UI_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_UI_BACKEND"),
    );

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub surfaces: Vec<String>,
}

impl Default for UiServiceInfo {
    #[inline]
    fn default() -> Self {
        Self {
            protocol: "newengine.ui-api/v1".to_owned(),
            features: vec![
                "provider-owned-layout".to_owned(),
                "declarative-actions".to_owned(),
                "runtime-debug-node-projection".to_owned(),
                "surface-node-modal-v1".to_owned(),
                "surface-node-navigation-v1".to_owned(),
                "surface-node-action-routes-v1".to_owned(),
                "draw-frame-bin-v1".to_owned(),
                "atlas-text-quads".to_owned(),
                "ui-font-style-tokens-v1".to_owned(),
                "ui-theme-font-tokens-v1".to_owned(),
                "ui-component-catalog-v1".to_owned(),
                "retained-component-node-tree-v1".to_owned(),
                "same-foundation-ui-node-v1".to_owned(),
                "pixel-aligned-text-v1".to_owned(),
                "neui-compiled-document-mount-v1".to_owned(),
                "state-patch-bindings-v1".to_owned(),
                "retained-surface-node-v1".to_owned(),
                "debug-tree-v1".to_owned(),
                "debug-bindings-v1".to_owned(),
            ],
            methods: ui_service_methods().iter().map(|it| (*it).to_owned()).collect(),
            surfaces: vec![
                UI_SURFACE_ENGINE_LOADING.to_owned(),
                UI_SURFACE_ENGINE_ERROR_MODAL.to_owned(),
                UI_SURFACE_RUNTIME_OVERLAY.to_owned(),
                UI_SURFACE_RUNTIME_DEBUG_OVERLAY.to_owned(),
                UI_SURFACE_ENGINE_PRIMARY.to_owned(),
            ],
        }
    }
}

pub const UI_SERVICE_METHODS: &[&str] = &[
    UI_SERVICE_METHOD_INFO,
    UI_SERVICE_METHOD_INVOKE,
    UI_SERVICE_METHOD_SHUTDOWN_V1,
    UI_SERVICE_METHOD_SURFACE_MANIFEST_V1,
    UI_SERVICE_METHOD_SURFACE_CATALOG_V1,
    UI_SERVICE_METHOD_LAYOUT_MANIFEST_V1,
    UI_SERVICE_METHOD_ACTION_MANIFEST_V1,
    UI_SERVICE_METHOD_LOADING_SHELL_V1,
    UI_SERVICE_METHOD_DEBUG_TELEMETRY_SCHEMA,
    UI_SERVICE_METHOD_SURFACE_NODE_V1,
    UI_SERVICE_METHOD_DRAW_FRAME_V1,
    UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1,
    UI_SERVICE_METHOD_DOCUMENT_XML_V1,
    UI_SERVICE_METHOD_REGISTRY_LOAD_V1,
    UI_SERVICE_METHOD_MOUNT_SURFACE_V1,
    UI_SERVICE_METHOD_UNMOUNT_SURFACE_V1,
    UI_SERVICE_METHOD_SET_SURFACE_VISIBLE_V1,
    UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1,
    UI_SERVICE_METHOD_DISPATCH_INPUT_V1,
    UI_SERVICE_METHOD_DISPATCH_ACTION_V1,
    UI_SERVICE_METHOD_NAVIGATE_V1,
    UI_SERVICE_METHOD_DEBUG_TREE_V1,
    UI_SERVICE_METHOD_DEBUG_BINDINGS_V1,
];

#[inline]
pub const fn ui_service_methods() -> &'static [&'static str] {
    UI_SERVICE_METHODS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiInvokeEnvelope {
    pub method: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiFrameRequest {
    pub version: u32,
    pub frame_index: u64,
    pub dt_sec: f32,
    pub surface_size_px: [u32; 2],
    pub pixels_per_point: f32,
}

impl UiFrameRequest {
    #[inline]
    pub fn new(frame_index: u64, dt_sec: f32, surface_size_px: [u32; 2], pixels_per_point: f32) -> Self {
        Self {
            version: 1,
            frame_index,
            dt_sec,
            surface_size_px,
            pixels_per_point: pixels_per_point.max(0.0001),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiFrameResponse {
    pub version: u32,
    pub draw_list: UiDrawList,
}

impl UiFrameResponse {
    #[inline]
    pub fn new(draw_list: UiDrawList) -> Self {
        Self { version: 1, draw_list }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiAck {
    pub ok: bool,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

impl UiAck {
    #[inline]
    pub fn ok(provider: impl Into<String>) -> Self {
        Self { ok: true, provider: Some(provider.into()), message: None }
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiBindingMode {
    OneWay,
    TwoWay,
    Event,
}

impl Default for UiBindingMode {
    #[inline]
    fn default() -> Self { Self::OneWay }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiUpdatePolicy {
    Frame,
    Event,
    Dirty,
    OnChange,
    Manual,
}

impl Default for UiUpdatePolicy {
    #[inline]
    fn default() -> Self { Self::OnChange }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiStateSource {
    pub id: String,
    pub source: String,
    pub contract: String,
    pub update_policy: UiUpdatePolicy,
}

impl Default for UiStateSource {
    fn default() -> Self {
        Self { id: String::new(), source: String::new(), contract: String::new(), update_policy: UiUpdatePolicy::default() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiBindingEdge {
    pub element_id: String,
    pub property: String,
    pub source_id: String,
    pub path: String,
    pub mode: UiBindingMode,
    #[serde(default)]
    pub fallback: Option<String>,
    #[serde(default)]
    pub transform: Option<String>,
}

impl Default for UiBindingEdge {
    fn default() -> Self {
        Self {
            element_id: String::new(),
            property: String::new(),
            source_id: String::new(),
            path: String::new(),
            mode: UiBindingMode::default(),
            fallback: None,
            transform: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiActionEdge {
    pub element_id: String,
    pub trigger: String,
    pub action_id: String,
    pub target_gateway: String,
    pub command: String,
    #[serde(default)]
    pub payload_schema: Option<String>,
}

impl Default for UiActionEdge {
    fn default() -> Self {
        Self { element_id: String::new(), trigger: String::new(), action_id: String::new(), target_gateway: String::new(), command: String::new(), payload_schema: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiBindingPlan {
    pub document_ref: String,
    pub surface_id: String,
    pub state_sources: Vec<UiStateSource>,
    pub bindings: Vec<UiBindingEdge>,
    pub actions: Vec<UiActionEdge>,
}

impl Default for UiBindingPlan {
    fn default() -> Self {
        Self { document_ref: String::new(), surface_id: String::new(), state_sources: Vec::new(), bindings: Vec::new(), actions: Vec::new() }
    }
}


/// Runtime source kind for UI documents.
///
/// UI may come from compiled `.neui` assets, a runtime stream, or a generated
/// document, but all paths must end in the same compiled DTO/mount contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDocumentSourceKind {
    Asset,
    Stream,
    Generated,
}

impl Default for UiDocumentSourceKind {
    #[inline]
    fn default() -> Self { Self::Asset }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDocumentSource {
    pub kind: UiDocumentSourceKind,
    pub document_ref: String,
    pub style_ref: Option<String>,
    pub stream_id: Option<String>,
    pub generator_id: Option<String>,
}

impl Default for UiDocumentSource {
    #[inline]
    fn default() -> Self {
        Self {
            kind: UiDocumentSourceKind::Asset,
            document_ref: String::new(),
            style_ref: None,
            stream_id: None,
            generator_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiCompiledDocument {
    pub version: u32,
    pub source: UiDocumentSource,
    pub document_ref: String,
    pub surface_id: String,
    pub root_id: String,
    pub theme_ref: Option<String>,
    pub style_ref: Option<String>,
    pub dependencies: Vec<String>,
    pub style_dependencies: Vec<String>,
    pub binding_plan: UiBindingPlan,
}

impl Default for UiCompiledDocument {
    fn default() -> Self {
        Self {
            version: 1,
            source: UiDocumentSource::default(),
            document_ref: String::new(),
            surface_id: String::new(),
            root_id: String::new(),
            theme_ref: None,
            style_ref: None,
            dependencies: Vec::new(),
            style_dependencies: Vec::new(),
            binding_plan: UiBindingPlan::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiStateChange {
    pub source_id: String,
    pub path: String,
    pub value: serde_json::Value,
}

impl Default for UiStateChange {
    fn default() -> Self {
        Self { source_id: String::new(), path: String::new(), value: serde_json::Value::Null }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiStatePatch {
    pub frame_index: u64,
    pub surface_id: String,
    pub changes: Vec<UiStateChange>,
}

impl Default for UiStatePatch {
    fn default() -> Self { Self { frame_index: 0, surface_id: String::new(), changes: Vec::new() } }
}

impl UiStatePatch {
    #[inline]
    pub fn new(frame_index: u64, surface_id: impl Into<String>) -> Self {
        Self { frame_index, surface_id: surface_id.into(), changes: Vec::new() }
    }

    #[inline]
    pub fn with_change(mut self, source_id: impl Into<String>, path: impl Into<String>, value: serde_json::Value) -> Self {
        self.changes.push(UiStateChange { source_id: source_id.into(), path: path.into(), value });
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiFontStyle {
    /// Declarative font fallback stack. Providers may resolve this through
    /// `engine.ui.text`; the engine treats it as data, not concrete font files.
    pub stack: Vec<String>,
    /// Body text size in physical pixels before provider snapping.
    pub body_px: f32,
    /// Title text size in physical pixels before provider snapping.
    pub title_px: f32,
    /// Subtitle/footer text size in physical pixels before provider snapping.
    pub secondary_px: f32,
    /// Text baseline pitch in physical pixels. `0` means provider default.
    pub line_height_px: f32,
    /// Pixel-snap text quads to avoid blurred bitmap/atlas sampling.
    pub pixel_snap: bool,
}

impl Default for UiFontStyle {
    fn default() -> Self {
        Self {
            stack: vec!["AureliaSans".to_owned(), "Inter".to_owned(), "Segoe UI".to_owned(), "NotoSans".to_owned(), "NotoSansSymbols".to_owned()],
            body_px: 18.0,
            title_px: 30.0,
            secondary_px: 15.0,
            line_height_px: 0.0,
            pixel_snap: true,
        }
    }
}

impl UiFontStyle {
    #[inline]
    pub fn normalized(mut self) -> Self {
        if self.stack.is_empty() {
            self.stack = UiFontStyle::default().stack;
        }
        self.body_px = self.body_px.clamp(10.0, 48.0);
        self.title_px = self.title_px.clamp(14.0, 72.0);
        self.secondary_px = self.secondary_px.clamp(9.0, 36.0);
        self.line_height_px = self.line_height_px.clamp(0.0, 96.0);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiSurfaceAnchor {
    TopLeft,
    TopRight,
    Center,
    BottomLeft,
    BottomRight,
}

impl Default for UiSurfaceAnchor {
    #[inline]
    fn default() -> Self { Self::TopLeft }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSurfaceStyle {
    pub theme_id: String,
    pub font: UiFontStyle,
    pub accent_rgba: [u8; 4],
    pub panel_rgba: [u8; 4],
    pub panel_header_rgba: [u8; 4],
    pub text_rgba: [u8; 4],
    pub text_muted_rgba: [u8; 4],
    pub danger_rgba: [u8; 4],
    pub row_even_alpha: u8,
    pub row_odd_alpha: u8,
    pub shadow_alpha: u8,
    /// Rounded surface radius in physical pixels. Providers may approximate it
    /// when the active renderer has no signed-distance/AA UI shader yet.
    pub corner_radius_px: f32,
    /// Thin modern surface outline. This is a style token, not a hardcoded provider color.
    pub border_rgba: [u8; 4],
    pub border_px: f32,
    /// Modal backdrop tint. UI state owns whether it is active; provider only paints it.
    pub backdrop_rgba: [u8; 4],
    pub anchor: UiSurfaceAnchor,
    pub min_size_px: [f32; 2],
    pub max_size_px: [f32; 2],
    pub margin_px: [f32; 2],
    pub padding_px: [f32; 4],
    pub row_pitch_px: f32,
}

impl Default for UiSurfaceStyle {
    fn default() -> Self {
        Self {
            theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
            font: UiFontStyle::default(),
            accent_rgba: [255, 203, 76, 255],
            panel_rgba: [6, 8, 15, 248],
            panel_header_rgba: [10, 13, 24, 255],
            text_rgba: [236, 244, 255, 255],
            text_muted_rgba: [188, 202, 224, 255],
            danger_rgba: [255, 168, 122, 255],
            row_even_alpha: 18,
            row_odd_alpha: 8,
            shadow_alpha: 180,
            corner_radius_px: 18.0,
            border_rgba: [122, 157, 210, 54],
            border_px: 1.0,
            backdrop_rgba: [1, 3, 8, 118],
            anchor: UiSurfaceAnchor::TopLeft,
            min_size_px: [260.0, 120.0],
            max_size_px: [560.0, 420.0],
            margin_px: [12.0, 12.0],
            padding_px: [34.0, 98.0, 34.0, 58.0],
            row_pitch_px: 0.0,
        }
    }
}

impl UiSurfaceStyle {
    #[inline]
    pub fn normalized(mut self) -> Self {
        self.font = self.font.normalized();
        if self.theme_id.trim().is_empty() {
            self.theme_id = UI_THEME_NORTHSTAR_DEFAULT.to_owned();
        }
        self.min_size_px[0] = self.min_size_px[0].clamp(96.0, 4096.0);
        self.min_size_px[1] = self.min_size_px[1].clamp(64.0, 4096.0);
        self.max_size_px[0] = self.max_size_px[0].max(self.min_size_px[0]).clamp(96.0, 4096.0);
        self.max_size_px[1] = self.max_size_px[1].max(self.min_size_px[1]).clamp(64.0, 4096.0);
        self.margin_px[0] = self.margin_px[0].clamp(0.0, 512.0);
        self.margin_px[1] = self.margin_px[1].clamp(0.0, 512.0);
        for value in self.padding_px.iter_mut() {
            *value = value.clamp(0.0, 512.0);
        }
        self.row_pitch_px = self.row_pitch_px.clamp(0.0, 256.0);
        self.corner_radius_px = self.corner_radius_px.clamp(0.0, 64.0);
        self.border_px = self.border_px.clamp(0.0, 8.0);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFontRole {
    Title,
    Body,
    Secondary,
    Code,
    Icon,
}

impl Default for UiFontRole {
    #[inline]
    fn default() -> Self { Self::Body }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiThemeFontToken {
    pub id: String,
    pub role: UiFontRole,
    /// Provider-neutral family stack. A provider may map these names to real
    /// font assets through `engine.ui.text`; runtime never sees font files.
    pub family_stack: Vec<String>,
    pub size_px: f32,
    pub line_height_px: f32,
    pub weight: u16,
    pub pixel_snap: bool,
}

impl Default for UiThemeFontToken {
    fn default() -> Self {
        Self {
            id: "body".to_owned(),
            role: UiFontRole::Body,
            family_stack: UiFontStyle::default().stack,
            size_px: 18.0,
            line_height_px: 24.0,
            weight: 500,
            pixel_snap: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiThemeComponentStyle {
    pub component_id: String,
    pub font_token: String,
    pub min_size_px: [f32; 2],
    pub padding_px: [f32; 4],
    pub row_pitch_px: f32,
    pub interactive: bool,
    pub paint_layer: i32,
}

impl Default for UiThemeComponentStyle {
    fn default() -> Self {
        Self {
            component_id: UI_COMPONENT_PANEL.to_owned(),
            font_token: "body".to_owned(),
            min_size_px: [260.0, 120.0],
            padding_px: [28.0, 22.0, 28.0, 22.0],
            row_pitch_px: 26.0,
            interactive: false,
            paint_layer: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiThemeDefinition {
    pub id: String,
    pub display_name: String,
    pub default_component: String,
    pub fonts: BTreeMap<String, UiThemeFontToken>,
    pub components: BTreeMap<String, UiThemeComponentStyle>,
    pub base_style: UiSurfaceStyle,
}

impl Default for UiThemeDefinition {
    fn default() -> Self {
        let mut fonts = BTreeMap::new();
        fonts.insert("title".to_owned(), UiThemeFontToken {
            id: "title".to_owned(),
            role: UiFontRole::Title,
            size_px: 30.0,
            line_height_px: 36.0,
            weight: 700,
            ..UiThemeFontToken::default()
        });
        fonts.insert("body".to_owned(), UiThemeFontToken::default());
        fonts.insert("secondary".to_owned(), UiThemeFontToken {
            id: "secondary".to_owned(),
            role: UiFontRole::Secondary,
            size_px: 15.0,
            line_height_px: 20.0,
            weight: 500,
            ..UiThemeFontToken::default()
        });
        fonts.insert("code".to_owned(), UiThemeFontToken {
            id: "code".to_owned(),
            role: UiFontRole::Code,
            family_stack: vec!["AureliaMono".to_owned(), "CascadiaMono".to_owned(), "NotoSansMono".to_owned()],
            size_px: 16.0,
            line_height_px: 22.0,
            weight: 500,
            ..UiThemeFontToken::default()
        });

        let mut components = BTreeMap::new();
        components.insert(UI_COMPONENT_PANEL.to_owned(), UiThemeComponentStyle::default());
        components.insert(UI_COMPONENT_STACK.to_owned(), UiThemeComponentStyle {
            component_id: UI_COMPONENT_STACK.to_owned(),
            row_pitch_px: 26.0,
            ..UiThemeComponentStyle::default()
        });
        components.insert(UI_COMPONENT_ROW.to_owned(), UiThemeComponentStyle {
            component_id: UI_COMPONENT_ROW.to_owned(),
            row_pitch_px: 26.0,
            interactive: true,
            ..UiThemeComponentStyle::default()
        });
        components.insert(UI_COMPONENT_TEXT.to_owned(), UiThemeComponentStyle {
            component_id: UI_COMPONENT_TEXT.to_owned(),
            font_token: "body".to_owned(),
            ..UiThemeComponentStyle::default()
        });
        components.insert(UI_COMPONENT_ACTION.to_owned(), UiThemeComponentStyle {
            component_id: UI_COMPONENT_ACTION.to_owned(),
            font_token: "body".to_owned(),
            interactive: true,
            ..UiThemeComponentStyle::default()
        });
        components.insert(UI_COMPONENT_SPACER.to_owned(), UiThemeComponentStyle {
            component_id: UI_COMPONENT_SPACER.to_owned(),
            min_size_px: [1.0, 10.0],
            ..UiThemeComponentStyle::default()
        });

        Self {
            id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
            display_name: "North Star Default".to_owned(),
            default_component: UI_COMPONENT_PANEL.to_owned(),
            fonts,
            components,
            base_style: UiSurfaceStyle::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiNodeTone {
    Normal,
    Accent,
    Danger,
    Disabled,
}

impl Default for UiNodeTone {
    #[inline]
    fn default() -> Self { Self::Normal }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiNodeMessageSeverity {
    Info,
    Success,
    Warning,
    Danger,
}

impl Default for UiNodeMessageSeverity {
    #[inline]
    fn default() -> Self { Self::Info }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNodeMessage {
    pub title: String,
    pub detail: String,
    pub severity: UiNodeMessageSeverity,
    pub age_sec: f32,
    pub ttl_sec: f32,
}

impl Default for UiNodeMessage {
    fn default() -> Self {
        Self {
            title: String::new(),
            detail: String::new(),
            severity: UiNodeMessageSeverity::Info,
            age_sec: 0.0,
            ttl_sec: 2.2,
        }
    }
}

impl UiNodeMessage {
    #[inline]
    pub fn new(title: impl Into<String>, detail: impl Into<String>, severity: UiNodeMessageSeverity) -> Self {
        Self { title: title.into(), detail: detail.into(), severity, ..Self::default() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiComponentNode {
    pub id: String,
    pub component_id: String,
    pub text: String,
    pub value: Option<String>,
    pub detail: Option<String>,
    pub icon: Option<String>,
    pub font_token: Option<String>,
    pub tone: UiNodeTone,
    pub state_tags: Vec<String>,
    pub action_id: Option<String>,
    pub props: BTreeMap<String, serde_json::Value>,
    pub children: Vec<UiComponentNode>,
}

impl Default for UiComponentNode {
    fn default() -> Self {
        Self {
            id: String::new(),
            component_id: UI_COMPONENT_TEXT.to_owned(),
            text: String::new(),
            value: None,
            detail: None,
            icon: None,
            font_token: None,
            tone: UiNodeTone::Normal,
            state_tags: Vec::new(),
            action_id: None,
            props: BTreeMap::new(),
            children: Vec::new(),
        }
    }
}

impl UiComponentNode {
    #[inline]
    pub fn text(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self { id: id.into(), text: text.into(), ..Self::default() }
    }

    #[inline]
    pub fn row(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self { id: id.into(), component_id: UI_COMPONENT_ROW.to_owned(), text: text.into(), ..Self::default() }
    }

    #[inline]
    pub fn action(id: impl Into<String>, text: impl Into<String>, action_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            component_id: UI_COMPONENT_ACTION.to_owned(),
            text: text.into(),
            action_id: Some(action_id.into()),
            ..Self::default()
        }
    }

    #[inline]
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    #[inline]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    #[inline]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    #[inline]
    pub fn with_tone(mut self, tone: UiNodeTone) -> Self {
        self.tone = tone;
        self
    }

    #[inline]
    pub fn tagged(mut self, tag: impl Into<String>) -> Self {
        self.state_tags.push(tag.into());
        self
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiSurfaceAdmissionPolicy {
    /// Default retained-UI behavior: this surface does not block other surfaces
    /// from being created or opened.
    AcceptAll,
    /// While this surface is visible, the active UI provider must reject new
    /// visible surfaces with a different surface id. This is an explicit UI-node
    /// policy, not a provider-specific hardcoded branch.
    AcceptAllButThis,
}

impl Default for UiSurfaceAdmissionPolicy {
    #[inline]
    fn default() -> Self { Self::AcceptAll }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSurfaceNode {
    pub version: u32,
    pub surface_id: String,
    pub source: String,
    pub visible: bool,
    pub modal: bool,
    pub z_order: i32,
    pub title: String,
    pub subtitle: String,
    pub body_lines: Vec<String>,
    pub footer_lines: Vec<String>,
    pub style_tags: Vec<String>,
    pub theme_id: String,
    pub style_ref: Option<String>,
    pub component_id: String,
    pub components: Vec<UiComponentNode>,
    pub message: Option<UiNodeMessage>,
    pub style: UiSurfaceStyle,
    pub admission_policy: UiSurfaceAdmissionPolicy,
    pub metrics: BTreeMap<String, serde_json::Value>,
}

impl Default for UiSurfaceNode {
    fn default() -> Self {
        Self {
            version: 1,
            surface_id: String::new(),
            source: String::new(),
            visible: true,
            modal: false,
            z_order: 0,
            title: String::new(),
            subtitle: String::new(),
            body_lines: Vec::new(),
            footer_lines: Vec::new(),
            style_tags: Vec::new(),
            theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
            style_ref: None,
            component_id: UI_COMPONENT_PANEL.to_owned(),
            components: Vec::new(),
            message: None,
            style: UiSurfaceStyle::default(),
            admission_policy: UiSurfaceAdmissionPolicy::default(),
            metrics: BTreeMap::new(),
        }
    }
}

impl UiSurfaceNode {
    #[inline]
    pub fn new(surface_id: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            surface_id: surface_id.into(),
            source: source.into(),
            ..Self::default()
        }
    }

    #[inline]
    pub fn hidden(surface_id: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            visible: false,
            surface_id: surface_id.into(),
            source: source.into(),
            ..Self::default()
        }
    }

    #[inline]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    #[inline]
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    #[inline]
    pub fn with_body_lines(mut self, lines: Vec<String>) -> Self {
        self.body_lines = lines;
        self
    }

    #[inline]
    pub fn with_footer_lines(mut self, lines: Vec<String>) -> Self {
        self.footer_lines = lines;
        self
    }

    #[inline]
    pub fn with_theme(mut self, theme_id: impl Into<String>) -> Self {
        self.theme_id = theme_id.into();
        self.style.theme_id = self.theme_id.clone();
        self
    }

    #[inline]
    pub fn with_component(mut self, component_id: impl Into<String>) -> Self {
        self.component_id = component_id.into();
        self
    }

    #[inline]
    pub fn with_style_ref(mut self, style_ref: impl Into<String>) -> Self {
        self.style_ref = Some(style_ref.into());
        self
    }

    #[inline]
    pub fn with_components(mut self, components: Vec<UiComponentNode>) -> Self {
        self.components = components;
        self
    }

    #[inline]
    pub fn with_message(mut self, message: UiNodeMessage) -> Self {
        self.message = Some(message);
        self
    }

    #[inline]
    pub fn with_style(mut self, style: UiSurfaceStyle) -> Self {
        self.style = style.normalized();
        self.theme_id = self.style.theme_id.clone();
        self
    }

    #[inline]
    pub fn with_admission_policy(mut self, policy: UiSurfaceAdmissionPolicy) -> Self {
        self.admission_policy = policy;
        self
    }

    #[inline]
    pub fn with_metric(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metrics.insert(key.into(), value);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiRegistryLoadRequest {
    pub registry_ref: String,
}
impl Default for UiRegistryLoadRequest { fn default() -> Self { Self { registry_ref: String::new() } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiMountSurfaceRequest {
    pub surface_id: String,
    pub document: UiCompiledDocument,
    pub visible: bool,
}
impl Default for UiMountSurfaceRequest { fn default() -> Self { Self { surface_id: String::new(), document: UiCompiledDocument::default(), visible: true } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSurfaceRequest {
    pub surface_id: String,
}
impl Default for UiSurfaceRequest { fn default() -> Self { Self { surface_id: String::new() } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSurfaceVisibilityRequest {
    pub surface_id: String,
    pub visible: bool,
}
impl Default for UiSurfaceVisibilityRequest { fn default() -> Self { Self { surface_id: String::new(), visible: true } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDispatchInputRequest {
    pub surface_id: String,
    pub event: String,
    pub payload: serde_json::Value,
}
impl Default for UiDispatchInputRequest { fn default() -> Self { Self { surface_id: String::new(), event: String::new(), payload: serde_json::Value::Null } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDispatchActionRequest {
    pub surface_id: String,
    pub action_id: String,
    pub payload: serde_json::Value,
}
impl Default for UiDispatchActionRequest { fn default() -> Self { Self { surface_id: String::new(), action_id: String::new(), payload: serde_json::Value::Null } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNavigateRequest {
    pub surface_id: String,
    pub target: String,
}
impl Default for UiNavigateRequest { fn default() -> Self { Self { surface_id: String::new(), target: String::new() } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDebugTreeResponse {
    pub version: u32,
    pub surface_id: String,
    pub nodes: Vec<serde_json::Value>,
}
impl Default for UiDebugTreeResponse { fn default() -> Self { Self { version: 1, surface_id: String::new(), nodes: Vec::new() } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiDebugBindingsResponse {
    pub version: u32,
    pub surface_id: String,
    pub bindings: Vec<UiBindingEdge>,
    pub actions: Vec<UiActionEdge>,
}
impl Default for UiDebugBindingsResponse { fn default() -> Self { Self { version: 1, surface_id: String::new(), bindings: Vec::new(), actions: Vec::new() } } }

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UiSurfaceNodeLayout {
    pub screen_w: f32,
    pub screen_h: f32,
    pub panel_x: f32,
    pub panel_y: f32,
    pub panel_w: f32,
    pub panel_h: f32,
    pub body_x: f32,
    pub body_y: f32,
    pub body_w: f32,
    pub body_line_pitch: f32,
    pub footer_y: f32,
    pub large_panel: bool,
}

impl UiSurfaceNodeLayout {
    #[inline]
    pub fn hit_body_line_index(self, mouse_pos: Option<(f32, f32)>) -> Option<usize> {
        let (mx, my) = mouse_pos?;
        if mx < self.body_x || mx > self.body_x + self.body_w || my < self.body_y {
            return None;
        }
        if my > self.panel_y + self.panel_h - 56.0 {
            return None;
        }
        let idx = ((my - self.body_y) / self.body_line_pitch).floor() as isize;
        if idx < 0 { None } else { Some(idx as usize) }
    }

    #[inline]
    pub fn hit_item_index_after_header(self, mouse_pos: Option<(f32, f32)>, header_lines: usize, item_count: usize) -> Option<usize> {
        let line = self.hit_body_line_index(mouse_pos)?;
        let item = line.checked_sub(header_lines)?;
        if item < item_count { Some(item) } else { None }
    }
}

#[inline]
pub fn ui_surface_node_layout(
    surface_size_px: [u32; 2],
    style_tags: &[String],
    style: &UiSurfaceStyle,
    body_line_count: usize,
    footer_line_count: usize,
) -> UiSurfaceNodeLayout {
    let w = surface_size_px[0].max(1) as f32;
    let h = surface_size_px[1].max(1) as f32;
    let style = style.clone().normalized();

    let workspace = style_tags.iter().any(|tag| tag == "workspace" || tag == "fullscreen");
    let available_w = (w - style.margin_px[0] * 2.0).max(style.min_size_px[0]);
    let available_h = (h - style.margin_px[1] * 2.0).max(style.min_size_px[1]);
    let line_count = body_line_count.max(1) + footer_line_count + 2;
    let content_h = line_count as f32 * style.row_pitch_px.max(style.font.line_height_px).max(24.0)
        + style.padding_px[1] + style.padding_px[3] + 10.0;
    let panel_w = if workspace {
        available_w
    } else {
        style.max_size_px[0].min(available_w).max(style.min_size_px[0])
    };
    let panel_h = if workspace {
        available_h
    } else {
        style.max_size_px[1].min(available_h).max(style.min_size_px[1]).max(content_h.min(available_h))
    };

    let panel_x = match style.anchor {
        UiSurfaceAnchor::TopRight | UiSurfaceAnchor::BottomRight => (w - panel_w - style.margin_px[0]).max(style.margin_px[0]),
        UiSurfaceAnchor::Center => ((w - panel_w) * 0.5).max(style.margin_px[0]),
        UiSurfaceAnchor::TopLeft | UiSurfaceAnchor::BottomLeft => style.margin_px[0],
    };
    let panel_y = match style.anchor {
        UiSurfaceAnchor::BottomLeft | UiSurfaceAnchor::BottomRight => (h - panel_h - style.margin_px[1]).max(style.margin_px[1]),
        UiSurfaceAnchor::Center => ((h - panel_h) * 0.5).max(style.margin_px[1]),
        UiSurfaceAnchor::TopLeft | UiSurfaceAnchor::TopRight => style.margin_px[1],
    };

    let raw_line_h = if style.font.line_height_px > 0.0 { style.font.line_height_px } else { 24.0 };
    let line_pitch = style.row_pitch_px.max(raw_line_h + 2.0);
    let large_panel = panel_h >= 360.0 || panel_w >= 420.0;
    UiSurfaceNodeLayout {
        screen_w: w,
        screen_h: h,
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        body_x: panel_x + style.padding_px[0],
        body_y: panel_y + style.padding_px[1],
        body_w: (panel_w - style.padding_px[0] - style.padding_px[2]).max(32.0),
        body_line_pitch: line_pitch,
        footer_y: panel_y + panel_h - style.padding_px[3] + 4.0,
        large_panel,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiRuntimeDebugOverlayTelemetry {
    pub version: u32,
    pub surface_id: String,
    pub source: String,
    pub frame_index: u64,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub metrics: BTreeMap<String, serde_json::Value>,
}

impl UiRuntimeDebugOverlayTelemetry {
    #[inline]
    pub fn new(frame_index: u64, text: impl Into<String>) -> Self {
        let text = text.into();
        let lines = text.lines().map(str::to_owned).collect();
        Self {
            version: 1,
            surface_id: UI_SURFACE_RUNTIME_DEBUG_OVERLAY.to_owned(),
            source: "engine.runtime.debug".to_owned(),
            frame_index,
            text,
            lines,
            metrics: BTreeMap::new(),
        }
    }

    #[inline]
    pub fn with_metric(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metrics.insert(key.into(), value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_service_ids_are_engine_gateway_first() {
        assert_eq!(ENGINE_UI_SERVICE_ID, "engine.ui");
        assert_eq!(UI_BACKEND_SERVICE_SPEC.engine_gateway_id, ENGINE_UI_SERVICE_ID);
        assert_eq!(UI_BACKEND_SERVICE_SPEC.provider_service_id, UI_SERVICE_ID);
        assert_eq!(UI_BACKEND_SERVICE_SPEC.backend_capability_id, UI_BACKEND_CAPABILITY_ID);
    }

    #[test]
    fn ui_runtime_contract_contains_json_control_methods() {
        let methods = UI_RUNTIME_CONTRACT_SPEC.required_methods;
        assert!(methods.contains(&UI_SERVICE_METHOD_INFO));
        assert!(methods.contains(&UI_SERVICE_METHOD_INVOKE));
        assert!(methods.contains(&UI_SERVICE_METHOD_SHUTDOWN_V1));
    }

    #[test]
    fn ui_service_methods_include_draw_frame() {
        assert!(ui_service_methods().contains(&UI_SERVICE_METHOD_DRAW_FRAME_V1));
    }

    #[test]
    fn ui_service_methods_include_binary_draw_frame() {
        assert!(ui_service_methods().contains(&UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1));
    }

    #[test]
    fn ui_service_methods_include_neui_runtime_lifecycle() {
        assert!(ui_service_methods().contains(&UI_SERVICE_METHOD_MOUNT_SURFACE_V1));
        assert!(ui_service_methods().contains(&UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1));
        assert!(ui_service_methods().contains(&UI_SERVICE_METHOD_DEBUG_TREE_V1));
        assert!(ui_service_methods().contains(&UI_SERVICE_METHOD_DEBUG_BINDINGS_V1));
    }

    #[test]
    fn state_patch_is_surface_scoped() {
        let patch = UiStatePatch::new(42, UI_SURFACE_ENGINE_LOADING)
            .with_change("loading", "progress", serde_json::json!(0.5));
        assert_eq!(patch.surface_id, UI_SURFACE_ENGINE_LOADING);
        assert_eq!(patch.changes.len(), 1);
    }

    #[test]
    fn telemetry_defaults_to_runtime_debug_surface() {
        let telemetry = UiRuntimeDebugOverlayTelemetry::new(7, "FPS 60");
        assert_eq!(telemetry.surface_id, UI_SURFACE_RUNTIME_DEBUG_OVERLAY);
        assert_eq!(telemetry.lines, vec!["FPS 60".to_owned()]);
    }
}
