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
    let Some(bytes) = newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_DRAW_FRAME_V1,
        &payload,
    )
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

fn ensure_production_loading_images(request: &UiFrameRequest, draw_list: &mut UiDrawList) {
    let loading_surface_requested = request
        .render_surface_ids
        .iter()
        .any(|surface_id| surface_id == UI_SURFACE_ENGINE_LOADING)
        || request
            .frame_input
            .render_surface_ids
            .iter()
            .any(|surface_id| surface_id == UI_SURFACE_ENGINE_LOADING);
    let visuals = newengine_core::loading::LoadingVisualRefs::from_last_startup_config_or_default();
    let early_loading_bootstrap = request.frame_index <= 4 && visuals.image_layer_count() > 0;
    if !loading_surface_requested && !early_loading_bootstrap {
        return;
    }

    let existing_images = draw_list
        .paint
        .commands
        .iter()
        .filter(|command| matches!(command, UiPaintCommand::Image(_)))
        .count();
    if existing_images > 0 {
        return;
    }

    // This path is the production loading compositor fallback. The provider mesh
    // here is the generic retained-UI chrome that produced the visible accent
    // stripe/title/debug rows on the loading screen, so it must not survive.
    draw_list.mesh.clear();

    let sw = request.surface_size_px[0].max(1) as f32;
    let sh = request.surface_size_px[1].max(1) as f32;
    let clip = Some([0.0, 0.0, sw, sh]);
    let mut emitted = 0usize;

    if let Some(texture_ref) = valid_loading_texture_ref(visuals.background.as_deref()) {
        push_loading_image(
            draw_list,
            "loading.background",
            "loading-background",
            texture_ref,
            [0.0, 0.0, sw, sh],
            0,
            clip,
            0.0,
            None,
        );
        emitted += 1;
    }

    if let Some(texture_ref) = valid_loading_texture_ref(visuals.logo.as_deref()) {
        let logo = 360.0_f32.min(sw * 0.42).min(sh * 0.55).max(96.0);
        push_loading_image(
            draw_list,
            "loading.logo",
            "loading-brand-logo",
            texture_ref,
            [(sw - logo) * 0.5, (sh - logo) * 0.5, logo, logo],
            10,
            clip,
            0.0,
            None,
        );
        emitted += 1;
    }

    if let Some(texture_ref) = valid_loading_texture_ref(visuals.spinner.as_deref()) {
        let size = 64.0_f32.min(sw * 0.10).min(sh * 0.12).max(24.0);
        // Use wall-clock time, not frame_index. The launch gate can produce uneven
        // frames while assets upload, so frame-index animation looks slow/stuttery.
        let spinner_animation =
            loading_spinner_animation_spec().runtime(request.frame_input.now_ms);
        let spinner_rotation = spinner_animation.rotation_radians;
        push_loading_image(
            draw_list,
            "loading.spinner",
            "loading-spinner",
            texture_ref,
            [(sw - size) * 0.5, sh - size - 96.0, size, size],
            20,
            clip,
            spinner_rotation,
            Some(spinner_animation),
        );
        emitted += 1;
    }

    if emitted > 0 {
        draw_list.paint.diagnostics.push(format!(
            "engine.ui.loading production image fallback emitted={} source='{}' background={} logo={} spinner={}",
            emitted,
            visuals.source,
            visuals.background.as_deref().unwrap_or(""),
            visuals.logo.as_deref().unwrap_or(""),
            visuals.spinner.as_deref().unwrap_or(""),
        ));
        if request.frame_index <= 4 || request.frame_index % 120 == 1 {
            newengine_ulog_api::ulog::warn!(
                "ui gateway production loading image fallback: frame={} emitted={} paint_images={} source='{}' surface={}x{}",
                request.frame_index,
                emitted,
                draw_list
                    .paint
                    .commands
                    .iter()
                    .filter(|command| matches!(command, UiPaintCommand::Image(_)))
                    .count(),
                visuals.source,
                sw,
                sh
            );
        }
    }
}

fn valid_loading_texture_ref(value: Option<&str>) -> Option<&str> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|value| value.to_ascii_lowercase().contains(".ytd@"))
}

#[derive(Debug, Clone)]
struct LoadingSpinnerAnimationSpec {
    rotation_rps: f32,
    sprite_fps: f32,
    sprite_frames: Option<usize>,
    sprite_columns: Option<usize>,
    sprite_rows: Option<usize>,
    frame_width: Option<usize>,
    frame_height: Option<usize>,
    source: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct LoadingSpinnerRuntimeAnimation {
    rotation_radians: f32,
    sprite_frame_index: Option<usize>,
    sprite_frames: Option<usize>,
    sprite_columns: Option<usize>,
    sprite_rows: Option<usize>,
    frame_width: Option<usize>,
    frame_height: Option<usize>,
}

impl LoadingSpinnerAnimationSpec {
    fn fallback() -> Self {
        Self {
            rotation_rps: 2.8,
            sprite_fps: 24.0,
            sprite_frames: Some(1),
            sprite_columns: Some(1),
            sprite_rows: Some(1),
            frame_width: Some(64),
            frame_height: Some(64),
            source: "engine-default",
        }
    }

    fn runtime(&self, now_ms: u64) -> LoadingSpinnerRuntimeAnimation {
        let t_sec = (now_ms % 10_000) as f32 * 0.001;
        let rotation_radians = (t_sec * self.rotation_rps.max(0.0) * std::f32::consts::TAU)
            .rem_euclid(std::f32::consts::TAU);
        let sprite_frame_index = if self.sprite_fps > 0.0 {
            Some(
                ((t_sec * self.sprite_fps).floor() as usize)
                    % self.sprite_frames.unwrap_or(usize::MAX).max(1),
            )
        } else {
            None
        };
        LoadingSpinnerRuntimeAnimation {
            rotation_radians,
            sprite_frame_index,
            sprite_frames: self.sprite_frames,
            sprite_columns: self.sprite_columns,
            sprite_rows: self.sprite_rows,
            frame_width: self.frame_width,
            frame_height: self.frame_height,
        }
    }
}

static LOADING_SPINNER_ANIMATION_SPEC: std::sync::OnceLock<LoadingSpinnerAnimationSpec> =
    std::sync::OnceLock::new();

fn loading_spinner_animation_spec() -> &'static LoadingSpinnerAnimationSpec {
    LOADING_SPINNER_ANIMATION_SPEC.get_or_init(load_spinner_animation_spec_from_neui)
}

fn load_spinner_animation_spec_from_neui() -> LoadingSpinnerAnimationSpec {
    let Some(source) = read_loading_neui_source() else {
        return LoadingSpinnerAnimationSpec::fallback();
    };
    let Some(tag) = extract_image_tag_by_id(&source, "loading.spinner") else {
        return LoadingSpinnerAnimationSpec::fallback();
    };

    let mut spec = LoadingSpinnerAnimationSpec::fallback();
    spec.source = "loading.neui";
    if let Some(value) = neui_param_f32(&tag, "rotation_rps") {
        spec.rotation_rps = value.clamp(0.0, 20.0);
    } else if let Some(value) = neui_param_f32(&tag, "rotation_rad_per_sec")
        .or_else(|| neui_param_f32(&tag, "rotation_rad_s"))
    {
        spec.rotation_rps = (value / std::f32::consts::TAU).clamp(0.0, 20.0);
    }
    if let Some(value) = neui_param_f32(&tag, "sprite_fps") {
        spec.sprite_fps = value.clamp(0.0, 240.0);
    } else if let Some(ms) = neui_param_usize(&tag, "sprite_frame_ms").filter(|ms| *ms > 0) {
        spec.sprite_fps = (1000.0 / ms as f32).clamp(0.0, 240.0);
    }
    spec.sprite_frames = neui_param_usize(&tag, "sprite_frames")
        .or_else(|| neui_param_usize(&tag, "frame_count"))
        .or(spec.sprite_frames);
    spec.sprite_columns = neui_param_usize(&tag, "sprite_columns")
        .or_else(|| neui_param_usize(&tag, "columns"))
        .or(spec.sprite_columns);
    spec.sprite_rows = neui_param_usize(&tag, "sprite_rows")
        .or_else(|| neui_param_usize(&tag, "rows"))
        .or(spec.sprite_rows);
    spec.frame_width = neui_param_usize(&tag, "frame_width")
        .or_else(|| neui_param_usize(&tag, "sprite_frame_width"))
        .or(spec.frame_width);
    spec.frame_height = neui_param_usize(&tag, "frame_height")
        .or_else(|| neui_param_usize(&tag, "sprite_frame_height"))
        .or(spec.frame_height);
    spec
}

fn read_loading_neui_source() -> Option<String> {
    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        for rel in [
            "gameAssets/ui/src/engine/loading.neui.xml",
            "../gameAssets/ui/src/engine/loading.neui.xml",
            "../../gameAssets/ui/src/engine/loading.neui.xml",
            "../../../gameAssets/ui/src/engine/loading.neui.xml",
            "NorthStar/gameAssets/ui/src/engine/loading.neui.xml",
        ] {
            candidates.push(cwd.join(rel));
        }
    }
    candidates.push(std::path::PathBuf::from(
        "NorthStar/gameAssets/ui/src/engine/loading.neui.xml",
    ));

    for path in candidates {
        if let Ok(source) = std::fs::read_to_string(&path) {
            return Some(source);
        }
    }
    None
}

fn extract_image_tag_by_id(source: &str, id: &str) -> Option<String> {
    let mut search_from = 0usize;
    while let Some(offset) = source[search_from..].find("<Image") {
        let start = search_from + offset;
        let end = source[start..].find('>').map(|offset| start + offset + 1)?;
        let tag = &source[start..end];
        if extract_xml_attr(tag, "id").as_deref() == Some(id) {
            return Some(tag.to_owned());
        }
        search_from = end;
    }
    None
}

fn neui_param_f32(tag: &str, key: &str) -> Option<f32> {
    neui_param_raw(tag, key).and_then(|value| value.parse::<f32>().ok())
}

fn neui_param_usize(tag: &str, key: &str) -> Option<usize> {
    neui_param_raw(tag, key).and_then(|value| value.parse::<usize>().ok())
}

fn neui_param_raw(tag: &str, key: &str) -> Option<String> {
    extract_xml_attr(tag, key).or_else(|| {
        for attr in ["tags", "args", "class"] {
            if let Some(value) = extract_xml_attr(tag, attr) {
                if let Some(found) = token_param(&value, key) {
                    return Some(found);
                }
            }
        }
        None
    })
}

fn token_param(value: &str, key: &str) -> Option<String> {
    for token in value.split(|ch: char| ch.is_whitespace() || ch == ';' || ch == ',') {
        if let Some(rest) = token.strip_prefix(key) {
            let rest = rest.strip_prefix('=').or_else(|| rest.strip_prefix(':'))?;
            if !rest.trim().is_empty() {
                return Some(rest.trim().to_owned());
            }
        }
    }
    None
}

fn extract_xml_attr(tag: &str, key: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let key_bytes = key.as_bytes();
    let mut i = 0usize;
    while i + key_bytes.len() < bytes.len() {
        if &bytes[i..i + key_bytes.len()] == key_bytes {
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
            let mut j = i + key_bytes.len();
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if before_ok && j < bytes.len() && bytes[j] == b'=' {
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
                    let quote = bytes[j];
                    j += 1;
                    let start = j;
                    while j < bytes.len() && bytes[j] != quote {
                        j += 1;
                    }
                    if j <= bytes.len() {
                        return Some(String::from_utf8_lossy(&bytes[start..j]).trim().to_owned());
                    }
                }
            }
        }
        i += 1;
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn push_loading_image(
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
        rotation_radians,
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
    rotation_radians: f32,
    spinner_animation: Option<LoadingSpinnerRuntimeAnimation>,
) {
    if draw_list.texture_delta.set.contains_key(&texture_id) {
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
                if rotation_radians.is_finite() && rotation_radians.abs() > 0.000_1 {
                    rotate_ui_texture_rgba8_in_place(&mut texture, rotation_radians);
                }
            }
            let size = texture.size;
            let bytes = texture.rgba8.len();
            draw_list.texture_delta.set.insert(texture_id, texture);
            if draw_list.texture_delta.set.len() <= 4 {
                newengine_ulog_api::ulog::warn!(
                    "engine.ui.loading texture payload bound node_id={} ref='{}' tex_id={} size={}x{} bytes={} rotation_rad={:.3}",
                    node_id,
                    texture_ref,
                    texture_id.0,
                    size[0],
                    size[1],
                    bytes,
                    rotation_radians
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

fn rotate_ui_texture_rgba8_in_place(texture: &mut newengine_ui_api::UiTexture, angle: f32) {
    let width = texture.size[0] as usize;
    let height = texture.size[1] as usize;
    if width == 0
        || height == 0
        || texture.rgba8.len() != width.saturating_mul(height).saturating_mul(4)
    {
        return;
    }

    let src = texture.rgba8.clone();
    let mut dst = vec![0u8; src.len()];
    let cx = (width as f32 - 1.0) * 0.5;
    let cy = (height as f32 - 1.0) * 0.5;
    let s = angle.sin();
    let c = angle.cos();

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            // Inverse rotation: sample source that maps into this destination pixel.
            let sx = cx + dx * c + dy * s;
            let sy = cy - dx * s + dy * c;
            let sx_i = sx.round() as isize;
            let sy_i = sy.round() as isize;
            let dst_i = (y * width + x) * 4;
            if sx_i >= 0 && sy_i >= 0 && (sx_i as usize) < width && (sy_i as usize) < height {
                let src_i = ((sy_i as usize) * width + sx_i as usize) * 4;
                dst[dst_i..dst_i + 4].copy_from_slice(&src[src_i..src_i + 4]);
            }
        }
    }
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

#[allow(clippy::too_many_arguments)]
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
    let first_font_ref = draw_list
        .paint
        .commands
        .iter()
        .find_map(|command| match command {
            UiPaintCommand::Text(text) if !text.font_ref.trim().is_empty() => {
                Some(text.font_ref.as_str())
            }
            _ => None,
        });
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
        "ui(mesh_vertices={} mesh_indices={} mesh_cmds={} paint_cmds={} paint_text={} paint_vector={} paint_images={} paint_diags={} first_font_ref={:?} first_vector_ref={:?} tex_set={} tex_set_bytes={} patches={} patch_bytes={} free={})",
        draw_list.mesh.vertices.len(),
        draw_list.mesh.indices.len(),
        draw_list.mesh.cmds.len(),
        draw_list.paint.commands.len(),
        paint_text,
        paint_vector,
        paint_images,
        draw_list.paint.diagnostics.len(),
        first_font_ref,
        first_vector_ref,
        draw_list.texture_delta.set.len(),
        texture_set_bytes,
        draw_list.texture_delta.patches.len(),
        patch_bytes,
        draw_list.texture_delta.free.len(),
    )
}
