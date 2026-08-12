use newengine_core::render::SceneLaunchStatus;
use newengine_system_contracts::ScreenOverlayStatus;

pub(super) fn provider_draw_has_active_animation(draw: &newengine_ui_api::UiDrawList) -> bool {
    draw.paint.commands.iter().any(|command| match command {
        newengine_ui_api::UiPaintCommand::Rect(rect) => {
            rect.node.role == "hover-underline-animated"
        }
        _ => false,
    })
}

pub(super) fn loading_overlay_requires_immediate_publish(
    previous: Option<&ScreenOverlayStatus>,
    next: &ScreenOverlayStatus,
) -> bool {
    previous.is_none_or(|previous| {
        previous.kind != next.kind
            || previous.reason != next.reason
            || previous.title != next.title
            || previous.terminal != next.terminal
    })
}

#[inline]
pub(super) fn effective_scene_launch_active(
    status: Option<&SceneLaunchStatus>,
    presentation_blocks_world_bootstrap: bool,
) -> bool {
    status.is_some_and(|status| status.active) && !presentation_blocks_world_bootstrap
}
