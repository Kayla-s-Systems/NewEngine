use super::*;

pub(super) fn panel_component(
    panel: &UiScreenPanelDescriptor,
    visible: bool,
    hovered: bool,
) -> UiComponentNode {
    let mut component = UiComponentNode {
        id: panel.slot_id.clone(),
        component_id: UI_COMPONENT_ROW.to_owned(),
        text: panel.label.clone(),
        value: Some(panel.source_gateway.clone()),
        detail: Some(panel.data_contract.clone()),
        icon: None,
        font_token: None,
        tone: if !visible {
            UiNodeTone::Disabled
        } else if hovered || panel.required {
            UiNodeTone::Accent
        } else {
            UiNodeTone::Normal
        },
        state_tags: {
            let mut tags = panel.tags.clone();
            tags.push("dock-panel".to_owned());
            tags.push("closable".to_owned());
            tags.push("resizable".to_owned());
            tags.push("detachable".to_owned());
            tags.push(if visible {
                "visible".to_owned()
            } else {
                "hidden".to_owned()
            });
            tags.push(if hovered {
                "hovered".to_owned()
            } else {
                "idle".to_owned()
            });
            tags
        },
        action_id: Some(format!("editor.dock.toggle.{}", panel.slot_id)),
        props: BTreeMap::new(),
        children: Vec::new(),
    };
    component
        .props
        .insert("surface_id".to_owned(), serde_json::json!(panel.surface_id));
    component
        .props
        .insert("required".to_owned(), serde_json::json!(panel.required));
    component
        .props
        .insert("debug_only".to_owned(), serde_json::json!(panel.debug_only));
    component.props.insert(
        "dock_label".to_owned(),
        serde_json::json!(dock_slot_label(&panel.slot_id)),
    );
    component.props.insert(
        "panel_title".to_owned(),
        serde_json::json!(panel.label.as_str()),
    );
    component
        .props
        .insert("interactive".to_owned(), serde_json::json!(true));
    component
        .props
        .insert("transition_ms".to_owned(), serde_json::json!(120));
    component.props.insert(
        "hover_fill_rgba".to_owned(),
        serde_json::json!([31, 48, 70, 235]),
    );
    component.props.insert(
        "pressed_fill_rgba".to_owned(),
        serde_json::json!([22, 37, 56, 245]),
    );
    component
        .props
        .insert("underline_hover".to_owned(), serde_json::json!(true));
    component
        .props
        .insert("underline_duration_ms".to_owned(), serde_json::json!(135));
    component.props.insert(
        "underline_rgba".to_owned(),
        serde_json::json!([101, 170, 232, 220]),
    );
    component
        .props
        .insert("visible".to_owned(), serde_json::json!(visible));
    component
        .props
        .insert("dockable".to_owned(), serde_json::json!(true));
    component
        .props
        .insert("resizable".to_owned(), serde_json::json!(true));
    component
        .props
        .insert("detachable".to_owned(), serde_json::json!(true));
    component.props.insert(
        "tooltip".to_owned(),
        serde_json::json!(format!(
            "{} | {}",
            panel.source_gateway, panel.data_contract
        )),
    );
    component
}

#[allow(clippy::too_many_arguments)]
pub(super) fn screen_panel<const N: usize>(
    slot_id: &str,
    label: &str,
    surface_id: &str,
    source_gateway: &str,
    data_contract: &str,
    required: bool,
    debug_only: bool,
    tags: [&str; N],
) -> UiScreenPanelDescriptor {
    UiScreenPanelDescriptor {
        slot_id: slot_id.to_owned(),
        label: label.to_owned(),
        surface_id: surface_id.to_owned(),
        source_gateway: source_gateway.to_owned(),
        data_contract: data_contract.to_owned(),
        required,
        debug_only,
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
    }
}

pub(super) fn screen_metrics(
    descriptor: &UiScreenProfileDescriptor,
    frame_index: u64,
) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("frame_index".to_owned(), serde_json::json!(frame_index)),
        (
            "screen_profile".to_owned(),
            serde_json::json!(descriptor.profile.id()),
        ),
        (
            "layout_id".to_owned(),
            serde_json::json!(descriptor.layout_id.as_str()),
        ),
        (
            "viewport_surface_id".to_owned(),
            serde_json::json!(descriptor.viewport_surface_id.as_str()),
        ),
        (
            "input_focus_policy".to_owned(),
            serde_json::json!(format!("{:?}", descriptor.input_focus_policy)),
        ),
        ("gateway".to_owned(), serde_json::json!("engine.ui")),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_ui_api::UiGameInputMode;
    use serde_json::json;

    #[test]
    fn string_profile_can_select_game() {
        let parsed = parse_config_value(&json!("game")).unwrap();
        assert_eq!(parsed.profile, UiScreenProfile::Game);
    }

    #[test]
    fn object_profile_rejects_removed_editor_profile() {
        assert!(parse_config_value(&json!({"profile":"editor"})).is_err());
    }

    #[test]
    fn empty_object_profile_defaults_to_runtime_game() {
        let parsed = parse_config_value(&json!({})).unwrap();
        assert_eq!(parsed.profile, UiScreenProfile::Game);
        assert!(
            parsed.publish_editor_shell,
            "game profile should allow the editor shell capability; UiInGameEditorState still gates actual F2 visibility"
        );
    }

    #[test]
    fn presentation_flow_is_parsed_as_authored_state_graph() {
        let parsed = parse_config_value(&json!({
            "profile": "game",
            "presentation_flow": {
                "enabled": true,
                "id": "game.frontend",
                "initial_state": "main_menu",
                "states": [
                    {
                        "id": "main_menu",
                        "document_ref": "ui/frontend/main_menu.neui@surface",
                        "surface_id": "engine.main_menu",
                        "input_focus_policy": "ui_surface",
                        "blocks_world_bootstrap": true,
                        "blocks_gameplay_input": true
                    },
                    {
                        "id": "loading",
                        "blocks_world_bootstrap": false,
                        "blocks_gameplay_input": true
                    },
                    {
                        "id": "gameplay",
                        "document_ref": "ui/game/game_hud.neui@surface",
                        "surface_id": "game.hud",
                        "input_focus_policy": "game_viewport"
                    }
                ],
                "transitions": [
                    {
                        "from": "main_menu",
                        "to": "loading",
                        "on_action": "game.start",
                        "reset_runtime_ready": true
                    },
                    {
                        "from": "loading",
                        "to": "gameplay",
                        "on_runtime_ready": true
                    }
                ]
            }
        }))
        .unwrap();
        let flow = parsed.presentation_flow.expect("presentation flow");
        assert!(flow.is_valid());
        assert_eq!(flow.initial_state, "main_menu");
        assert_eq!(flow.states.len(), 3);
        assert_eq!(flow.transitions.len(), 2);
        assert!(
            flow.state("main_menu")
                .expect("main menu")
                .blocks_world_bootstrap
        );
    }

    #[test]
    fn presentation_flow_rejects_transition_to_unknown_state() {
        let parsed = parse_config_value(&json!({
            "profile": "game",
            "presentation_flow": {
                "enabled": true,
                "id": "broken",
                "initial_state": "main_menu",
                "states": [{"id": "main_menu"}],
                "transitions": [{
                    "from": "main_menu",
                    "to": "missing",
                    "on_action": "game.start"
                }]
            }
        }))
        .unwrap();
        assert!(!parsed.presentation_flow.expect("flow").is_valid());
    }

    #[test]
    fn presentation_flow_rejects_duplicate_states_and_ambiguous_actions() {
        let parsed = parse_config_value(&json!({
            "profile": "game",
            "presentation_flow": {
                "enabled": true,
                "id": "duplicate-flow",
                "initial_state": "main_menu",
                "states": [
                    {"id": "main_menu"},
                    {"id": "main_menu"},
                    {"id": "gameplay"}
                ],
                "transitions": [
                    {"from": "main_menu", "to": "gameplay", "on_action": "game.start"},
                    {"from": "main_menu", "to": "gameplay", "on_action": "game.start"}
                ]
            }
        }))
        .unwrap();
        let errors = parsed.presentation_flow.expect("flow").validation_errors();
        assert!(errors
            .iter()
            .any(|error| error.contains("duplicate state id")));
        assert!(errors
            .iter()
            .any(|error| error.contains("ambiguous action transition")));
    }

    #[test]
    fn presentation_flow_rejects_unreachable_states() {
        let parsed = parse_config_value(&json!({
            "profile": "game",
            "presentation_flow": {
                "enabled": true,
                "id": "unreachable-flow",
                "initial_state": "main_menu",
                "states": [
                    {"id": "main_menu"},
                    {"id": "gameplay"},
                    {"id": "credits"}
                ],
                "transitions": [
                    {"from": "main_menu", "to": "gameplay", "on_action": "game.start"}
                ]
            }
        }))
        .unwrap();
        let errors = parsed.presentation_flow.expect("flow").validation_errors();
        assert!(errors
            .iter()
            .any(|error| error.contains("state 'credits' is unreachable")));
    }

    #[test]
    fn presentation_flow_requires_document_and_surface_as_a_pair() {
        let parsed = parse_config_value(&json!({
            "profile": "game",
            "presentation_flow": {
                "enabled": true,
                "id": "surface-pair-flow",
                "initial_state": "main_menu",
                "states": [{
                    "id": "main_menu",
                    "document_ref": "ui/frontend/main_menu.neui@surface"
                }],
                "transitions": []
            }
        }))
        .unwrap();
        let errors = parsed.presentation_flow.expect("flow").validation_errors();
        assert!(errors
            .iter()
            .any(|error| error.contains("document_ref and surface_id together")));
    }

    #[test]
    fn presentation_flow_rejects_conflicting_runtime_ready_transitions() {
        let parsed = parse_config_value(&json!({
            "profile": "game",
            "presentation_flow": {
                "enabled": true,
                "id": "runtime-ready-flow",
                "initial_state": "loading",
                "states": [
                    {"id": "loading"},
                    {"id": "gameplay"},
                    {"id": "error"}
                ],
                "transitions": [
                    {"from": "loading", "to": "gameplay", "on_runtime_ready": true},
                    {"from": "loading", "to": "error", "on_runtime_ready": true}
                ]
            }
        }))
        .unwrap();
        let errors = parsed.presentation_flow.expect("flow").validation_errors();
        assert!(errors
            .iter()
            .any(|error| error.contains("ambiguous runtime_ready transition")));
    }

    #[test]
    fn game_gui_layer_stack_is_parsed_as_data_driven_runtime_config() {
        let parsed = parse_config_value(&json!({
            "profile": "game",
            "game_gui": {
                "enabled": true,
                "layers": [
                    {
                        "id": "hud",
                        "kind": "hud",
                        "document_ref": "ui/game/game_hud.neui@surface",
                        "surface_id": "game.hud",
                        "visible": true,
                        "input_mode": "game_only"
                    },
                    {
                        "id": "pause",
                        "kind": "menu",
                        "document_ref": "ui/game/pause_menu.neui@surface",
                        "surface_id": "game.pause",
                        "visible": false,
                        "input_mode": "ui_only"
                    }
                ]
            }
        }))
        .unwrap();

        let game_gui = parsed.game_gui.expect("game gui config");
        assert!(game_gui.enabled);
        assert!(game_gui.is_valid());
        let layers = game_gui.resolved_layers();
        assert_eq!(layers[0].id, "hud");
        assert_eq!(layers[0].z_order, UiGameLayerKind::Hud.default_z_order());
        assert_eq!(layers[1].id, "pause");
        assert_eq!(layers[1].input_mode, UiGameInputMode::UiOnly);
    }

    #[test]
    fn game_profile_keeps_game_ui_root_as_data() {
        let parsed = parse_config_value(&json!({
            "profile":"game",
            "game_ui_root_surface_id":"game.hud"
        }))
        .unwrap();
        assert_eq!(parsed.profile, UiScreenProfile::Game);
        assert_eq!(parsed.game_ui_root_surface_id.as_deref(), Some("game.hud"));
    }
}
