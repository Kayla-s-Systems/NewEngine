use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn log_ui_gateway_frame(
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
    let warn_slow = total_ms >= 33.3;
    let sampled = frame_index % 240 == 1;
    let has_provider_diagnostics = !response.diagnostics.diagnostics.is_empty()
        || !response.diagnostics.font_resolve.is_empty()
        || response.diagnostics.caret_visible.is_some();
    if !warn_slow && !sampled && !has_provider_diagnostics {
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
    let paint_text = draw_list
        .paint
        .commands
        .iter()
        .filter(|command| matches!(command, UiPaintCommand::Text(_)))
        .count();
    let paint_vector = draw_list
        .paint
        .commands
        .iter()
        .filter(|command| matches!(command, UiPaintCommand::Vector(_)))
        .count();
    let paint_images = draw_list
        .paint
        .commands
        .iter()
        .filter(|command| matches!(command, UiPaintCommand::Image(_)))
        .count();
    let first_text = draw_list
        .paint
        .commands
        .iter()
        .find_map(|command| match command {
            UiPaintCommand::Text(text) => Some(text),
            _ => None,
        });
    let first_font_ref = first_text
        .filter(|text| !text.font_ref.trim().is_empty())
        .map(|text| text.font_ref.as_str());
    let first_vector_ref = draw_list
        .paint
        .commands
        .iter()
        .find_map(|command| match command {
            UiPaintCommand::Vector(vector) if !vector.vector.uri.trim().is_empty() => {
                Some(vector.vector.uri.as_str())
            }
            _ => None,
        });
    format!(
        "ui(mesh_vertices={} mesh_indices={} mesh_cmds={} paint_cmds={} paint_text={} paint_vector={} paint_images={} paint_diags={} first_font_ref={:?} first_text={:?} first_text_rect={:?} first_text_clip={:?} first_text_color={:?} first_text_px={:?} first_vector_ref={:?} tex_set={} tex_set_bytes={} patches={} patch_bytes={} free={})",
        draw_list.mesh.vertices.len(),
        draw_list.mesh.indices.len(),
        draw_list.mesh.cmds.len(),
        draw_list.paint.commands.len(),
        paint_text,
        paint_vector,
        paint_images,
        draw_list.paint.diagnostics.len(),
        first_font_ref,
        first_text.map(|text| text.text.as_str()),
        first_text.map(|text| text.rect),
        first_text.and_then(|text| text.clip_rect),
        first_text.map(|text| format!("0x{:08x}", text.color)),
        first_text.map(|text| text.font_px),
        first_vector_ref,
        draw_list.texture_delta.set.len(),
        texture_set_bytes,
        draw_list.texture_delta.patches.len(),
        patch_bytes,
        draw_list.texture_delta.free.len(),
    )
}
