#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ui_api::{
    UiRuntimeDebugOverlayTelemetry, ENGINE_UI_SERVICE_ID,
    UI_SERVICE_METHOD_DEBUG_OVERLAY_TELEMETRY_V1,
};

/// Publish runtime/debug UI state to the active `engine.ui` provider.
///
/// This is intentionally a gateway call, not a direct dependency on concrete/native
/// UI implementations and not a render-backend debug string. If no UI provider
/// is loaded the call degrades silently; strict profiles can require
/// `engine.ui` through the startup contract.
pub fn publish_debug_overlay_telemetry(telemetry: &UiRuntimeDebugOverlayTelemetry) {
    let payload = match serde_json::to_vec(telemetry) {
        Ok(payload) => payload,
        Err(e) => {
            log::warn!("ui gateway: failed to encode runtime debug overlay telemetry: {e}");
            return;
        }
    };

    match newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_DEBUG_OVERLAY_TELEMETRY_V1,
        &payload,
    ) {
        Ok(Some(_)) | Ok(None) => {}
        Err(e) => log::warn!("ui gateway: debug overlay telemetry publish failed: {e}"),
    }
}
