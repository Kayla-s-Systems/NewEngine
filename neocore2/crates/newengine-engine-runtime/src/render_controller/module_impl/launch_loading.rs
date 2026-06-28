#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::SceneLaunchStatus;

const SCENE_LAUNCH_PROGRESS_START: f32 = 0.90;
const SCENE_LAUNCH_PROGRESS_RANGE: f32 = 0.09;

#[inline]
fn scene_launch_progress(gate: &crate::gameplay::GameReadyWorldLaunchGate) -> f32 {
    if gate.total_textures == 0 {
        return 0.95;
    }

    let ready = gate.total_textures.saturating_sub(gate.waiting_textures);
    let ratio = ready as f32 / gate.total_textures.max(1) as f32;
    (SCENE_LAUNCH_PROGRESS_START + ratio * SCENE_LAUNCH_PROGRESS_RANGE)
        .clamp(SCENE_LAUNCH_PROGRESS_START, 0.995)
}

#[inline]
pub(super) fn scene_launch_loading_status(
    gate: &crate::gameplay::GameReadyWorldLaunchGate,
) -> SceneLaunchStatus {
    let progress = scene_launch_progress(gate);
    let detail = if gate.total_textures == 0 {
        gate.reason.clone()
    } else {
        format!(
            "{} · resources ready {}/{} · failed {}",
            gate.reason,
            gate.total_textures.saturating_sub(gate.waiting_textures),
            gate.total_textures,
            gate.failed_textures
        )
    };

    SceneLaunchStatus::loading(
        "NEWENGINE // LOADING WORLD",
        "Preparing playable world...",
        detail,
        progress,
    )
}
