#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

pub use newengine_ui_draw::{
    reserved, UiDrawCmd, UiDrawList, UiMesh, UiRect, UiTexId, UiTexture, UiTextureDelta,
    UiTexturePatch, UiVertex,
};
use std::collections::BTreeMap;

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
pub const UI_SERVICE_METHOD_DEBUG_OVERLAY_TELEMETRY_V1: &str = "debug_overlay_telemetry_v1";
pub const UI_SERVICE_METHOD_PAUSE_MENU_STATE_V1: &str = "pause_menu_state_v1";
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
pub const UI_SURFACE_ENGINE_PAUSE_MENU: &str = "engine.pause_menu";

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
                "runtime-debug-overlay".to_owned(),
                "pause-menu-modal".to_owned(),
                "pause-menu-feedback-v1".to_owned(),
                "declarative-pause-menu-theme".to_owned(),
                "declarative-menu-document-v1".to_owned(),
                "menu-action-routes-v1".to_owned(),
                "draw-frame-bin-v1".to_owned(),
                "atlas-text-quads".to_owned(),
                "neui-compiled-document-mount-v1".to_owned(),
                "state-patch-bindings-v1".to_owned(),
                "debug-tree-v1".to_owned(),
                "debug-bindings-v1".to_owned(),
            ],
            methods: ui_service_methods().iter().map(|it| (*it).to_owned()).collect(),
            surfaces: vec![
                UI_SURFACE_ENGINE_LOADING.to_owned(),
                UI_SURFACE_ENGINE_ERROR_MODAL.to_owned(),
                UI_SURFACE_RUNTIME_OVERLAY.to_owned(),
                UI_SURFACE_RUNTIME_DEBUG_OVERLAY.to_owned(),
                UI_SURFACE_ENGINE_PAUSE_MENU.to_owned(),
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
    UI_SERVICE_METHOD_DEBUG_OVERLAY_TELEMETRY_V1,
    UI_SERVICE_METHOD_PAUSE_MENU_STATE_V1,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiCompiledDocument {
    pub version: u32,
    pub document_ref: String,
    pub surface_id: String,
    pub root_id: String,
    pub theme_ref: Option<String>,
    pub dependencies: Vec<String>,
    pub binding_plan: UiBindingPlan,
}

impl Default for UiCompiledDocument {
    fn default() -> Self {
        Self { version: 1, document_ref: String::new(), surface_id: String::new(), root_id: String::new(), theme_ref: None, dependencies: Vec::new(), binding_plan: UiBindingPlan::default() }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiPauseMenuItemTone {
    Normal,
    Accent,
    Danger,
    Disabled,
}

impl Default for UiPauseMenuItemTone {
    #[inline]
    fn default() -> Self { Self::Normal }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPauseMenuItem {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub emphasized: bool,
    #[serde(default)]
    pub tone: UiPauseMenuItemTone,
}

impl UiPauseMenuItem {
    #[inline]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value: None,
            detail: None,
            emphasized: false,
            tone: UiPauseMenuItemTone::Normal,
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
    pub fn emphasized(mut self, emphasized: bool) -> Self {
        self.emphasized = emphasized;
        self
    }

    #[inline]
    pub fn with_tone(mut self, tone: UiPauseMenuItemTone) -> Self {
        self.tone = tone;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiPauseMenuMessageSeverity {
    Info,
    Success,
    Warning,
    Danger,
}

impl Default for UiPauseMenuMessageSeverity {
    #[inline]
    fn default() -> Self { Self::Info }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPauseMenuMessage {
    pub title: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub severity: UiPauseMenuMessageSeverity,
    #[serde(default)]
    pub age_sec: f32,
    #[serde(default)]
    pub ttl_sec: f32,
}

impl UiPauseMenuMessage {
    #[inline]
    pub fn new(title: impl Into<String>, detail: impl Into<String>, severity: UiPauseMenuMessageSeverity) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            severity,
            age_sec: 0.0,
            ttl_sec: 2.2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPauseMenuTheme {
    pub id: String,
    pub accent_rgba: [u8; 4],
    pub accent_secondary_rgba: [u8; 4],
    pub panel_rgba: [u8; 4],
    pub panel_hot_rgba: [u8; 4],
    pub text_rgba: [u8; 4],
    pub text_muted_rgba: [u8; 4],
    pub danger_rgba: [u8; 4],
}

impl Default for UiPauseMenuTheme {
    #[inline]
    fn default() -> Self {
        Self {
            id: "newengine.dark.gold-contrast".to_owned(),
            accent_rgba: [255, 203, 76, 255],
            accent_secondary_rgba: [255, 112, 196, 255],
            panel_rgba: [5, 6, 10, 255],
            panel_hot_rgba: [18, 17, 12, 255],
            text_rgba: [246, 250, 255, 255],
            text_muted_rgba: [188, 202, 224, 255],
            danger_rgba: [255, 86, 98, 255],
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UiPauseMenuLayout {
    pub screen_w: f32,
    pub screen_h: f32,
    pub panel_x: f32,
    pub panel_y: f32,
    pub panel_w: f32,
    pub panel_h: f32,
    pub list_x: f32,
    pub list_y: f32,
    pub list_right: f32,
    pub item_h: f32,
    pub item_visual_h: f32,
    pub footer_y: f32,
    pub rail_x: f32,
    pub rail_w: f32,
}

impl UiPauseMenuLayout {
    #[inline]
    pub fn hit_item_index(self, mouse_pos: Option<(f32, f32)>, item_count: usize) -> Option<usize> {
        let (mx, my) = mouse_pos?;
        if mx < self.list_x || mx > self.list_right || my < self.list_y {
            return None;
        }
        let idx = ((my - self.list_y) / self.item_h).floor() as isize;
        if idx < 0 || idx as usize >= item_count { None } else { Some(idx as usize) }
    }
}

#[inline]
pub fn pause_menu_layout(surface_size_px: [u32; 2], animation_alpha: f32, item_count: usize) -> UiPauseMenuLayout {
    let w = surface_size_px[0].max(1) as f32;
    let h = surface_size_px[1].max(1) as f32;
    let a = animation_alpha.clamp(0.0, 1.0);
    let panel_x = 72.0 - (1.0 - a) * 58.0;
    let panel_y = (h * 0.105).max(44.0) + (1.0 - a) * 18.0;
    let panel_w = (w * 0.42).clamp(430.0, 720.0);
    let panel_h = (h * 0.80).clamp(452.0, 836.0);
    let list_x = panel_x + 36.0;
    let list_y = panel_y + 142.0;
    let item_h = 52.0;
    let footer_y = (panel_y + panel_h - 110.0).max(list_y + item_count as f32 * item_h + 16.0);
    let rail_x = panel_x + panel_w + 28.0;
    let rail_w = (w - rail_x - 72.0).clamp(250.0, 520.0);
    UiPauseMenuLayout {
        screen_w: w,
        screen_h: h,
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        list_x,
        list_y,
        list_right: panel_x + panel_w - 36.0,
        item_h,
        item_visual_h: 42.0,
        footer_y,
        rail_x,
        rail_w,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPauseMenuState {
    pub version: u32,
    pub surface_id: String,
    pub visible: bool,
    pub paused: bool,
    pub page: String,
    pub title: String,
    pub subtitle: String,
    #[serde(default)]
    pub items: Vec<UiPauseMenuItem>,
    #[serde(default)]
    pub selected_index: usize,
    #[serde(default)]
    pub hovered_index: Option<usize>,
    #[serde(default)]
    pub footer_lines: Vec<String>,
    #[serde(default)]
    pub animation_alpha: f32,
    #[serde(default)]
    pub backdrop_opacity: f32,
    #[serde(default)]
    pub blur_radius_px: f32,
    #[serde(default)]
    pub theme: UiPauseMenuTheme,
    #[serde(default)]
    pub message: Option<UiPauseMenuMessage>,
}

impl Default for UiPauseMenuState {
    #[inline]
    fn default() -> Self { Self::hidden() }
}

impl UiPauseMenuState {
    #[inline]
    pub fn hidden() -> Self {
        Self {
            version: 1,
            surface_id: UI_SURFACE_ENGINE_PAUSE_MENU.to_owned(),
            visible: false,
            paused: false,
            page: "hidden".to_owned(),
            title: "PAUSE".to_owned(),
            subtitle: String::new(),
            items: Vec::new(),
            selected_index: 0,
            hovered_index: None,
            footer_lines: Vec::new(),
            animation_alpha: 0.0,
            backdrop_opacity: 0.0,
            blur_radius_px: 0.0,
            theme: UiPauseMenuTheme::default(),
            message: None,
        }
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
