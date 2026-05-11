#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_system_contracts::{
    ScreenOverlayProgress, ScreenOverlayReason, ScreenOverlayStatus, ScreenOverlayStatusKind,
    SystemTaskPhase, SystemTaskStatus,
};

pub fn overlay_from_task_status(task: &SystemTaskStatus) -> Option<ScreenOverlayStatus> {
    let kind = match task.phase {
        SystemTaskPhase::Queued | SystemTaskPhase::Preparing | SystemTaskPhase::Running => {
            ScreenOverlayStatusKind::Loading
        }
        SystemTaskPhase::Applying => ScreenOverlayStatusKind::Applying,
        SystemTaskPhase::Completed => ScreenOverlayStatusKind::Ready,
        SystemTaskPhase::Failed => ScreenOverlayStatusKind::Error,
        SystemTaskPhase::Cancelled => return None,
    };

    Some(ScreenOverlayStatus::new(
        kind,
        ScreenOverlayReason::JobSystem,
        "NEWENGINE // TASK",
        task.label.clone(),
        task.detail.clone(),
        task.progress_01().map(ScreenOverlayProgress::percent),
        matches!(task.phase, SystemTaskPhase::Completed | SystemTaskPhase::Failed),
    ))
}
