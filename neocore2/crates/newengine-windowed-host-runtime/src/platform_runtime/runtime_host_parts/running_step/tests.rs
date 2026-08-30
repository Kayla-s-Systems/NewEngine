use super::*;
use newengine_ui_api::{UiGameGuiConfig, UiGameLayerDescriptor, UiScreenInputFocusPolicy};

#[test]
fn game_editor_and_debug_domains_remain_separate_packets() {
    let mut config = UiGameGuiConfig::simple_hud("ui/game/hud.neui@surface", "game.hud");
    config.layers.push(
        UiGameLayerDescriptor::menu("pause", "ui/game/pause.neui@surface", "game.pause")
            .initially_hidden(),
    );
    let state = UiGameLayerStackState::from_config(&config, 9);
    let game_plan = state.composition_plan(3);
    assert_eq!(game_plan.domain, UiLayerDomain::GameViewport);
    assert_eq!(game_plan.surface_ids, vec!["game.hud".to_owned()]);

    let mut debug_plan =
        UiLayerCompositionPlan::disabled(UiLayerDomain::Debug, UI_PRESENTATION_TARGET_PRIMARY, 9);
    debug_plan.surface_ids = vec![UI_SURFACE_RUNTIME_DEBUG_OVERLAY.to_owned()];

    let mut editor_plan =
        UiLayerCompositionPlan::disabled(UiLayerDomain::Editor, UI_PRESENTATION_TARGET_PRIMARY, 9);
    editor_plan.surface_ids = vec![UI_SURFACE_EDITOR_SHELL.to_owned()];

    let mut packets = UiLayerDrawPacketSet::new(9);
    packets.push(game_plan.draw_packet(newengine_ui_api::UiDrawList::new()));
    packets.push(editor_plan.draw_packet(newengine_ui_api::UiDrawList::new()));
    packets.push(debug_plan.draw_packet(newengine_ui_api::UiDrawList::new()));
    assert_eq!(
        packets
            .packets
            .iter()
            .map(|packet| packet.domain)
            .collect::<Vec<_>>(),
        vec![
            UiLayerDomain::GameViewport,
            UiLayerDomain::Editor,
            UiLayerDomain::Debug,
        ]
    );
}

#[test]
fn shipping_game_profile_never_composites_runtime_debug_overlay() {
    assert!(!runtime_debug_overlay_allowed(true));
    assert!(runtime_debug_overlay_allowed(false));
}

#[test]
fn active_editor_shell_is_not_suppressed_by_game_hud() {
    assert!(should_request_shell_ui(
        true, false, false, true, true, true, true,
    ));
    assert!(!should_request_shell_ui(
        true, false, false, true, true, false, true,
    ));
    assert!(!should_request_shell_ui(
        true, true, true, true, false, true, true,
    ));
}

#[test]
fn game_viewport_without_shell_surface_never_requests_empty_shell_draw() {
    assert!(!should_request_shell_ui(
        true, false, true, true, false, false, false,
    ));
}

#[test]
fn active_frontend_surface_still_requests_shell_draw() {
    assert!(should_request_shell_ui(
        true, false, true, true, false, false, true,
    ));
}

#[test]
fn project_presentation_surface_routes_to_system_lane_for_frontend() {
    assert_eq!(
        presentation_surface_domain(UiScreenInputFocusPolicy::UiSurface),
        Some(UiLayerDomain::System)
    );
}

#[test]
fn project_presentation_surface_routes_to_game_lane_for_hud() {
    assert_eq!(
        presentation_surface_domain(UiScreenInputFocusPolicy::GameViewport),
        Some(UiLayerDomain::GameViewport)
    );
}

#[test]
fn presentation_surface_is_deduplicated_in_composition_plan() {
    let mut plan =
        UiLayerCompositionPlan::disabled(UiLayerDomain::System, UI_PRESENTATION_TARGET_PRIMARY, 1);
    append_surface_once(&mut plan, "forest-road.frontend.title");
    append_surface_once(&mut plan, "forest-road.frontend.title");
    assert_eq!(
        plan.surface_ids,
        vec!["forest-road.frontend.title".to_owned()]
    );
}

#[test]
fn disabled_game_ui_stack_does_not_claim_a_render_lane() {
    let state = UiGameLayerStackState::default();
    assert!(!state.composition_plan(0).is_active());
}
