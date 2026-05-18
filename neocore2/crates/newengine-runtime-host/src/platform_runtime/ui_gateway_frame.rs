#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::EngineResult;
use newengine_ui::draw::UiDrawList;
use newengine_ui_api::{
    UiFrameRequest, UiFrameResponse, ENGINE_UI_SERVICE_ID, UI_SERVICE_METHOD_DRAW_FRAME_V1,
};

/// Requests a provider-owned UI draw list from the engine-facing UI gateway.
///
/// Runtime-host must not bind to any concrete UI implementation. It
/// submits only the stable UI frame request DTO to `engine.ui`; the host gateway
/// routes it to the active provider declared by descriptor metadata.
pub(crate) fn request_ui_draw_list(
    frame_index: u64,
    dt_sec: f32,
    surface_size_px: [u32; 2],
    pixels_per_point: f32,
) -> EngineResult<Option<UiDrawList>> {
    if !newengine_plugin_host::has_service(ENGINE_UI_SERVICE_ID) {
        return Ok(None);
    }

    let request = UiFrameRequest::new(frame_index, dt_sec, surface_size_px, pixels_per_point);
    let payload = serde_json::to_vec(&request)
        .map_err(|e| newengine_core::EngineError::other(format!("encode ui frame request failed: {e}")))?;

    let Some(bytes) = newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_DRAW_FRAME_V1,
        &payload,
    )
    .map_err(newengine_core::EngineError::other)? else {
        return Ok(None);
    };

    let response: UiFrameResponse = serde_json::from_slice(&bytes)
        .map_err(|e| newengine_core::EngineError::other(format!("decode ui frame response failed: {e}")))?;
    Ok(Some(response.draw_list))
}
