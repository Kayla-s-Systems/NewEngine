use super::*;

pub(crate) fn dispatch_input_frame(
    frame_index: u64,
    input: &UiInputFrame,
    surface_size_px: [u32; 2],
    pixels_per_point: f32,
) -> EngineResult<Option<UiEventDispatchFrame>> {
    let request = UiDispatchInputRequest {
        frame_index,
        input: input.clone(),
        surface_size_px,
        pixels_per_point,
        ..UiDispatchInputRequest::default()
    };
    let payload = serde_json::to_vec(&request).map_err(|e| {
        newengine_core::EngineError::other(format!("encode ui dispatch input request failed: {e}"))
    })?;
    let Some(bytes) = ui_dispatch_input_call()
        .call_optional(&payload)
        .map_err(newengine_core::EngineError::other)?
    else {
        return Ok(None);
    };

    let frame: UiEventDispatchFrame = serde_json::from_slice(&bytes).map_err(|e| {
        newengine_core::EngineError::other(format!("decode ui dispatch input response failed: {e}"))
    })?;

    apply_ui_state_patches(&frame.state_patches);
    dispatch_ui_actions(&frame.actions);
    log_ui_dispatch_frame(&frame);
    Ok(Some(frame))
}

fn apply_ui_state_patches(patches: &[UiStatePatch]) {
    if patches.is_empty() {
        return;
    }
    for patch in patches {
        if is_runtime_overlay_surface(patch.surface_id.as_str()) {
            continue;
        }
        let payload = match serde_json::to_vec(patch) {
            Ok(payload) => payload,
            Err(e) => {
                newengine_ulog_api::ulog::warn!(
                    "ui gateway: failed to encode state patch surface='{}': {e}",
                    patch.surface_id
                );
                continue;
            }
        };
        if let Err(e) = ui_apply_state_patch_call().call_optional(&payload) {
            newengine_ulog_api::ulog::warn!(
                "ui gateway: state patch apply failed surface='{}' err='{e}'",
                patch.surface_id
            );
        }
    }
}

fn is_runtime_overlay_surface(surface_id: &str) -> bool {
    matches!(
        surface_id,
        UI_SURFACE_ENGINE_ERROR_MODAL | UI_SURFACE_RUNTIME_DEBUG_OVERLAY
    )
}

fn dispatch_ui_actions(actions: &[UiActionDispatch]) {
    for action in actions {
        if action.target_gateway.trim().is_empty() || action.method.trim().is_empty() {
            newengine_ulog_api::ulog::warn!(
                "ui gateway: action dispatch skipped action='{}' target='{}' method='{}'",
                action.action_id,
                action.target_gateway,
                action.method
            );
            continue;
        }

        let payload = match action_dispatch_payload(action) {
            Ok(payload) => payload,
            Err(e) => {
                newengine_ulog_api::ulog::warn!(
                    "ui gateway: failed to encode action dispatch action='{}': {e}",
                    action.action_id
                );
                continue;
            }
        };

        match newengine_core::call_service_v1_optional(
            &action.target_gateway,
            &action.method,
            &payload,
        ) {
            Ok(Some(_)) => {}
            Ok(None) => newengine_ulog_api::ulog::warn!(
                "ui gateway: action target unavailable action='{}' target='{}' method='{}'",
                action.action_id,
                action.target_gateway,
                action.method
            ),
            Err(e) => newengine_ulog_api::ulog::warn!(
                "ui gateway: action dispatch failed action='{}' target='{}' method='{}' err='{e}'",
                action.action_id,
                action.target_gateway,
                action.method
            ),
        }
    }
}

fn action_dispatch_payload(action: &UiActionDispatch) -> Result<Vec<u8>, serde_json::Error> {
    if action.method == UI_SERVICE_METHOD_DISPATCH_ACTION_V1 {
        serde_json::to_vec(&UiDispatchActionRequest {
            surface_id: action.surface_id.clone(),
            action_id: action.action_id.clone(),
            payload: action.payload.clone(),
        })
    } else {
        serde_json::to_vec(&action.payload)
    }
}

fn log_ui_dispatch_frame(frame: &UiEventDispatchFrame) {
    let sampled = frame.frame_index <= 4 || frame.frame_index % 240 == 1;
    let active = !frame.actions.is_empty() || !frame.state_patches.is_empty();
    if !sampled && !active {
        return;
    }
    newengine_ulog_api::ulog::debug!(
        "ui.dispatch_input_v1 frame={} hovered={} hovered_action={} focused={} captured={} actions={} first_action={} patches={} capture_active={} diagnostics={}",
        frame.frame_index,
        frame.hovered_node.as_ref().map(|it| it.node_id.as_str()).unwrap_or("none"),
        frame.hovered_node.as_ref().and_then(|it| it.action_id.as_deref()).unwrap_or("none"),
        frame.focused_node.as_ref().map(|it| it.node_id.as_str()).unwrap_or("none"),
        frame.captured_pointer_owner.as_ref().map(|it| it.node_id.as_str()).unwrap_or("none"),
        frame.actions.len(),
        frame.actions.first().map(|it| it.action_id.as_str()).unwrap_or("none"),
        frame.state_patches.len(),
        frame.capture_state.active,
        frame.diagnostics.len(),
    );
}
