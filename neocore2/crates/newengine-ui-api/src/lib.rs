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
pub const UI_SERVICE_METHOD_DRAW_FRAME_V1: &str = "draw_frame_v1";
pub const UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1: &str = "draw_frame_bin_v1";

pub const UI_SURFACE_ENGINE_LOADING: &str = "engine.loading";
pub const UI_SURFACE_ENGINE_ERROR_MODAL: &str = "engine.error_modal";
pub const UI_SURFACE_RUNTIME_OVERLAY: &str = "runtime.overlay";
pub const UI_SURFACE_RUNTIME_DEBUG_OVERLAY: &str = "runtime.debug_overlay";

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
                "draw-frame-bin-v1".to_owned(),
                "atlas-text-quads".to_owned(),
            ],
            methods: ui_service_methods().iter().map(|it| (*it).to_owned()).collect(),
            surfaces: vec![
                UI_SURFACE_ENGINE_LOADING.to_owned(),
                UI_SURFACE_ENGINE_ERROR_MODAL.to_owned(),
                UI_SURFACE_RUNTIME_OVERLAY.to_owned(),
                UI_SURFACE_RUNTIME_DEBUG_OVERLAY.to_owned(),
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
