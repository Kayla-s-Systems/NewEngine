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
                "draw-frame-bin-v1".to_owned(),
                "atlas-text-quads".to_owned(),
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
    fn telemetry_defaults_to_runtime_debug_surface() {
        let telemetry = UiRuntimeDebugOverlayTelemetry::new(7, "FPS 60");
        assert_eq!(telemetry.surface_id, UI_SURFACE_RUNTIME_DEBUG_OVERLAY);
        assert_eq!(telemetry.lines, vec!["FPS 60".to_owned()]);
    }
}
