#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::RenderBackendStatus;
use newengine_system_contracts::{ScreenOverlayReason, ScreenOverlayStatus};

pub fn overlay_from_render_backend_status(
    status: &RenderBackendStatus,
) -> Option<ScreenOverlayStatus> {
    if !status.degraded {
        return None;
    }

    let phase = status.phase.unwrap_or("unknown");
    let detail = status.message.as_deref().unwrap_or(
        "GPU backend entered degraded mode. Event loop is alive; renderer must be recreated.",
    );

    Some(ScreenOverlayStatus::degraded(
        ScreenOverlayReason::GpuDeviceLost,
        format!("Renderer backend degraded at {phase}"),
        detail,
    ))
}
