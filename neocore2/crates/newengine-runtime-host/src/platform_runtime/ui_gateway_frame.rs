#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use newengine_core::EngineResult;
use newengine_system_contracts::ScreenOverlayStatus;
use newengine_system_runtime::loading_surface_projection;
use newengine_ui::draw::UiDrawList;
use newengine_ui::UiProviderBinding;
use newengine_ui_api::{
    decode_ui_frame_response_bin, encode_ui_frame_request_bin, UiFrameRequest, UiFrameResponse,
    UiRuntimeDebugOverlayTelemetry, ENGINE_UI_SERVICE_ID, UI_SERVICE_METHOD_DEBUG_OVERLAY_TELEMETRY_V1,
    UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1, UI_SERVICE_METHOD_DRAW_FRAME_V1,
};

static TRY_BINARY_UI_FRAME: AtomicBool = AtomicBool::new(true);

/// Requests a provider-owned UI draw list from the engine-facing UI gateway.
///
/// Runtime-host must not bind to any concrete UI implementation. It submits
/// only the stable UI frame request DTO to `engine.ui`; the host gateway routes
/// it to the active provider declared by descriptor metadata.
pub(crate) fn request_ui_draw_list(
    frame_index: u64,
    dt_sec: f32,
    surface_size_px: [u32; 2],
    pixels_per_point: f32,
) -> EngineResult<Option<UiDrawList>> {
    if !newengine_plugin_host::has_service(ENGINE_UI_SERVICE_ID) {
        return Ok(None);
    }

    let started = Instant::now();
    let request = UiFrameRequest::new(frame_index, dt_sec, surface_size_px, pixels_per_point);

    if TRY_BINARY_UI_FRAME.load(Ordering::Relaxed) {
        match request_ui_draw_list_bin(&request, started) {
            Ok(Some(draw_list)) => return Ok(Some(draw_list)),
            Ok(None) => return Ok(None),
            Err(err) => {
                if TRY_BINARY_UI_FRAME.swap(false, Ordering::Relaxed) {
                    log::debug!(
                        "ui gateway: binary draw-frame path unavailable; falling back to json err='{}'",
                        err
                    );
                }
            }
        }
    }

    request_ui_draw_list_json(&request, started)
}

fn request_ui_draw_list_bin(
    request: &UiFrameRequest,
    started: Instant,
) -> Result<Option<UiDrawList>, String> {
    let payload = encode_ui_frame_request_bin(request)
        .map_err(|e| format!("encode binary ui frame request failed: {e}"))?;
    let encode_ms = started.elapsed().as_secs_f32() * 1000.0;

    let service_started = Instant::now();
    let Some(bytes) = newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1,
        &payload,
    )
    .map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let service_ms = service_started.elapsed().as_secs_f32() * 1000.0;

    let decode_started = Instant::now();
    let response = decode_ui_frame_response_bin(&bytes)
        .map_err(|e| format!("decode binary ui frame response failed: {e}"))?;
    let decode_ms = decode_started.elapsed().as_secs_f32() * 1000.0;

    log_ui_gateway_frame("bin", request.frame_index, started, encode_ms, service_ms, decode_ms, bytes.len(), &response.draw_list);
    Ok(Some(response.draw_list))
}

fn request_ui_draw_list_json(
    request: &UiFrameRequest,
    started: Instant,
) -> EngineResult<Option<UiDrawList>> {
    let payload = serde_json::to_vec(request)
        .map_err(|e| newengine_core::EngineError::other(format!("encode ui frame request failed: {e}")))?;
    let encode_ms = started.elapsed().as_secs_f32() * 1000.0;

    let service_started = Instant::now();
    let Some(bytes) = newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_DRAW_FRAME_V1,
        &payload,
    )
    .map_err(newengine_core::EngineError::other)? else {
        return Ok(None);
    };
    let service_ms = service_started.elapsed().as_secs_f32() * 1000.0;

    let decode_started = Instant::now();
    let response: UiFrameResponse = serde_json::from_slice(&bytes)
        .map_err(|e| newengine_core::EngineError::other(format!("decode ui frame response failed: {e}")))?;
    let decode_ms = decode_started.elapsed().as_secs_f32() * 1000.0;

    log_ui_gateway_frame("json", request.frame_index, started, encode_ms, service_ms, decode_ms, bytes.len(), &response.draw_list);
    Ok(Some(response.draw_list))
}

fn log_ui_gateway_frame(
    codec: &'static str,
    frame_index: u64,
    started: Instant,
    encode_ms: f32,
    service_ms: f32,
    decode_ms: f32,
    response_bytes: usize,
    draw_list: &UiDrawList,
) {
    let total_ms = started.elapsed().as_secs_f32() * 1000.0;
    if total_ms >= 4.0 || frame_index % 120 == 1 {
        let stats = ui_draw_list_stats(draw_list);
        let log_line = format!(
            "ui gateway frame: frame={} codec={} total_ms={:.2} encode={:.2}ms service={:.2}ms decode={:.2}ms response_bytes={} {}",
            frame_index,
            codec,
            total_ms,
            encode_ms,
            service_ms,
            decode_ms,
            response_bytes,
            stats,
        );
        if total_ms >= 16.6 {
            log::warn!("{}", log_line);
        } else {
            log::debug!("{}", log_line);
        }
    }
}

/// Publishes a loading/error overlay projection to the selected `engine.ui` provider.
///
/// The platform shell may still keep a native minimal fallback for pre-render or
/// provider-missing states, but the normal loading surface is now provider-owned.
pub(crate) fn publish_loading_overlay(
    status: &ScreenOverlayStatus,
    provider: UiProviderBinding,
    frame_index: u64,
) {
    if !newengine_plugin_host::has_service(ENGINE_UI_SERVICE_ID) {
        return;
    }

    let projection = loading_surface_projection(status, provider);
    let mut metrics = BTreeMap::new();
    metrics.insert(
        "surface_projection".to_owned(),
        serde_json::to_value(&projection).unwrap_or(serde_json::Value::Null),
    );
    let telemetry = UiRuntimeDebugOverlayTelemetry {
        version: 1,
        surface_id: projection.surface_id().to_owned(),
        source: "engine.loading".to_owned(),
        frame_index,
        text: format!("{}\n{}\n{}", status.title, status.status, status.detail),
        lines: vec![
            status.title.clone(),
            status.status.clone(),
            status.detail.clone(),
            format!("progress={:.0}%", status.progress_01() * 100.0),
        ],
        metrics,
    };
    publish_debug_overlay_telemetry(&telemetry);
}

/// Publishes provider-neutral runtime debug telemetry through `engine.ui`.
///
/// This lives in runtime-host, not render-controller: render produces telemetry
/// resources, while the host owns service routing to UI providers.
pub(crate) fn publish_debug_overlay_telemetry(telemetry: &UiRuntimeDebugOverlayTelemetry) {
    if !newengine_plugin_host::has_service(ENGINE_UI_SERVICE_ID) {
        return;
    }

    let payload = match serde_json::to_vec(telemetry) {
        Ok(payload) => payload,
        Err(e) => {
            log::warn!("ui gateway: failed to encode debug overlay telemetry: {e}");
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

fn ui_draw_list_stats(draw_list: &UiDrawList) -> String {
    let texture_set_bytes: usize = draw_list
        .texture_delta
        .set
        .values()
        .map(|texture| texture.rgba8.len())
        .sum();
    let patch_bytes: usize = draw_list
        .texture_delta
        .patches
        .iter()
        .map(|patch| patch.rgba8.len())
        .sum();
    format!(
        "ui(vertices={} indices={} cmds={} tex_set={} tex_set_bytes={} patches={} patch_bytes={} free={})",
        draw_list.mesh.vertices.len(),
        draw_list.mesh.indices.len(),
        draw_list.mesh.cmds.len(),
        draw_list.texture_delta.set.len(),
        texture_set_bytes,
        draw_list.texture_delta.patches.len(),
        patch_bytes,
        draw_list.texture_delta.free.len(),
    )
}
