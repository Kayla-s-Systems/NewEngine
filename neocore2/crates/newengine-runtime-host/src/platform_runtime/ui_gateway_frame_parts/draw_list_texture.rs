use super::draw_list_animation::LoadingSpinnerRuntimeAnimation;
use super::draw_list_state::{loading_texture_is_resident, mark_loading_texture_resident};
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn push_loading_image(
    draw_list: &mut UiDrawList,
    node_id: &str,
    component_id: &str,
    texture_ref: &str,
    rect: [f32; 4],
    z_index: i32,
    clip_rect: Option<[f32; 4]>,
    rotation_radians: f32,
    spinner_animation: Option<LoadingSpinnerRuntimeAnimation>,
) {
    let texture_id = texture_id_for_loading_ref(texture_ref);
    ensure_loading_texture_payload(
        draw_list,
        texture_id,
        texture_ref,
        node_id,
        spinner_animation,
    );
    draw_list
        .paint
        .push(UiPaintCommand::Image(UiImagePaintCommand {
            node: UiPaintNodeRef {
                surface_id: UI_SURFACE_ENGINE_LOADING.to_owned(),
                node_id: node_id.to_owned(),
                component_id: component_id.to_owned(),
                role: component_id.to_owned(),
                state: "normal".to_owned(),
                state_tags: vec!["production-loading".to_owned(), "ytd-texture".to_owned()],
                z_index,
            },
            rect,
            texture_id: Some(texture_id),
            texture_ref: Some(texture_ref.to_owned()),
            tint_rgba: 0xffff_ffff,
            rotation_radians,
            clip_rect,
            ..Default::default()
        }));
}

fn ensure_loading_texture_payload(
    draw_list: &mut UiDrawList,
    texture_id: UiTexId,
    texture_ref: &str,
    node_id: &str,
    spinner_animation: Option<LoadingSpinnerRuntimeAnimation>,
) {
    if draw_list.texture_delta.set.contains_key(&texture_id) {
        return;
    }

    let cacheable = spinner_animation
        .map(|animation| animation.sprite_frames.unwrap_or(1) <= 1)
        .unwrap_or(true);
    if cacheable && loading_texture_is_resident(texture_id, texture_ref) {
        return;
    }

    let payload = match serde_json::to_vec(&serde_json::json!({ "texture_ref": texture_ref })) {
        Ok(payload) => payload,
        Err(err) => {
            draw_list.paint.diagnostics.push(format!(
                "engine.ui.loading texture payload encode failed node_id={} ref='{}' err={}",
                node_id, texture_ref, err
            ));
            return;
        }
    };

    let bytes = match newengine_core::call_service_v1(
        newengine_assets_api::ENGINE_ASSETS_TEXTURES_SERVICE_ID,
        newengine_assets_api::textures_method::ENTRY_RGBA8_V1,
        &payload,
    ) {
        Ok(bytes) => bytes,
        Err(err) => {
            newengine_ulog_api::ulog::warn!(
                "engine.ui.loading texture payload resolve failed node_id={} ref='{}' err={}",
                node_id,
                texture_ref,
                err
            );
            draw_list.paint.diagnostics.push(format!(
                "engine.ui.loading texture payload resolve failed node_id={} ref='{}' err={}",
                node_id, texture_ref, err
            ));
            return;
        }
    };

    match decode_loading_rgba8_texture(&bytes) {
        Ok(mut texture) => {
            if node_id.contains("spinner") {
                apply_spinner_sprite_frame_if_sheet(&mut texture, spinner_animation);
            }
            let size = texture.size;
            let bytes = texture.rgba8.len();
            draw_list.texture_delta.set.insert(texture_id, texture);
            if cacheable {
                mark_loading_texture_resident(texture_id, texture_ref);
            }
            if draw_list.texture_delta.set.len() <= 4 {
                newengine_ulog_api::ulog::debug!(
                    "engine.ui.loading texture payload bound node_id={} ref='{}' tex_id={} size={}x{} bytes={} cacheable={}",
                    node_id,
                    texture_ref,
                    texture_id.0,
                    size[0],
                    size[1],
                    bytes,
                    cacheable
                );
            }
        }
        Err(err) => {
            draw_list.paint.diagnostics.push(format!(
                "engine.ui.loading texture payload decode failed node_id={} ref='{}' err={}",
                node_id, texture_ref, err
            ));
        }
    }
}

fn decode_loading_rgba8_texture(bytes: &[u8]) -> Result<newengine_ui_api::UiTexture, String> {
    let min_len = newengine_assets_api::texture_wire::HEADER_LEN;
    if bytes.len() < min_len {
        return Err(format!(
            "short rgba8 frame bytes={} expected_at_least={}",
            bytes.len(),
            min_len
        ));
    }
    if bytes[0..4] != newengine_assets_api::texture_wire::MAGIC[..] {
        return Err("bad rgba8 frame magic".to_owned());
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != newengine_assets_api::texture_wire::VERSION_RGBA8_V1 {
        return Err(format!("unsupported rgba8 frame version={}", version));
    }
    let width = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let height = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    let payload_len = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
    let expected_frame_len = min_len.saturating_add(payload_len);
    if bytes.len() != expected_frame_len {
        return Err(format!(
            "rgba8 frame size mismatch bytes={} expected={}",
            bytes.len(),
            expected_frame_len
        ));
    }
    let rgba8 = bytes[min_len..].to_vec();
    let expected_rgba = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if rgba8.len() != expected_rgba {
        return Err(format!(
            "rgba8 payload size mismatch bytes={} expected={} extent={}x{}",
            rgba8.len(),
            expected_rgba,
            width,
            height
        ));
    }
    Ok(newengine_ui_api::UiTexture {
        size: [width, height],
        rgba8,
    })
}

fn apply_spinner_sprite_frame_if_sheet(
    texture: &mut newengine_ui_api::UiTexture,
    animation: Option<LoadingSpinnerRuntimeAnimation>,
) {
    let Some(animation) = animation else {
        return;
    };
    let Some(frame_index) = animation.sprite_frame_index else {
        return;
    };

    let width = texture.size[0] as usize;
    let height = texture.size[1] as usize;
    if width == 0
        || height == 0
        || texture.rgba8.len() != width.saturating_mul(height).saturating_mul(4)
    {
        return;
    }

    let explicit_frame = match (animation.frame_width, animation.frame_height) {
        (Some(frame_w), Some(frame_h)) if frame_w > 0 && frame_h > 0 => {
            let columns = animation
                .sprite_columns
                .unwrap_or_else(|| (width / frame_w).max(1))
                .max(1);
            let rows = animation
                .sprite_rows
                .unwrap_or_else(|| (height / frame_h).max(1))
                .max(1);
            let frame_count = animation
                .sprite_frames
                .unwrap_or(columns.saturating_mul(rows))
                .max(1);
            if frame_w.saturating_mul(columns) <= width && frame_h.saturating_mul(rows) <= height {
                Some((frame_w, frame_h, columns, rows, frame_count))
            } else {
                None
            }
        }
        _ => None,
    };

    let (frame_w, frame_h, columns, rows, frame_count) = if let Some(explicit) = explicit_frame {
        explicit
    } else if let (Some(columns), Some(rows)) = (animation.sprite_columns, animation.sprite_rows) {
        let columns = columns.max(1);
        let rows = rows.max(1);
        let frame_w = width / columns;
        let frame_h = height / rows;
        let frame_count = animation
            .sprite_frames
            .unwrap_or(columns.saturating_mul(rows))
            .max(1);
        if frame_w == 0 || frame_h == 0 {
            return;
        }
        (frame_w, frame_h, columns, rows, frame_count)
    } else if width > height && width.is_multiple_of(height) {
        let columns = width / height;
        let frame_count = animation.sprite_frames.unwrap_or(columns).max(1);
        (height, height, columns, 1usize, frame_count)
    } else if height > width && height.is_multiple_of(width) {
        let rows = height / width;
        let frame_count = animation.sprite_frames.unwrap_or(rows).max(1);
        (width, width, 1usize, rows, frame_count)
    } else {
        return;
    };

    if !(2..=256).contains(&frame_count) || columns == 0 || rows == 0 {
        return;
    }

    let frame = frame_index % frame_count;
    let col = frame % columns;
    let row = (frame / columns).min(rows.saturating_sub(1));
    let frame_x = col.saturating_mul(frame_w);
    let frame_y = row.saturating_mul(frame_h);
    if frame_x + frame_w > width || frame_y + frame_h > height {
        return;
    }

    let src = texture.rgba8.clone();
    let mut dst = vec![0u8; frame_w.saturating_mul(frame_h).saturating_mul(4)];
    for y in 0..frame_h {
        let src_start = ((frame_y + y) * width + frame_x) * 4;
        let src_end = src_start + frame_w * 4;
        let dst_start = y * frame_w * 4;
        dst[dst_start..dst_start + frame_w * 4].copy_from_slice(&src[src_start..src_end]);
    }
    texture.size = [frame_w as u32, frame_h as u32];
    texture.rgba8 = dst;
}

fn texture_id_for_loading_ref(texture_ref: &str) -> UiTexId {
    let mut hash = 0x811c_9dc5u32;
    for byte in texture_ref.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    reserved::external_from_u32(hash)
}
