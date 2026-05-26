#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicBool, Ordering};

use newengine_core::EngineResult;
use newengine_ui_api::{
    decode_ui_frame_response_bin, encode_ui_frame_request_bin, UiComponentNode, UiDrawList,
    UiFrameRequest, UiFrameResponse, UiRuntimeDebugOverlayTelemetry, UiSurfaceAnchor,
    UiSurfaceNode, UiSurfaceStyle, ENGINE_UI_SERVICE_ID, UI_COMPONENT_PANEL,
    UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1, UI_SERVICE_METHOD_DRAW_FRAME_V1,
    UI_SERVICE_METHOD_SURFACE_NODE_V1, UI_SURFACE_RUNTIME_DEBUG_OVERLAY,
    UI_THEME_NORTHSTAR_DEFAULT,
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
    let node = surface_node_from_debug_telemetry(telemetry);
    publish_surface_node(&node);
}

fn surface_node_from_debug_telemetry(telemetry: &UiRuntimeDebugOverlayTelemetry) -> UiSurfaceNode {
    let mut lines = if telemetry.lines.is_empty() {
        telemetry.text.lines().map(str::to_owned).collect::<Vec<_>>()
    } else {
        telemetry.lines.clone()
    };
    if lines.is_empty() {
        lines.push(format!("frame={} source={}", telemetry.frame_index, telemetry.source));
    }
    UiSurfaceNode {
        version: 1,
        surface_id: if telemetry.surface_id.trim().is_empty() {
            UI_SURFACE_RUNTIME_DEBUG_OVERLAY.to_owned()
        } else {
            telemetry.surface_id.clone()
        },
        source: telemetry.source.clone(),
        visible: true,
        modal: false,
        z_order: 980,
        title: "RUNTIME DEBUG".to_owned(),
        subtitle: telemetry.source.clone(),
        body_lines: lines.clone(),
        footer_lines: Vec::new(),
        style_tags: vec!["retained".to_owned()],
        theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components: lines
            .iter()
            .enumerate()
            .map(|(index, line)| UiComponentNode::text(format!("debug.line.{index}"), line.clone()))
            .collect(),
        message: None,
        style: UiSurfaceStyle {
            anchor: UiSurfaceAnchor::TopLeft,
            min_size_px: [360.0, 180.0],
            max_size_px: [620.0, 520.0],
            margin_px: [12.0, 12.0],
            row_pitch_px: 22.0,
            ..UiSurfaceStyle::default()
        },
        metrics: telemetry.metrics.clone(),
    }
}


/// Publish a retained UI surface/node to the active `engine.ui` provider.
///
/// Runtime code owns only the state packet. The provider owns node retention,
/// layout, visibility and draw-list generation. This is the canonical path for
/// editor/runtime UI surfaces that should not flicker when their data source
/// refreshes slower than the render loop.
pub fn publish_surface_node(node: &UiSurfaceNode) {
    let payload = match serde_json::to_vec(node) {
        Ok(payload) => payload,
        Err(e) => {
            log::warn!(
                "ui gateway: failed to encode surface node surface='{}': {e}",
                node.surface_id
            );
            return;
        }
    };

    match newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_SURFACE_NODE_V1,
        &payload,
    ) {
        Ok(Some(_)) => log::trace!(
            "ui gateway: surface node published surface='{}' source='{}' visible={}",
            node.surface_id,
            node.source,
            node.visible
        ),
        Ok(None) => log_missing_ui_route_once("publish_surface_node"),
        Err(e) => log::warn!(
            "ui gateway: surface node publish failed surface='{}' err='{}'",
            node.surface_id,
            e
        ),
    }
}

/// Request a current-frame UI draw list through the canonical `engine.ui` gateway.
///
/// Runtime-host normally prepares provider UI before `engine.step()`, but modal UI
/// state such as ESC primary UI node and F1 Asset Browser is produced inside the render
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
    // Empty draw-lists are valid clear packets. Returning None would keep the
    // previous retained modal/menu UI alive in render backends that consume UI
    // through RenderCommand::SetUiDrawList.
    Some(response.draw_list)
}
