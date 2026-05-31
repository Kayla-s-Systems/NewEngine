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

