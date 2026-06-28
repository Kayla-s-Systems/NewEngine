#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

use newengine_core::EngineResult;
use newengine_ui_api::{
    decode_ui_frame_response_bin, encode_ui_frame_request_bin, UiComponentNode, UiDrawList,
    UiFrameRequest, UiFrameResponse, UiInputCaptureState, UiNodeMessage, UiNodeMessageSeverity,
    UiNodeTone, UiSurfaceAdmissionPolicy, UiSurfaceAnchor, UiSurfaceNode, UiSurfaceStyle,
    ENGINE_UI_SERVICE_ID, UI_COMPONENT_PANEL, UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1,
    UI_SERVICE_METHOD_DRAW_FRAME_V1, UI_SERVICE_METHOD_SURFACE_NODE_V1,
    UI_SURFACE_ENGINE_ERROR_MODAL, UI_THEME_NORTHSTAR_DEFAULT,
};

static UI_ROUTE_MISSING_LOGGED: AtomicBool = AtomicBool::new(false);
static TRY_BINARY_UI_DRAW_FRAME: AtomicBool = AtomicBool::new(true);

#[derive(Clone, Debug)]
pub struct UiFrameOutput {
    pub draw_list: UiDrawList,
    pub input_capture: UiInputCaptureState,
}

fn log_missing_ui_route_once(operation: &str) {
    if UI_ROUTE_MISSING_LOGGED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        newengine_ulog_api::ulog::warn!(
            "ui gateway: engine.ui route missing; operation='{}' skipped. Register/sync an engine.ui provider route with ui.backend capability in pluginsRuntime",
            operation
        );
    }
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
    metrics.insert(
        "safe_mode".to_owned(),
        serde_json::json!("degraded-ui-present"),
    );
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
            newengine_ulog_api::ulog::warn!(
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
        Ok(Some(_)) => newengine_ulog_api::ulog::trace!(
            "ui gateway: surface node published surface='{}' source='{}' visible={}",
            node.surface_id,
            node.source,
            node.visible
        ),
        Ok(None) => log_missing_ui_route_once("publish_surface_node"),
        Err(e) => newengine_ulog_api::ulog::warn!(
            "ui gateway: surface node publish failed surface='{}' err='{}'",
            node.surface_id,
            e
        ),
    }
}

/// Publish a provider-neutral runtime UI node request to the active `engine.ui` provider.
///
/// This is the generative sibling of `.neui` mounting. Runtime systems submit
/// data-only node trees and the provider retains/renders them through the same/// Request a current-frame UI draw list through the canonical `engine.ui` gateway.
///
/// Runtime-host normally prepares provider UI before `engine.step()`, but retained
/// UI nodes may be published during that same step. This helper lets a runtime
/// module publish freshly computed state and immediately request a same-frame draw
/// packet without depending on any concrete UI provider implementation.
#[allow(dead_code)]
pub fn request_draw_list(
    frame_index: u64,
    dt_sec: f32,
    surface_size_px: [u32; 2],
    pixels_per_point: f32,
) -> EngineResult<Option<UiDrawList>> {
    Ok(
        request_frame_output(frame_index, dt_sec, surface_size_px, pixels_per_point)?
            .map(|output| output.draw_list),
    )
}

pub fn request_frame_output(
    frame_index: u64,
    dt_sec: f32,
    surface_size_px: [u32; 2],
    pixels_per_point: f32,
) -> EngineResult<Option<UiFrameOutput>> {
    if !newengine_core::has_engine_gateway_route(ENGINE_UI_SERVICE_ID) {
        log_missing_ui_route_once("request_frame_output");
        return Ok(None);
    }

    let request = UiFrameRequest::new(frame_index, dt_sec, surface_size_px, pixels_per_point);

    if TRY_BINARY_UI_DRAW_FRAME.load(Ordering::Relaxed) {
        match request_frame_output_bin(&request) {
            Ok(output) => return Ok(output),
            Err(err) => {
                if TRY_BINARY_UI_DRAW_FRAME.swap(false, Ordering::Relaxed) {
                    newengine_ulog_api::ulog::warn!(
                        "ui gateway: binary draw-frame path unavailable; falling back to JSON control path err='{}'",
                        err
                    );
                }
            }
        }
    }

    request_frame_output_json(&request)
}

fn request_frame_output_bin(request: &UiFrameRequest) -> Result<Option<UiFrameOutput>, String> {
    let payload = encode_ui_frame_request_bin(request)
        .map_err(|e| format!("encode binary ui frame request failed: {e}"))?;
    let Some(bytes) = newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1,
        &payload,
    )
    .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    let response = decode_ui_frame_response_bin(&bytes)
        .map_err(|e| format!("decode binary ui frame response failed: {e}"))?;
    Ok(response_to_frame_output(response))
}

fn request_frame_output_json(request: &UiFrameRequest) -> EngineResult<Option<UiFrameOutput>> {
    let payload = serde_json::to_vec(request).map_err(|e| {
        newengine_core::EngineError::other(format!("encode ui frame request failed: {e}"))
    })?;

    let Some(bytes) = newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_DRAW_FRAME_V1,
        &payload,
    )
    .map_err(newengine_core::EngineError::other)?
    else {
        return Ok(None);
    };

    let response: UiFrameResponse = serde_json::from_slice(&bytes).map_err(|e| {
        newengine_core::EngineError::other(format!("decode ui frame response failed: {e}"))
    })?;
    Ok(response_to_frame_output(response))
}

fn response_to_frame_output(response: UiFrameResponse) -> Option<UiFrameOutput> {
    // Empty draw-lists are valid clear packets. Returning None would keep the
    // previous retained modal/menu UI alive in render backends that consume UI
    // through RenderCommand::SetUiDrawList.
    Some(UiFrameOutput {
        draw_list: response.draw_list,
        input_capture: response.input_capture,
    })
}
