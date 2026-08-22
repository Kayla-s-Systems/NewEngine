use newengine_core::host_events::{CursorGrabMode, CursorState};
use newengine_core::{TaskLane, TaskPriority};
use newengine_platform_api::{
    PlatformCursorGrabModeV1, PlatformCursorPollV1, PlatformCursorStateV1,
};

#[inline]
pub(crate) fn cursor_poll_from_state(state: CursorState) -> PlatformCursorPollV1 {
    PlatformCursorPollV1 {
        has_value: true,
        state: PlatformCursorStateV1 {
            visible: state.visible,
            grab: match state.grab {
                CursorGrabMode::None => PlatformCursorGrabModeV1::None,
                CursorGrabMode::Confined => PlatformCursorGrabModeV1::Confined,
                CursorGrabMode::Locked => PlatformCursorGrabModeV1::Locked,
            },
        },
    }
}

pub(crate) fn render_backend_label_from_id(id: &str) -> String {
    id.rsplit('.')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or(id)
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_uppercase())
        .collect::<String>()
}

pub(crate) fn platform_task_lane(value: &str) -> TaskLane {
    match value.trim().to_ascii_lowercase().as_str() {
        "simulation" => TaskLane::Simulation,
        "render-prep" | "render_prep" | "renderprep" => TaskLane::RenderPrep,
        "streaming" => TaskLane::Streaming,
        "asset-io" | "asset_io" | "asset" => TaskLane::AssetIo,
        "plugin" | "plugins" => TaskLane::Plugin,
        "background" | "bg" => TaskLane::Background,
        _ => TaskLane::Background,
    }
}

pub(crate) fn platform_task_priority(value: &str) -> TaskPriority {
    match value.trim().to_ascii_lowercase().as_str() {
        "critical" => TaskPriority::Critical,
        "interactive" => TaskPriority::Interactive,
        "normal" => TaskPriority::Normal,
        "background" | "bg" => TaskPriority::Background,
        _ => TaskPriority::Normal,
    }
}

pub(crate) fn leak_task_label(value: &str, fallback: &'static str) -> &'static str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return fallback;
    }
    Box::leak(trimmed.to_owned().into_boxed_str())
}
