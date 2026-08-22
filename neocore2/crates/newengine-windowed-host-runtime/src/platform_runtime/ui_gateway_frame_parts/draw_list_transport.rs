use super::draw_list_diagnostics::log_ui_gateway_frame;
use super::draw_list_loading::{animate_loading_draw_list, ensure_production_loading_images};
use super::draw_list_state::loading_animation_now_ms;
use super::*;

pub(crate) fn request_ui_draw_list(
    frame_index: u64,
    dt_sec: f32,
    surface_size_px: [u32; 2],
    pixels_per_point: f32,
    render_surface_ids: &[String],
    policy: &UiGatewayFramePolicy,
) -> EngineResult<Option<UiDrawList>> {
    let started = Instant::now();
    let now_ms = loading_animation_now_ms();
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
    let Some(bytes) = ui_draw_frame_bin_call()
        .call_optional(&payload)
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    let service_ms = service_started.elapsed().as_secs_f32() * 1000.0;

    let decode_started = Instant::now();
    let mut response = decode_ui_frame_response_bin(&bytes)
        .map_err(|e| format!("decode binary ui frame response failed: {e}"))?;
    let decode_ms = decode_started.elapsed().as_secs_f32() * 1000.0;
    ensure_production_loading_images(request, &mut response.draw_list);
    animate_loading_draw_list(&mut response.draw_list, request.frame_input.now_ms);

    log_ui_gateway_frame(
        "bin",
        request,
        started,
        encode_ms,
        service_ms,
        decode_ms,
        bytes.len(),
        &response,
    );
    Ok(Some(response.draw_list))
}

fn request_ui_draw_list_json(
    request: &UiFrameRequest,
    started: Instant,
) -> EngineResult<Option<UiDrawList>> {
    let payload = serde_json::to_vec(request).map_err(|e| {
        newengine_core::EngineError::other(format!("encode ui frame request failed: {e}"))
    })?;
    let encode_ms = started.elapsed().as_secs_f32() * 1000.0;

    let service_started = Instant::now();
    let Some(bytes) = ui_draw_frame_json_call()
        .call_optional(&payload)
        .map_err(newengine_core::EngineError::other)?
    else {
        return Ok(None);
    };
    let service_ms = service_started.elapsed().as_secs_f32() * 1000.0;

    let decode_started = Instant::now();
    let mut response: UiFrameResponse = serde_json::from_slice(&bytes).map_err(|e| {
        newengine_core::EngineError::other(format!("decode ui frame response failed: {e}"))
    })?;
    let decode_ms = decode_started.elapsed().as_secs_f32() * 1000.0;
    ensure_production_loading_images(request, &mut response.draw_list);
    animate_loading_draw_list(&mut response.draw_list, request.frame_input.now_ms);

    log_ui_gateway_frame(
        "json",
        request,
        started,
        encode_ms,
        service_ms,
        decode_ms,
        bytes.len(),
        &response,
    );
    Ok(Some(response.draw_list))
}
