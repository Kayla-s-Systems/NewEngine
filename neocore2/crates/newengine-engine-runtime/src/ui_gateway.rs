#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

use newengine_core::EngineResult;
use newengine_ui_api::{
    decode_ui_frame_response_bin, encode_ui_frame_request_bin, UiComponentNode, UiDrawList,
    UiFrameRequest, UiFrameResponse, UiNodeMessage, UiNodeMessageSeverity, UiNodeTone,
    UiRuntimeDebugOverlayTelemetry, UiSurfaceAdmissionPolicy, UiSurfaceAnchor,
    UiSurfaceNode, UiSurfaceStyle, ENGINE_UI_SERVICE_ID, UI_COMPONENT_PANEL,
    UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1, UI_SERVICE_METHOD_DRAW_FRAME_V1,
    UI_SERVICE_METHOD_SURFACE_NODE_V1, UI_SURFACE_ENGINE_ERROR_MODAL,
    UI_SURFACE_RUNTIME_DEBUG_OVERLAY, UI_THEME_NORTHSTAR_DEFAULT,
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

/// Publish a visible runtime error modal through the active `engine.ui` provider.
///
/// This is used for recoverable-but-fatal backend failures such as
/// `VK_ERROR_DEVICE_LOST`: the process may keep the platform loop alive, but the
/// player/editor must see why world rendering stopped instead of staring at a
/// stale loading screen.
pub(crate) fn publish_render_backend_error_modal(phase: &'static str, error: &str) {
    let mut metrics = BTreeMap::new();
    metrics.insert("phase".to_owned(), serde_json::json!(phase));
    metrics.insert("backend".to_owned(), serde_json::json!("engine.render"));
    metrics.insert("safe_mode".to_owned(), serde_json::json!("degraded-ui-present"));
    metrics.insert("error".to_owned(), serde_json::json!(error));

    let short_error = summarize_backend_error(error);
    let body_lines = vec![
        "The renderer backend was disabled after a fatal GPU/device error.".to_owned(),
        format!("Phase: {phase}"),
        format!("Error: {short_error}"),
        "The engine is still alive in degraded UI/safe-present mode.".to_owned(),
    ];

    let node = UiSurfaceNode {
        version: 1,
        surface_id: UI_SURFACE_ENGINE_ERROR_MODAL.to_owned(),
        source: "engine.ui.render_error".to_owned(),
        visible: true,
        modal: true,
        z_order: 1_500,
        title: "GPU backend stopped".to_owned(),
        subtitle: "Renderer switched to degraded UI/safe-present mode".to_owned(),
        body_lines: body_lines.clone(),
        footer_lines: vec![
            "Close the app, lower the render profile, or restart the renderer when hot-reload support lands.".to_owned(),
        ],
        style_tags: vec![
            "retained".to_owned(),
            "modal".to_owned(),
            "danger".to_owned(),
            "centered-error".to_owned(),
            "gpu-device-lost".to_owned(),
        ],
        theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
        style_ref: None,
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components: vec![
            UiComponentNode::text("render_error.title", "GPU DEVICE LOST")
                .with_tone(UiNodeTone::Danger)
                .tagged("error-title"),
            UiComponentNode::text("render_error.phase", format!("phase: {phase}"))
                .with_tone(UiNodeTone::Accent)
                .tagged("error-phase"),
            UiComponentNode::text("render_error.detail", short_error)
                .with_tone(UiNodeTone::Normal)
                .tagged("error-detail"),
            UiComponentNode::text(
                "render_error.mode",
                "world rendering is disabled; engine.ui remains active for diagnostics",
            )
            .with_tone(UiNodeTone::Disabled)
            .tagged("error-mode"),
        ],
        message: Some(UiNodeMessage::new(
            "Renderer backend stopped",
            "Fatal GPU/device error; world viewport disabled, diagnostics UI remains alive.",
            UiNodeMessageSeverity::Danger,
        )),
        style: UiSurfaceStyle {
            anchor: UiSurfaceAnchor::Center,
            min_size_px: [620.0, 280.0],
            max_size_px: [940.0, 520.0],
            row_pitch_px: 28.0,
            ..UiSurfaceStyle::default()
        },
        admission_policy: UiSurfaceAdmissionPolicy::AcceptAllButThis,
        metrics,
    };
    publish_surface_node(&node);
}

fn summarize_backend_error(error: &str) -> String {
    const MAX: usize = 460;
    let compact = error.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= MAX {
        compact
    } else {
        let mut out = compact.chars().take(MAX).collect::<String>();
        out.push_str("...");
        out
    }
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
        style_ref: None,
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
        admission_policy: Default::default(),
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
/// Runtime-host normally prepares provider UI before `engine.step()`, but retained
/// UI nodes may be published during that same step. This helper lets a runtime
/// module publish freshly computed state and immediately request a same-frame draw
/// packet without depending on any concrete UI provider implementation.
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
