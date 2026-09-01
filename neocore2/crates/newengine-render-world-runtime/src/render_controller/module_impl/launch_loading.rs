#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::SceneLaunchStatus;

const SCENE_LAUNCH_PROGRESS_START: f32 = 0.90;
const SCENE_LAUNCH_PROGRESS_RANGE: f32 = 0.09;

#[inline]
fn scene_launch_progress(
    gate: &newengine_gameplay_world_runtime::gameplay::WorldActivationState,
) -> f32 {
    if gate.residency.total == 0 {
        return 0.95;
    }

    let ready = gate.residency.total.saturating_sub(gate.residency.waiting);
    let ratio = ready as f32 / gate.residency.total.max(1) as f32;
    (SCENE_LAUNCH_PROGRESS_START + ratio * SCENE_LAUNCH_PROGRESS_RANGE)
        .clamp(SCENE_LAUNCH_PROGRESS_START, 0.995)
}

#[inline]
pub(super) fn scene_launch_loading_status(
    gate: &newengine_gameplay_world_runtime::gameplay::WorldActivationState,
) -> SceneLaunchStatus {
    let progress = scene_launch_progress(gate);
    let detail = if gate.residency.total == 0 {
        gate.reason.clone()
    } else {
        format!(
            "{} | resources ready {}/{} | failed {}",
            gate.reason,
            gate.residency.total.saturating_sub(gate.residency.waiting),
            gate.residency.total,
            gate.residency.failed
        )
    };

    let status = if gate.reason.contains("physics provider")
        || gate.reason.contains("physics collision")
        || gate.reason.contains("collision registration")
    {
        "Preparing world collision..."
    } else if gate.reason.contains("static world") {
        "Streaming world geometry..."
    } else if gate.reason.contains("material") || gate.reason.contains("texture") {
        "Loading world materials..."
    } else {
        "Preparing playable world..."
    };

    SceneLaunchStatus::loading("NEWENGINE // LOADING WORLD", status, detail, progress)
}
