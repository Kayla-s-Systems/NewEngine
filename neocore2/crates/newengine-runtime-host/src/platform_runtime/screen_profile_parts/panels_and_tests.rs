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
            "{} · {}",
            panel.source_gateway, panel.data_contract
        )),
    );
    component
}

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
    use serde_json::json;

    #[test]
    fn string_profile_can_select_game() {
        let parsed = parse_config_value(&json!("game")).unwrap();
        assert_eq!(parsed.profile, UiScreenProfile::Game);
    }

    #[test]
    fn object_profile_can_select_editor() {
        let parsed = parse_config_value(&json!({"profile":"editor"})).unwrap();
        assert_eq!(parsed.profile, UiScreenProfile::Editor);
        assert!(parsed.publish_editor_shell);
    }

    #[test]
    fn empty_object_profile_defaults_to_editor_shell() {
        let parsed = parse_config_value(&json!({})).unwrap();
        assert_eq!(parsed.profile, UiScreenProfile::Editor);
        assert!(parsed.publish_editor_shell);
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
