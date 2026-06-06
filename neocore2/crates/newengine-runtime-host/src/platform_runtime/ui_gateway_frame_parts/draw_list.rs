use super::*;

pub(crate) fn request_ui_draw_list(
    frame_index: u64,
    dt_sec: f32,
    surface_size_px: [u32; 2],
    pixels_per_point: f32,
    render_surface_ids: &[String],
    policy: &UiGatewayFramePolicy,
) -> EngineResult<Option<UiDrawList>> {
    if !newengine_core::has_engine_gateway_route(ENGINE_UI_SERVICE_ID) {
        return Ok(None);
    }

    let started = Instant::now();
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|it| it.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0);
    let render_surface_ids: Vec<String> = render_surface_ids
        .iter()
        .map(|surface_id| surface_id.trim().to_owned())
        .filter(|surface_id| !surface_id.is_empty())
        .collect();
    let diagnostics_flags = vec![
        "frame".to_owned(),
        "caret".to_owned(),
        "font.resolve".to_owned(),
        "dispatch".to_owned(),
    ];
    let request = UiFrameRequest::new(frame_index, dt_sec, surface_size_px, pixels_per_point)
        .with_now_ms(now_ms)
        .with_render_surface_ids(render_surface_ids)
        .with_diagnostics_flags(diagnostics_flags);

    if TRY_BINARY_UI_FRAME.load(Ordering::Relaxed) || policy.binary_frame_required {
        match request_ui_draw_list_bin(&request, started) {
            Ok(Some(draw_list)) => return Ok(Some(draw_list)),
            Ok(None) if policy.binary_frame_required => {
                return Err(newengine_core::EngineError::other(
                    "ui gateway: binary draw-frame path required by engine.ui policy but active provider returned no response",
                ));
            }
            Ok(None) => return Ok(None),
            Err(err) => {
                let fallback_allowed = policy.handle_binary_error(err)?;
                if !fallback_allowed {
                    return Ok(None);
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

    log_ui_gateway_frame("bin", request, started, encode_ms, service_ms, decode_ms, bytes.len(), &response);
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

    log_ui_gateway_frame("json", request, started, encode_ms, service_ms, decode_ms, bytes.len(), &response);
    Ok(Some(response.draw_list))
}

fn log_ui_gateway_frame(
    codec: &'static str,
    request: &UiFrameRequest,
    started: Instant,
    _encode_ms: f32,
    service_ms: f32,
    _decode_ms: f32,
    response_bytes: usize,
    response: &UiFrameResponse,
) {
    let frame_index = request.frame_index;
    let total_ms = started.elapsed().as_secs_f32() * 1000.0;
    let over_budget = total_ms >= 16.6;
    let warn_slow = total_ms >= 33.3;
    let sampled = frame_index % 240 == 1;
    let has_provider_diagnostics = !response.diagnostics.diagnostics.is_empty()
        || !response.diagnostics.font_resolve.is_empty()
        || response.diagnostics.caret_visible.is_some();
    if !warn_slow && !(over_budget && sampled) && !sampled && !has_provider_diagnostics {
        return;
    }

    let live = request.live_input();
    let stats = if newengine_ulog_api::ulog::debug_enabled() {
        format!(" {}", ui_draw_list_stats(&response.draw_list))
    } else {
        String::new()
    };
    let log_line = format!(
        "ui gateway frame: frame={} now_ms={} codec={} total_ms={:.2} service={:.2}ms response_bytes={} provider='{}' caret={:?} font_diags={}{}",
        frame_index,
        live.now_ms,
        codec,
        total_ms,
        service_ms,
        response_bytes,
        response.diagnostics.provider,
        response.diagnostics.caret_visible,
        response.diagnostics.font_resolve.len(),
        stats,
    );
    if warn_slow {
        newengine_ulog_api::ulog::warn!("{}", log_line);
    } else {
        newengine_ulog_api::ulog::debug!("{}", log_line);
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
