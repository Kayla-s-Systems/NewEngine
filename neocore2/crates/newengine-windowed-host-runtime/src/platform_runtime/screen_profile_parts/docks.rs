use super::*;

pub(super) fn push_editor_docks(
    out: &mut Vec<UiComponentNode>,
    descriptor: &UiScreenProfileDescriptor,
    scene_snapshot: &UiEditorSceneSnapshot,
    inspector_snapshot: &UiEditorInspectorSnapshot,
    layout: &EditorLayoutMetrics,
) {
    let dock_y = layout.viewport_y;
    if layout.left_visible {
        if let Some(panel) = descriptor
            .panels
            .iter()
            .find(|panel| panel.slot_id == "left.scene_tree")
        {
            out.push(with_rect(
                panel_component(
                    panel,
                    true,
                    layout.hovered_dock_slot == Some("left.scene_tree"),
                ),
                8.0,
                dock_y,
                layout.left_w - 12.0,
                28.0,
            ));
        }
        let content_x = 14.0;
        let content_w = (layout.left_w - 24.0).max(120.0);
        let content_y = dock_y + 34.0;
        if scene_snapshot.entities.is_empty() {
            out.push(with_rect(
                UiComponentNode::row(
                    "editor.scene_tree.empty",
                    EDITOR_CHROME.empty_outliner_title,
                )
                .with_value(SCENE_TREE_NEUI_REF)
                .with_detail(EDITOR_CHROME.empty_outliner_detail)
                .with_tone(UiNodeTone::Normal)
                .with_tooltip("World Outliner consumes UiEditorSceneSnapshot; no raw ECS traversal")
                .tagged("scene-tree")
                .tagged("empty-state"),
                content_x,
                content_y,
                content_w,
                44.0,
            ));
        } else {
            let row_h = 24.0;
            let available_h = (layout.viewport_h - 42.0).max(row_h);
            let max_rows = (available_h / row_h).floor().max(1.0) as usize;
            for (index, entity) in scene_snapshot.entities.iter().take(max_rows).enumerate() {
                let indent = if entity.parent_key.is_some() {
                    12.0
                } else {
                    0.0
                };
                let row = lively_editor_action(UiComponentNode::action(
                    format!("editor.scene_tree.entity.{}", entity.entity_key),
                    entity.name.clone(),
                    format!(
                        "engine.editor.selection.select_entity.{}",
                        entity.entity_key
                    ),
                ))
                .with_value(entity.kind.clone())
                .with_detail(if entity.components.is_empty() {
                    entity.kind.clone()
                } else {
                    entity.components.join(" · ")
                })
                .with_tone(if entity.selected {
                    UiNodeTone::Accent
                } else {
                    UiNodeTone::Normal
                })
                .with_tooltip(format!(
                    "{} · entity {}{}",
                    entity.kind,
                    entity.entity_key,
                    entity
                        .parent_key
                        .map(|parent| format!(" · parent {parent}"))
                        .unwrap_or_default(),
                ))
                .with_prop("selected", serde_json::json!(entity.selected))
                .with_prop("entity_key", serde_json::json!(entity.entity_key))
                .with_prop("parent_key", serde_json::json!(entity.parent_key))
                .tagged("scene-tree")
                .tagged("world-outliner-row")
                .tagged(if entity.selected {
                    "selected"
                } else {
                    "unselected"
                });
                out.push(with_rect(
                    row,
                    content_x + indent,
                    content_y + index as f32 * row_h,
                    (content_w - indent).max(80.0),
                    row_h - 1.0,
                ));
            }
            if scene_snapshot.entities.len() > max_rows {
                out.push(with_rect(
                    UiComponentNode::row(
                        "editor.scene_tree.overflow",
                        format!("+{} more actors", scene_snapshot.entities.len() - max_rows),
                    )
                    .with_detail("Resize the Outliner dock to show more")
                    .with_tone(UiNodeTone::Normal)
                    .tagged("scene-tree")
                    .tagged("overflow"),
                    content_x,
                    content_y + max_rows as f32 * row_h,
                    content_w,
                    row_h,
                ));
            }
        }
    }

    if layout.right_visible {
        let right_x = layout.screen_w - layout.right_w + 4.0;
        if let Some(panel) = descriptor
            .panels
            .iter()
            .find(|panel| panel.slot_id == "right.inspector")
        {
            out.push(with_rect(
                panel_component(
                    panel,
                    true,
                    layout.hovered_dock_slot == Some("right.inspector"),
                ),
                right_x,
                dock_y,
                layout.right_w - 12.0,
                28.0,
            ));
        }
        let details_x = right_x + 6.0;
        let details_w = (layout.right_w - 24.0).max(160.0);
        let mut details_y = dock_y + 34.0;
        if inspector_snapshot.entity_key.is_none() {
            out.push(with_rect(
                UiComponentNode::row(
                    "editor.inspector.empty",
                    EDITOR_CHROME.empty_inspector_title,
                )
                .with_value(INSPECTOR_NEUI_REF)
                .with_detail(EDITOR_CHROME.empty_inspector_detail)
                .with_tone(UiNodeTone::Normal)
                .with_tooltip("Select an actor in the viewport or World Outliner")
                .tagged("inspector")
                .tagged("details")
                .tagged("empty-state"),
                details_x,
                details_y,
                details_w,
                44.0,
            ));
        } else {
            out.push(with_rect(
                UiComponentNode::row("editor.inspector.actor", inspector_snapshot.name.clone())
                    .with_value(inspector_snapshot.kind.clone())
                    .with_detail(if inspector_snapshot.selection_count > 1 {
                        format!(
                            "{} actors selected · primary",
                            inspector_snapshot.selection_count
                        )
                    } else {
                        format!(
                            "Actor {}",
                            inspector_snapshot.entity_key.unwrap_or_default()
                        )
                    })
                    .with_tone(UiNodeTone::Accent)
                    .tagged("inspector")
                    .tagged("details")
                    .tagged("actor-header"),
                details_x,
                details_y,
                details_w,
                42.0,
            ));
            details_y += 46.0;

            if let Some(transform) = inspector_snapshot.transform.as_ref() {
                out.push(with_rect(
                    UiComponentNode::row("editor.inspector.transform.header", "Transform")
                        .with_detail("Actor transform")
                        .with_tone(UiNodeTone::Normal)
                        .tagged("inspector")
                        .tagged("component-header")
                        .tagged("transform"),
                    details_x,
                    details_y,
                    details_w,
                    24.0,
                ));
                details_y += 25.0;
                for (group_id, label, value, unit, step) in [
                    (
                        "position",
                        "Location",
                        transform.position,
                        "world units",
                        0.1,
                    ),
                    (
                        "rotation",
                        "Rotation",
                        transform.rotation_degrees,
                        "degrees",
                        1.0,
                    ),
                    ("scale", "Scale", transform.scale, "ratio", 0.01),
                ] {
                    let label_w = 66.0;
                    let gap = 4.0;
                    let input_w = ((details_w - label_w - gap * 4.0) / 3.0).max(38.0);
                    out.push(with_rect(
                        UiComponentNode::row(
                            format!("editor.inspector.transform.{group_id}.label"),
                            label,
                        )
                        .with_detail(unit)
                        .with_tone(UiNodeTone::Normal)
                        .tagged("inspector")
                        .tagged("transform-label"),
                        details_x + 4.0,
                        details_y,
                        label_w,
                        26.0,
                    ));
                    for (index, axis) in ["x", "y", "z"].into_iter().enumerate() {
                        out.push(with_rect(
                            transform_numeric_input(group_id, axis, value[index], step, unit),
                            details_x + label_w + gap * 2.0 + index as f32 * (input_w + gap),
                            details_y,
                            input_w,
                            26.0,
                        ));
                    }
                    details_y += 29.0;
                }
                details_y += 4.0;
            }

            out.push(with_rect(
                UiComponentNode::row("editor.inspector.components.header", "Components")
                    .with_detail(format!("{} attached", inspector_snapshot.components.len()))
                    .with_tone(UiNodeTone::Normal)
                    .tagged("inspector")
                    .tagged("component-header"),
                details_x,
                details_y,
                details_w,
                24.0,
            ));
            details_y += 25.0;
            let component_budget = (((layout.viewport_y + layout.viewport_h) - details_y) / 23.0)
                .floor()
                .max(1.0) as usize;
            for (index, component) in inspector_snapshot
                .components
                .iter()
                .take(component_budget)
                .enumerate()
            {
                out.push(with_rect(
                    UiComponentNode::row(
                        format!("editor.inspector.component.{index}"),
                        component.clone(),
                    )
                    .with_detail("Actor Component")
                    .with_tone(UiNodeTone::Normal)
                    .tagged("inspector")
                    .tagged("component-row"),
                    details_x + 4.0,
                    details_y + index as f32 * 23.0,
                    (details_w - 8.0).max(120.0),
                    22.0,
                ));
            }
        }
    }
}

fn transform_numeric_input(
    group_id: &str,
    axis: &str,
    value: f32,
    step: f32,
    unit: &str,
) -> UiComponentNode {
    let mut input = lively_editor_action(
        UiComponentNode::action(
            format!("editor.inspector.transform.{group_id}.{axis}"),
            axis.to_ascii_uppercase(),
            format!("game.editor.transform.{group_id}.{axis}"),
        )
        .with_value(format!("{value:.3}"))
        .with_detail(unit)
        .with_tone(UiNodeTone::Normal)
        .with_tooltip(format!(
            "Edit {} {} and press Enter",
            group_id,
            axis.to_ascii_uppercase()
        ))
        .tagged("inspector")
        .tagged("transform-input")
        .tagged("numeric-input"),
    );
    input.component_id = UI_COMPONENT_INPUT.to_owned();
    input
        .props
        .insert("numeric".to_owned(), serde_json::json!(true));
    input
        .props
        .insert("step".to_owned(), serde_json::json!(step));
    input
        .props
        .insert("commit_on_enter".to_owned(), serde_json::json!(true));
    input
        .props
        .insert("select_all_on_focus".to_owned(), serde_json::json!(true));
    input
        .props
        .insert("text_align".to_owned(), serde_json::json!("right"));
    input
}
