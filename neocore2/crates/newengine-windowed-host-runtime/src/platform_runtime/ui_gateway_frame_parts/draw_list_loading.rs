use super::draw_list_animation::loading_spinner_animation_spec;
use super::draw_list_state::begin_loading_texture_frame;
use super::draw_list_texture::push_loading_image;
use super::*;

pub(super) fn ensure_production_loading_images(
    request: &UiFrameRequest,
    draw_list: &mut UiDrawList,
) {
    let loading_surface_requested = request
        .render_surface_ids
        .iter()
        .any(|surface_id| surface_id == UI_SURFACE_ENGINE_LOADING)
        || request
            .frame_input
            .render_surface_ids
            .iter()
            .any(|surface_id| surface_id == UI_SURFACE_ENGINE_LOADING);
    let visuals =
        newengine_core::loading::LoadingVisualRefs::from_last_startup_config_for_phase_or_default(
            newengine_core::loading::LoadingPhase::RuntimeLoading,
        );
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
    begin_loading_texture_frame(request.frame_index);

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

    let logo_refs: Vec<_> = visuals
        .logo_refs()
        .into_iter()
        .filter_map(|texture_ref| valid_loading_texture_ref(Some(texture_ref)))
        .collect();
    let logo_rects = newengine_core::loading::layout_logo_rects(
        newengine_core::loading::BootViewport {
            width: sw,
            height: sh,
            scale: request.pixels_per_point.max(0.01),
        },
        logo_refs.len(),
    );
    for (index, (texture_ref, rect)) in logo_refs.into_iter().zip(logo_rects).enumerate() {
        let node_id = format!("loading.logo.{index}");
        push_loading_image(
            draw_list,
            &node_id,
            "loading-brand-logo",
            texture_ref,
            [rect.x, rect.y, rect.w, rect.h],
            10 + index as i32,
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
            "engine.ui.loading production image fallback emitted={} source='{}' background={} logos=[{}] spinner={}",
            emitted,
            visuals.source,
            visuals.background.as_deref().unwrap_or(""),
            visuals.logo_refs().join(","),
            visuals.spinner.as_deref().unwrap_or(""),
        ));
        if request.frame_index <= 4 || request.frame_index % 120 == 1 {
            newengine_ulog_api::ulog::debug!(
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

pub(crate) fn animate_loading_draw_list(draw_list: &mut UiDrawList, now_ms: u64) {
    let rotation_radians = loading_spinner_animation_spec()
        .runtime(now_ms)
        .rotation_radians;
    apply_loading_spinner_rotation(draw_list, rotation_radians);
}

pub(super) fn apply_loading_spinner_rotation(draw_list: &mut UiDrawList, rotation_radians: f32) {
    if !rotation_radians.is_finite() {
        return;
    }

    for command in &mut draw_list.paint.commands {
        let UiPaintCommand::Image(image) = command else {
            continue;
        };
        if is_loading_spinner_image(image) {
            image.rotation_radians = rotation_radians;
        }
    }
}

fn is_loading_spinner_image(image: &UiImagePaintCommand) -> bool {
    image.node.node_id == "loading.spinner"
        || image.node.component_id.contains("spinner")
        || image.node.role.contains("spinner")
        || image
            .node
            .state_tags
            .iter()
            .any(|tag| tag == "loading-spinner" || tag == "startup-spinner")
}

fn valid_loading_texture_ref(value: Option<&str>) -> Option<&str> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|value| value.to_ascii_lowercase().contains(".ytd@"))
}
