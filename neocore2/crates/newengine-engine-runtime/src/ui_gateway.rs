#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicBool, Ordering};

use newengine_core::EngineResult;
use newengine_ui_api::{
    decode_ui_frame_response_bin, encode_ui_frame_request_bin, UiDrawList, UiFrameRequest,
    UiFrameResponse, UiPauseMenuState, UiRuntimeDebugOverlayTelemetry, ENGINE_UI_SERVICE_ID,
    UI_SERVICE_METHOD_DEBUG_OVERLAY_TELEMETRY_V1, UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1,
    UI_SERVICE_METHOD_DRAW_FRAME_V1, UI_SERVICE_METHOD_PAUSE_MENU_STATE_V1,
};

static UI_ROUTE_MISSING_LOGGED: AtomicBool = AtomicBool::new(false);
static TRY_BINARY_UI_DRAW_FRAME: AtomicBool = AtomicBool::new(true);

fn log_missing_ui_route_once(operation: &str) {
    if UI_ROUTE_MISSING_LOGGED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        log::warn!(
            "ui gateway: engine.ui route missing; operation='{}' skipped. Build/sync AureliaUI so aurelia_ui_provider-<version>-<profile>.dll exists in NewEngine/neocore2/plugins",
            operation
        );
    }
}

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
        Ok(Some(_)) => log::trace!(
            "ui gateway: telemetry published surface='{}' source='{}'",
            telemetry.surface_id,
            telemetry.source
        ),
        Ok(None) => log_missing_ui_route_once("publish_debug_overlay_telemetry"),
        Err(e) => log::warn!("ui gateway: debug overlay telemetry publish failed: {e}"),
    }
}

pub fn publish_pause_menu_state(state: &UiPauseMenuState) {
    let payload = match serde_json::to_vec(state) {
        Ok(payload) => payload,
        Err(e) => {
            log::warn!("ui gateway: failed to encode pause menu state: {e}");
            return;
        }
    };

    match newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_PAUSE_MENU_STATE_V1,
        &payload,
    ) {
        Ok(Some(_)) => log::trace!(
            "ui gateway: pause menu state published visible={} selected={} items={}",
            state.visible,
            state.selected_index,
            state.items.len()
        ),
        Ok(None) => log_missing_ui_route_once("publish_pause_menu_state"),
        Err(e) => log::warn!("ui gateway: pause menu state publish failed: {e}"),
    }
}

/// Request a current-frame UI draw list through the canonical `engine.ui` gateway.
///
/// Runtime-host normally prepares provider UI before `engine.step()`, but modal UI
/// state such as ESC pause menu and F1 Asset Browser is produced inside the render
/// controller during that same step. This helper lets the render controller publish
/// the freshly computed state and immediately request a same-frame draw packet,
/// without depending on any concrete UI provider implementation.
pub fn request_draw_list(
    frame_index: u64,
    dt_sec: f32,
    surface_size_px: [u32; 2],
    pixels_per_point: f32,
) -> EngineResult<Option<UiDrawList>> {
    if !newengine_core::has_engine_gateway_route(ENGINE_UI_SERVICE_ID) {
        log_missing_ui_route_once("request_draw_list");
        return Ok(None);
    }

    let request = UiFrameRequest::new(frame_index, dt_sec, surface_size_px, pixels_per_point);

    if TRY_BINARY_UI_DRAW_FRAME.load(Ordering::Relaxed) {
        match request_draw_list_bin(&request) {
            Ok(draw_list) => return Ok(draw_list),
            Err(err) => {
                if TRY_BINARY_UI_DRAW_FRAME.swap(false, Ordering::Relaxed) {
                    log::warn!(
                        "ui gateway: binary draw-frame path unavailable; falling back to JSON control path err='{}'",
                        err
                    );
                }
            }
        }
    }

    request_draw_list_json(&request)
}

fn request_draw_list_bin(request: &UiFrameRequest) -> Result<Option<UiDrawList>, String> {
    let payload = encode_ui_frame_request_bin(request)
        .map_err(|e| format!("encode binary ui frame request failed: {e}"))?;
    let Some(bytes) = newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1,
        &payload,
    )
    .map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let response = decode_ui_frame_response_bin(&bytes)
        .map_err(|e| format!("decode binary ui frame response failed: {e}"))?;
    Ok(non_empty_draw_list(response))
}

fn request_draw_list_json(request: &UiFrameRequest) -> EngineResult<Option<UiDrawList>> {
    let payload = serde_json::to_vec(request)
        .map_err(|e| newengine_core::EngineError::other(format!("encode ui frame request failed: {e}")))?;

    let Some(bytes) = newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_DRAW_FRAME_V1,
        &payload,
    )
    .map_err(newengine_core::EngineError::other)? else {
        return Ok(None);
    };

    let response: UiFrameResponse = serde_json::from_slice(&bytes)
        .map_err(|e| newengine_core::EngineError::other(format!("decode ui frame response failed: {e}")))?;
    Ok(non_empty_draw_list(response))
}

fn non_empty_draw_list(response: UiFrameResponse) -> Option<UiDrawList> {
    if ui_draw_list_is_empty(&response.draw_list) {
        None
    } else {
        Some(response.draw_list)
    }
}

fn ui_draw_list_is_empty(draw_list: &UiDrawList) -> bool {
    draw_list.mesh.vertices.is_empty()
        && draw_list.mesh.indices.is_empty()
        && draw_list.mesh.cmds.is_empty()
        && draw_list.texture_delta.set.is_empty()
        && draw_list.texture_delta.patches.is_empty()
        && draw_list.texture_delta.free.is_empty()
}
