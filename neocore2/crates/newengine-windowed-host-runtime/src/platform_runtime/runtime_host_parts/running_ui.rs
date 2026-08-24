use newengine_core::render::SceneLaunchStatus;

pub(super) fn provider_draw_has_active_animation(draw: &newengine_ui_api::UiDrawList) -> bool {
    draw.paint.commands.iter().any(|command| match command {
        newengine_ui_api::UiPaintCommand::Rect(rect) => {
            rect.node.role == "hover-underline-animated"
        }
        _ => false,
    })
}

#[inline]
pub(super) fn effective_scene_launch_active(
    status: Option<&SceneLaunchStatus>,
    presentation_blocks_world_bootstrap: bool,
) -> bool {
    status.is_some_and(|status| status.active) && !presentation_blocks_world_bootstrap
}
