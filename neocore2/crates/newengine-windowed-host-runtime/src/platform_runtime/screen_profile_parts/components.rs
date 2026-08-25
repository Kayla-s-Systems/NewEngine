use super::*;
use newengine_ui_api::UiEditorSceneSnapshot;

mod bottom;
mod docks;
mod top_chrome;
mod viewport;

pub(super) fn editor_components(
    descriptor: &UiScreenProfileDescriptor,
    runtime_mode: UiEditorRuntimeMode,
    runtime_paused: bool,
    viewport_state: &UiEditorViewportState,
    scene_snapshot: &UiEditorSceneSnapshot,
    inspector_snapshot: &UiEditorInspectorSnapshot,
    authoring_state: &UiInGameEditorState,
    layout: &EditorLayoutMetrics,
    active_menu_id: Option<&str>,
) -> Vec<UiComponentNode> {
    let mut out = Vec::new();
    push_editor_regions(&mut out, layout);
    top_chrome::push_top_chrome(&mut out, authoring_state, layout, active_menu_id);
    docks::push_editor_docks(
        &mut out,
        descriptor,
        scene_snapshot,
        inspector_snapshot,
        layout,
    );
    viewport::push_editor_viewport(
        &mut out,
        runtime_mode,
        runtime_paused,
        viewport_state,
        layout,
        active_menu_id,
    );
    bottom::push_bottom_and_status(&mut out, descriptor, authoring_state, layout);
    out
}

pub(super) fn push_editor_regions(out: &mut Vec<UiComponentNode>, layout: &EditorLayoutMetrics) {
    out.push(region_panel(
        "editor.region.menu_bar",
        "",
        0.0,
        0.0,
        layout.screen_w,
        layout.menu_h,
        [11, 16, 24, 255],
    ));
    out.push(region_panel(
        "editor.region.toolbar",
        "",
        0.0,
        layout.menu_h,
        layout.screen_w,
        layout.toolbar_h,
        [8, 12, 18, 255],
    ));
    if layout.left_visible {
        out.push(region_panel(
            "editor.region.left_dock",
            "",
            6.0,
            layout.viewport_y,
            (layout.left_w - 8.0).max(1.0),
            layout.viewport_h,
            [8, 13, 20, 245],
        ));
    }
    // The renderer inserts the live UiViewportSlot texture before UI chrome.
    // Keep the viewport region as a transparent structural/border layer so the
    // editor shell never obscures the actual world image.
    out.push(region_panel(
        "editor.region.viewport",
        "",
        layout.viewport_x,
        layout.viewport_y,
        layout.viewport_w,
        layout.viewport_h,
        [5, 8, 12, 0],
    ));
    if layout.right_visible {
        out.push(region_panel(
            "editor.region.right_dock",
            "",
            layout.screen_w - layout.right_w + 2.0,
            layout.viewport_y,
            (layout.right_w - 8.0).max(1.0),
            layout.viewport_h,
            [8, 13, 20, 245],
        ));
    }
    if layout.bottom_visible {
        out.push(region_panel(
            "editor.region.bottom_dock",
            "",
            6.0,
            layout.bottom_y,
            (layout.screen_w - 12.0).max(1.0),
            (layout.bottom_h - 4.0).max(1.0),
            [5, 9, 15, 252],
        ));
    }
    out.push(region_panel(
        "editor.region.status",
        "",
        0.0,
        (layout.screen_h - layout.status_h).max(0.0),
        layout.screen_w,
        layout.status_h,
        [9, 13, 19, 255],
    ));
}

fn viewport_toolbar_action(
    id: impl Into<String>,
    label: impl Into<String>,
    action_id: impl Into<String>,
    active: bool,
    tooltip: impl Into<String>,
) -> UiComponentNode {
    lively_editor_action(UiComponentNode::action(id, label, action_id))
        .with_tone(if active {
            UiNodeTone::Accent
        } else {
            UiNodeTone::Normal
        })
        .with_tooltip(tooltip)
        .with_prop("fill_rgba", serde_json::json!([29, 34, 40, 238]))
        .with_prop("hover_fill_rgba", serde_json::json!([43, 51, 60, 248]))
        .with_prop("active_fill_rgba", serde_json::json!([31, 78, 120, 252]))
        .with_prop("radius_px", serde_json::json!(3.0))
        .tagged("viewport-toolbar")
        .tagged("button")
        .tagged(if active { "active" } else { "inactive" })
}

fn lively_editor_action(mut component: UiComponentNode) -> UiComponentNode {
    component
        .props
        .insert("interactive".to_owned(), serde_json::json!(true));
    component
        .props
        .insert("transition_ms".to_owned(), serde_json::json!(120));
    component.props.insert(
        "hover_border_rgba".to_owned(),
        serde_json::json!([101, 154, 210, 205]),
    );
    component.props.insert(
        "pressed_border_rgba".to_owned(),
        serde_json::json!([121, 181, 238, 235]),
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
        .insert("underline_height_px".to_owned(), serde_json::json!(1.0));
    component
}

pub(super) fn region_panel(
    id: &str,
    text: &str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fill: [u8; 4],
) -> UiComponentNode {
    let mut component = UiComponentNode::row(id, text)
        .with_tone(UiNodeTone::Normal)
        .tagged("region")
        .tagged("panel-region");
    component.component_id = UI_COMPONENT_PANEL.to_owned();
    component
        .props
        .insert("interactive".to_owned(), serde_json::json!(false));
    component
        .props
        .insert("draw_panel".to_owned(), serde_json::json!(true));
    component
        .props
        .insert("fill_rgba".to_owned(), serde_json::json!(fill));
    component.props.insert(
        "border_rgba".to_owned(),
        serde_json::json!([53, 68, 90, 155]),
    );
    component.props.insert(
        "radius_px".to_owned(),
        serde_json::json!(if h < 40.0 { 0.0 } else { 8.0 }),
    );
    with_rect(component, x, y, w, h)
}

pub(super) fn with_rect(
    mut component: UiComponentNode,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> UiComponentNode {
    set_rect(&mut component, x, y, w, h);
    component
}

pub(super) fn set_rect(component: &mut UiComponentNode, x: f32, y: f32, w: f32, h: f32) {
    // Generated editor chrome is authored in absolute pixel coordinates. Keep
    // Aurelia's layout/hit-test model on the same geometry as paint; without
    // this flag x_px/y_px are treated as flow metadata and visible controls get
    // hit boxes somewhere else in the document.
    component
        .props
        .insert("position".to_owned(), serde_json::json!("absolute"));
    component
        .props
        .insert("x_px".to_owned(), serde_json::json!(x.max(0.0)));
    component
        .props
        .insert("y_px".to_owned(), serde_json::json!(y.max(0.0)));
    component
        .props
        .insert("w_px".to_owned(), serde_json::json!(w.max(1.0)));
    component
        .props
        .insert("h_px".to_owned(), serde_json::json!(h.max(1.0)));
}

pub(super) fn menu_width(label: &str) -> f32 {
    (label.chars().count() as f32 * 8.0 + 24.0).clamp(44.0, 94.0)
}

pub(super) fn sort_components_by_layout_y(components: &mut [UiComponentNode]) {
    components.sort_by(|a, b| {
        let ay = component_layout_number(a, "y_px").unwrap_or(f32::MAX);
        let by = component_layout_number(b, "y_px").unwrap_or(f32::MAX);
        let ax = component_layout_number(a, "x_px").unwrap_or(f32::MAX);
        let bx = component_layout_number(b, "x_px").unwrap_or(f32::MAX);
        ay.partial_cmp(&by)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| component_paint_rank(a).cmp(&component_paint_rank(b)))
            .then_with(|| a.id.cmp(&b.id))
    });
}

pub(super) fn component_paint_rank(component: &UiComponentNode) -> u8 {
    if component
        .state_tags
        .iter()
        .any(|tag| tag == "region" || tag == "panel-region")
    {
        0
    } else {
        1
    }
}

pub(super) fn component_layout_number(component: &UiComponentNode, key: &str) -> Option<f32> {
    component
        .props
        .get(key)
        .and_then(|value| value.as_f64())
        .map(|value| value as f32)
}

pub(super) fn hovered_menu(layout: &EditorLayoutMetrics, id: &str) -> bool {
    layout.hovered_menu_id == Some(id)
}

#[cfg(test)]
mod geometry_tests {
    use super::*;

    #[test]
    fn set_rect_authors_absolute_geometry_for_provider_hit_testing() {
        let mut node =
            UiComponentNode::action("editor.toolbar.play", "", editor_command::RUNTIME_PLAY);
        set_rect(&mut node, 735.5, 34.0, 25.0, 22.0);

        assert_eq!(
            node.props
                .get("position")
                .and_then(serde_json::Value::as_str),
            Some("absolute")
        );
        assert_eq!(
            node.props.get("x_px").and_then(serde_json::Value::as_f64),
            Some(735.5)
        );
        assert_eq!(
            node.props.get("y_px").and_then(serde_json::Value::as_f64),
            Some(34.0)
        );
        assert_eq!(
            node.props.get("w_px").and_then(serde_json::Value::as_f64),
            Some(25.0)
        );
        assert_eq!(
            node.props.get("h_px").and_then(serde_json::Value::as_f64),
            Some(22.0)
        );
        assert_eq!(
            node.action_id.as_deref(),
            Some(editor_command::RUNTIME_PLAY)
        );
    }

    fn desktop_editor_layout() -> EditorLayoutMetrics {
        EditorLayoutMetrics {
            screen_w: 1600.0,
            screen_h: 900.0,
            menu_h: 28.0,
            toolbar_h: 40.0,
            status_h: 24.0,
            bottom_h: 220.0,
            left_w: 280.0,
            right_w: 340.0,
            gap: 6.0,
            viewport_x: 286.0,
            viewport_y: 68.0,
            viewport_w: 968.0,
            viewport_h: 588.0,
            bottom_y: 656.0,
            left_visible: true,
            right_visible: true,
            bottom_visible: true,
            hovered_dock_slot: None,
            hovered_menu_id: None,
        }
    }

    #[test]
    fn live_world_toolbar_has_save_exit_and_no_runtime_transport() {
        let layout = desktop_editor_layout();
        let state = UiInGameEditorState {
            enabled: true,
            dirty_placements: 3,
            pending_creates: 1,
            pending_deletes: 1,
            ..UiInGameEditorState::default()
        };
        let mut nodes = Vec::new();

        top_chrome::push_top_chrome(&mut nodes, &state, &layout, None);

        assert!(nodes
            .iter()
            .any(|node| node.id == "editor.toolbar.live_world"));
        assert!(nodes.iter().any(|node| {
            node.id == "editor.toolbar.save_map"
                && node.text == "Save (3)"
                && node.action_id.as_deref() == Some("game.editor.save")
        }));
        assert!(nodes.iter().any(|node| {
            node.id == "editor.toolbar.exit"
                && node.action_id.as_deref() == Some("game.editor.close")
        }));
        assert!(!nodes.iter().any(|node| {
            node.state_tags.iter().any(|tag| tag == "runtime-control")
                || matches!(
                    node.id.as_str(),
                    "editor.toolbar.play"
                        | "editor.toolbar.pause"
                        | "editor.toolbar.stop"
                        | "editor.toolbar.step"
                )
        }));
    }

    #[test]
    fn details_transform_publishes_nine_numeric_value_actions() {
        let layout = desktop_editor_layout();
        let descriptor = editing_overlay_descriptor();
        let scene = UiEditorSceneSnapshot::default();
        let inspector = UiEditorInspectorSnapshot {
            entity_key: Some(7),
            name: "Oak".to_owned(),
            kind: "Actor".to_owned(),
            transform: Some(newengine_ui_api::UiEditorInspectorTransformSnapshot {
                position: [1.0, 2.0, 3.0],
                rotation_degrees: [4.0, 5.0, 6.0],
                scale: [1.0, 1.0, 1.0],
            }),
            ..UiEditorInspectorSnapshot::default()
        };
        let mut nodes = Vec::new();

        docks::push_editor_docks(&mut nodes, &descriptor, &scene, &inspector, &layout);

        let inputs = nodes
            .iter()
            .filter(|node| {
                node.action_id
                    .as_deref()
                    .is_some_and(|id| id.starts_with("game.editor.transform."))
            })
            .collect::<Vec<_>>();
        assert_eq!(inputs.len(), 9);
        assert!(inputs
            .iter()
            .all(|node| node.component_id == UI_COMPONENT_INPUT));
        assert!(inputs.iter().any(|node| {
            node.action_id.as_deref() == Some("game.editor.transform.position.x")
                && node.value.as_deref() == Some("1.000")
        }));
        assert!(inputs.iter().any(|node| {
            node.action_id.as_deref() == Some("game.editor.transform.rotation.z")
                && node.value.as_deref() == Some("6.000")
        }));
    }
}
