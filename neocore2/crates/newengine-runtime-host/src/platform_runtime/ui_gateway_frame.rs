#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use newengine_core::EngineResult;
use newengine_system_contracts::ScreenOverlayStatus;
use newengine_system_runtime::loading_surface_projection;
use newengine_ui::UiProviderBinding;
use newengine_ui_api::{
    decode_ui_frame_response_bin, encode_ui_frame_request_bin, UiComponentNode, UiDrawList,
    UiFrameRequest, UiFrameResponse, UiRuntimeDebugOverlayTelemetry, UiSurfaceAnchor,
    UiSurfaceNode, UiSurfaceStyle, ENGINE_UI_SERVICE_ID, UI_COMPONENT_PANEL,
    UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1, UI_SERVICE_METHOD_DRAW_FRAME_V1,
    UI_SERVICE_METHOD_SURFACE_NODE_V1, UI_SURFACE_ENGINE_LOADING,
    UI_SURFACE_RUNTIME_DEBUG_OVERLAY, UI_THEME_NORTHSTAR_DEFAULT,
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
    if !newengine_core::has_engine_gateway_route(ENGINE_UI_SERVICE_ID) {
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
    // Empty draw-lists are valid clear packets. Provider absence is represented
    // by Ok(None) before decode; a decoded empty packet means "clear UI".
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
    // Empty draw-lists are valid clear packets. Provider absence is represented
    // by Ok(None) before decode; a decoded empty packet means "clear UI".
    Ok(Some(response.draw_list))
}

fn log_ui_gateway_frame(
    codec: &'static str,
    frame_index: u64,
    started: Instant,
    _encode_ms: f32,
    service_ms: f32,
    _decode_ms: f32,
    response_bytes: usize,
    draw_list: &UiDrawList,
) {
    let total_ms = started.elapsed().as_secs_f32() * 1000.0;
    let slow = total_ms >= 16.6;
    let sampled = frame_index % 240 == 1;
    if !slow && !sampled {
        return;
    }

    let stats = if log::log_enabled!(log::Level::Debug) {
        format!(" {}", ui_draw_list_stats(draw_list))
    } else {
        String::new()
    };
    let log_line = format!(
        "ui gateway frame: frame={} codec={} total_ms={:.2} service={:.2}ms response_bytes={}{}",
        frame_index,
        codec,
        total_ms,
        service_ms,
        response_bytes,
        stats,
    );
    if slow {
        log::warn!("{}", log_line);
    } else {
        log::debug!("{}", log_line);
    }
}

/// Publishes a loading/error overlay projection to the selected `engine.ui` provider.
///
/// No platform/native renderer is allowed here: when `engine.ui` has no route,
/// this function emits no draw packet and the caller logs a warning.
pub(crate) fn publish_loading_overlay(
    status: &ScreenOverlayStatus,
    provider: UiProviderBinding,
    frame_index: u64,
) {
    if !newengine_core::has_engine_gateway_route(ENGINE_UI_SERVICE_ID) {
        log::warn!("ui gateway: engine.ui route unavailable; loading overlay skipped without native/special renderer");
        return;
    }

    let projection = loading_surface_projection(status, provider);
    // Active retained loading surfaces must not render 100% until they are
    // explicitly hidden by `publish_loading_overlay_inactive`. `SceneLaunchStatus`
    // intentionally uses values like 0.995 during handoff, and {:.0} would round
    // that to 100%, which looked like a completed world while render residency
    // was still pending.
    let progress_percent = (status.progress_01() * 100.0).clamp(0.0, 99.0);
    let lines = vec![
        status.title.clone(),
        status.status.clone(),
        status.detail.clone(),
        format!("progress={:.0}%", progress_percent),
    ];
    let mut metrics = BTreeMap::new();
    metrics.insert("surface_projection".to_owned(), serde_json::to_value(&projection).unwrap_or(serde_json::Value::Null));
    metrics.insert("frame_index".to_owned(), serde_json::json!(frame_index));
    let node = UiSurfaceNode {
        version: 1,
        surface_id: if projection.surface_id().trim().is_empty() {
            UI_SURFACE_ENGINE_LOADING.to_owned()
        } else {
            projection.surface_id().to_owned()
        },
        source: "engine.loading".to_owned(),
        visible: true,
        modal: false,
        z_order: 900,
        title: status.title.clone(),
        subtitle: status.status.clone(),
        body_lines: lines.clone(),
        footer_lines: Vec::new(),
        style_tags: vec!["retained".to_owned()],
        theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
        style_ref: None,
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components: lines
            .iter()
            .enumerate()
            .map(|(index, line)| UiComponentNode::text(format!("loading.line.{index}"), line.clone()))
            .collect(),
        message: None,
        style: UiSurfaceStyle {
            anchor: UiSurfaceAnchor::Center,
            min_size_px: [460.0, 220.0],
            max_size_px: [760.0, 360.0],
            row_pitch_px: 24.0,
            ..UiSurfaceStyle::default()
        },
        admission_policy: Default::default(),
        metrics,
    };
    publish_surface_node(&node);
}


/// Clears the retained engine.loading surface in the selected `engine.ui` provider.
///
/// Loading surfaces are retained UI nodes. When the scene launch gate completes,
/// publishing no active overlay is not enough: the provider needs an explicit
/// hidden node so `Loading World 100%` cannot remain over the playable frame.
pub(crate) fn publish_loading_overlay_inactive(frame_index: u64) {
    if !newengine_core::has_engine_gateway_route(ENGINE_UI_SERVICE_ID) {
        log::warn!("ui gateway: engine.ui route unavailable; loading overlay clear skipped without native/special renderer");
        return;
    }

    let mut metrics = BTreeMap::new();
    metrics.insert("frame_index".to_owned(), serde_json::json!(frame_index));
    metrics.insert("reason".to_owned(), serde_json::json!("scene-launch-complete"));
    let node = UiSurfaceNode {
        version: 1,
        surface_id: UI_SURFACE_ENGINE_LOADING.to_owned(),
        source: "engine.loading".to_owned(),
        visible: false,
        modal: false,
        z_order: 900,
        title: String::new(),
        subtitle: String::new(),
        body_lines: Vec::new(),
        footer_lines: Vec::new(),
        style_tags: vec!["retained".to_owned(), "hidden".to_owned()],
        theme_id: UI_THEME_NORTHSTAR_DEFAULT.to_owned(),
        style_ref: None,
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components: Vec::new(),
        message: None,
        style: UiSurfaceStyle::default(),
        admission_policy: Default::default(),
        metrics,
    };
    publish_surface_node(&node);
}

fn publish_surface_node(node: &UiSurfaceNode) {
    let payload = match serde_json::to_vec(node) {
        Ok(payload) => payload,
        Err(e) => {
            log::warn!("ui gateway: failed to encode surface node surface='{}': {e}", node.surface_id);
            return;
        }
    };
    match newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_SURFACE_NODE_V1,
        &payload,
    ) {
        Ok(Some(_)) => {}
        Ok(None) => log::warn!(
            "ui gateway: engine.ui route unavailable; surface='{}' skipped without native/special renderer",
            node.surface_id,
        ),
        Err(e) => log::warn!("ui gateway: surface node publish failed surface='{}' err='{e}'", node.surface_id),
    }
}

/// Publishes provider-neutral runtime debug telemetry through `engine.ui`.
///
/// This lives in runtime-host, not render-controller: render produces telemetry
/// resources, while the host owns service routing to UI providers.
pub(crate) fn publish_debug_overlay_telemetry(telemetry: &UiRuntimeDebugOverlayTelemetry) {
    if !newengine_core::has_engine_gateway_route(ENGINE_UI_SERVICE_ID) {
        return;
    }
    let mut lines = if telemetry.lines.is_empty() {
        telemetry.text.lines().map(str::to_owned).collect::<Vec<_>>()
    } else {
        telemetry.lines.clone()
    };
    if lines.is_empty() {
        lines.push(format!("frame={} source={}", telemetry.frame_index, telemetry.source));
    }
    let node = UiSurfaceNode {
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
    };
    publish_surface_node(&node);
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
