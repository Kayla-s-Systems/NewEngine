use newengine_ui_api::{UiLayerCompositionPlan, UiLayerDomain, UiScreenInputFocusPolicy};

#[inline]
pub(super) fn should_request_shell_ui(
    provider_ui_active: bool,
    scene_launch_active: bool,
    provider_ui_needed: bool,
    provider_surface_ready: bool,
    game_ui_layer_active: bool,
    editor_overlay_active: bool,
    shell_plan_active: bool,
) -> bool {
    provider_ui_active
        && !scene_launch_active
        && shell_plan_active
        && (provider_ui_needed || provider_surface_ready)
        && (editor_overlay_active || !game_ui_layer_active)
}

#[inline]
pub(super) fn presentation_surface_domain(
    focus: UiScreenInputFocusPolicy,
) -> Option<UiLayerDomain> {
    match focus {
        UiScreenInputFocusPolicy::UiSurface => Some(UiLayerDomain::System),
        UiScreenInputFocusPolicy::GameViewport => Some(UiLayerDomain::GameViewport),
        UiScreenInputFocusPolicy::EditorShell => Some(UiLayerDomain::Editor),
        UiScreenInputFocusPolicy::Headless => None,
    }
}

#[inline]
pub(super) fn append_surface_once(plan: &mut UiLayerCompositionPlan, surface_id: &str) {
    let surface_id = surface_id.trim();
    if !surface_id.is_empty() && !plan.surface_ids.iter().any(|id| id == surface_id) {
        plan.surface_ids.push(surface_id.to_owned());
    }
}

#[inline]
pub(super) fn runtime_debug_overlay_allowed(game_profile_active: bool) -> bool {
    !game_profile_active
}
