use super::*;

pub(super) fn push_editor_viewport(
    out: &mut Vec<UiComponentNode>,
    runtime_mode: UiEditorRuntimeMode,
    runtime_paused: bool,
    viewport_state: &UiEditorViewportState,
    layout: &EditorLayoutMetrics,
    active_menu_id: Option<&str>,
) {
    out.push(
        with_rect(
            UiComponentNode::row("editor.viewport.preview", EDITOR_CHROME.viewport_title)
                .with_value(if runtime_paused {
                    format!("{} | PAUSED", DEFAULT_VIEWPORT_SURFACE)
                } else {
                    DEFAULT_VIEWPORT_SURFACE.to_owned()
                })
                .with_detail(match runtime_mode {
                    UiEditorRuntimeMode::Edit => EDITOR_CHROME.viewport_detail_edit,
                    UiEditorRuntimeMode::Simulate => EDITOR_CHROME.viewport_detail_simulate,
                    UiEditorRuntimeMode::Play => EDITOR_CHROME.viewport_detail_play,
                })
                .with_tone(UiNodeTone::Accent)
                .with_tooltip("The game/world image is rendered into this UI-owned viewport block, not behind the editor")
                .tagged("viewport")
                .tagged("preview-window"),
            layout.viewport_x,
            layout.viewport_y,
            layout.viewport_w,
            28.0,
        ),
    );
    let viewport_toolbar_y = layout.viewport_y + 34.0;
    let viewport_toolbar_h = 25.0;
    let mut viewport_control_x = layout.viewport_x + 8.0;

    out.push(with_rect(
        viewport_toolbar_action(
            "editor.viewport.projection",
            viewport_state.projection.label(),
            "editor.viewport.projection",
            viewport_state.projection != UiEditorViewportProjection::Perspective,
            "Viewport projection: cycle Perspective / Top / Front / Side",
        ),
        viewport_control_x,
        viewport_toolbar_y,
        92.0,
        viewport_toolbar_h,
    ));
    viewport_control_x += 96.0;
    out.push(with_rect(
        viewport_toolbar_action(
            "editor.viewport.shading",
            viewport_state.shading.label(),
            "editor.viewport.shading",
            viewport_state.shading != UiEditorViewportShading::Lit,
            "Viewport shading: cycle Lit / Unlit / Wireframe",
        ),
        viewport_control_x,
        viewport_toolbar_y,
        68.0,
        viewport_toolbar_h,
    ));
    viewport_control_x += 72.0;
    out.push(with_rect(
        viewport_toolbar_action(
            "editor.viewport.show",
            "Show",
            "editor.viewport.show",
            active_menu_id == Some("__viewport_show"),
            "Toggle editor viewport overlays",
        ),
        viewport_control_x,
        viewport_toolbar_y,
        58.0,
        viewport_toolbar_h,
    ));
    viewport_control_x += 68.0;

    let transform_controls = [
        (
            "select",
            "Q",
            UiEditorTransformMode::Select,
            "editor.viewport.transform.select",
            "Select tool (Q)",
        ),
        (
            "translate",
            "W",
            UiEditorTransformMode::Translate,
            "editor.viewport.transform.translate",
            "Move/translate tool (W)",
        ),
        (
            "rotate",
            "E",
            UiEditorTransformMode::Rotate,
            "editor.viewport.transform.rotate",
            "Rotate tool (E)",
        ),
        (
            "scale",
            "R",
            UiEditorTransformMode::Scale,
            "editor.viewport.transform.scale",
            "Scale tool (R)",
        ),
    ];
    for (id, label, mode, action, tooltip) in transform_controls {
        out.push(with_rect(
            viewport_toolbar_action(
                format!("editor.viewport.transform.{id}"),
                label,
                action,
                viewport_state.transform_mode == mode,
                tooltip,
            ),
            viewport_control_x,
            viewport_toolbar_y,
            28.0,
            viewport_toolbar_h,
        ));
        viewport_control_x += 31.0;
    }

    out.push(with_rect(
        viewport_toolbar_action(
            "editor.viewport.transform.space",
            viewport_state.transform_space.label(),
            "editor.viewport.transform.space",
            viewport_state.transform_space == UiEditorTransformSpace::Local,
            "Toggle transform orientation between World and Local",
        ),
        viewport_control_x + 4.0,
        viewport_toolbar_y,
        52.0,
        viewport_toolbar_h,
    ));
    viewport_control_x += 60.0;

    if layout.viewport_w >= 760.0 {
        viewport_control_x += 7.0;
        out.push(with_rect(
            viewport_toolbar_action(
                "editor.viewport.snap.translate.toggle",
                "Grid",
                "editor.viewport.snap.translate.toggle",
                viewport_state.translation_snap_enabled,
                "Toggle translation grid snapping",
            ),
            viewport_control_x,
            viewport_toolbar_y,
            42.0,
            viewport_toolbar_h,
        ));
        viewport_control_x += 45.0;
        out.push(with_rect(
            viewport_toolbar_action(
                "editor.viewport.snap.translate.value",
                format!("{:.0}", viewport_state.translation_snap_units),
                "editor.viewport.snap.translate.value",
                viewport_state.translation_snap_enabled,
                "Cycle translation snap: 1 / 5 / 10 / 50 / 100 units",
            ),
            viewport_control_x,
            viewport_toolbar_y,
            38.0,
            viewport_toolbar_h,
        ));
        viewport_control_x += 43.0;
        out.push(with_rect(
            viewport_toolbar_action(
                "editor.viewport.snap.rotate.toggle",
                "Rot",
                "editor.viewport.snap.rotate.toggle",
                viewport_state.rotation_snap_enabled,
                "Toggle rotation angle snapping",
            ),
            viewport_control_x,
            viewport_toolbar_y,
            38.0,
            viewport_toolbar_h,
        ));
        viewport_control_x += 41.0;
        out.push(with_rect(
            viewport_toolbar_action(
                "editor.viewport.snap.rotate.value",
                format!("{:.0}°", viewport_state.rotation_snap_degrees),
                "editor.viewport.snap.rotate.value",
                viewport_state.rotation_snap_enabled,
                "Cycle rotation snap: 5 / 10 / 15 / 30 / 45 / 90 degrees",
            ),
            viewport_control_x,
            viewport_toolbar_y,
            42.0,
            viewport_toolbar_h,
        ));
        viewport_control_x += 47.0;
        out.push(with_rect(
            viewport_toolbar_action(
                "editor.viewport.snap.scale.toggle",
                "Scale",
                "editor.viewport.snap.scale.toggle",
                viewport_state.scale_snap_enabled,
                "Toggle scale snapping",
            ),
            viewport_control_x,
            viewport_toolbar_y,
            48.0,
            viewport_toolbar_h,
        ));
        viewport_control_x += 51.0;
        out.push(with_rect(
            viewport_toolbar_action(
                "editor.viewport.snap.scale.value",
                format!("{:.0}%", viewport_state.scale_snap_percent),
                "editor.viewport.snap.scale.value",
                viewport_state.scale_snap_enabled,
                "Cycle scale snap: 1 / 5 / 10 / 25 / 50 percent",
            ),
            viewport_control_x,
            viewport_toolbar_y,
            42.0,
            viewport_toolbar_h,
        ));
    }

    if active_menu_id == Some("__viewport_show") {
        let checked =
            |enabled: bool, label: &str| format!("[{}] {label}", if enabled { "x" } else { " " });
        let popup = UiComponentNode::row("editor.viewport.show_popup", "Show")
            .with_detail("Viewport visualization overlays")
            .with_tone(UiNodeTone::Accent)
            .with_prop("padding_px", serde_json::json!(4.0))
            .tagged("menu-popup")
            .tagged("viewport-menu")
            .tagged("floating")
            .with_child(
                UiComponentNode::action(
                    "editor.viewport.show_popup.grid",
                    checked(viewport_state.show_grid, "Grid"),
                    "editor.viewport.show.grid",
                )
                .tagged("button"),
            )
            .with_child(
                UiComponentNode::action(
                    "editor.viewport.show_popup.collision",
                    checked(viewport_state.show_collision, "Collision"),
                    "editor.viewport.show.collision",
                )
                .tagged("button"),
            )
            .with_child(
                UiComponentNode::action(
                    "editor.viewport.show_popup.bounds",
                    checked(viewport_state.show_bounds, "Bounds"),
                    "editor.viewport.show.bounds",
                )
                .tagged("button"),
            )
            .with_child(
                UiComponentNode::action(
                    "editor.viewport.show_popup.gizmos",
                    checked(viewport_state.gizmo_visible, "Transform Gizmo"),
                    "editor.viewport.show.gizmos",
                )
                .tagged("button"),
            );
        out.push(with_rect(
            popup,
            layout.viewport_x + 172.0,
            viewport_toolbar_y + viewport_toolbar_h + 2.0,
            190.0,
            132.0,
        ));
    }

    if viewport_state.gizmo_visible {
        out.push(with_rect(
            UiComponentNode::row(
                "editor.viewport.gizmos",
                viewport_state.transform_mode.label(),
            )
            .with_value(VIEWPORT_GIZMOS_NEUI_REF)
            .with_detail(format!(
                "{} | grid snap {} {:.0} | rotation snap {} {:.0}°",
                viewport_state.projection.label(),
                if viewport_state.translation_snap_enabled {
                    "on"
                } else {
                    "off"
                },
                viewport_state.translation_snap_units,
                if viewport_state.rotation_snap_enabled {
                    "on"
                } else {
                    "off"
                },
                viewport_state.rotation_snap_degrees,
            ))
            .with_tone(UiNodeTone::Accent)
            .tagged("viewport")
            .tagged("viewport-gizmos")
            .tagged("schema-driven"),
            layout.viewport_x + 10.0,
            (layout.viewport_y + layout.viewport_h - 36.0).max(viewport_toolbar_y + 34.0),
            (layout.viewport_w * 0.42).clamp(240.0, 520.0),
            28.0,
        ));
    }
}
