use super::*;

pub(super) fn push_bottom_and_status(
    out: &mut Vec<UiComponentNode>,
    descriptor: &UiScreenProfileDescriptor,
    authoring_state: &UiInGameEditorState,
    layout: &EditorLayoutMetrics,
) {
    if layout.bottom_visible {
        let bottom_tabs = [
            (
                "bottom.asset_browser",
                "Content Browser",
                ASSET_BROWSER_NEUI_REF,
                "engine.assets document/catalog DTO",
            ),
            (
                "bottom.import_queue",
                "Import Queue",
                IMPORT_QUEUE_NEUI_REF,
                "engine.assets import/reimport job snapshot",
            ),
            (
                "bottom.output_log",
                "Output Log",
                OUTPUT_LOG_NEUI_REF,
                "diagnostic/output log snapshot",
            ),
            (
                "bottom.profiler_diagnostics",
                "Profiler / Diagnostics",
                PROFILER_DIAGNOSTICS_NEUI_REF,
                "route/job/profile diagnostics snapshot",
            ),
        ];
        let mut tab_x = 8.0;
        for (slot, _label, neui_ref, detail) in bottom_tabs {
            if let Some(panel) = descriptor.panels.iter().find(|panel| panel.slot_id == slot) {
                let mut component =
                    panel_component(panel, true, layout.hovered_dock_slot == Some(slot));
                component.value = Some(neui_ref.to_owned());
                component.detail = Some(detail.to_owned());
                out.push(with_rect(component, tab_x, layout.bottom_y, 204.0, 28.0));
                tab_x += 212.0;
            }
        }
        out.push(with_rect(
            UiComponentNode::row("editor.bottom.placeholder", "Editor bottom dock")
                .with_value("Content Browser | Import Queue | Output Log | Profiler/Diagnostics")
                .with_detail("Live editor panels consume engine DTOs and authored .neui surfaces")
                .with_tone(UiNodeTone::Normal)
                .tagged("bottom")
                .tagged("editor-panels")
                .tagged("neui-backed"),
            14.0,
            layout.bottom_y + 34.0,
            (layout.screen_w - 28.0).max(260.0),
            38.0,
        ));
    }

    if !layout.bottom_visible {
        out.push(with_rect(
            viewport_toolbar_action(
                "editor.content_drawer.open",
                "Content Drawer",
                "editor.dock.toggle.bottom.asset_browser",
                false,
                "Open the Content Browser bottom drawer",
            ),
            8.0,
            (layout.screen_h - layout.status_h - 32.0).max(layout.viewport_y),
            122.0,
            24.0,
        ));
    }

    let save_failed = authoring_state.last_save_succeeded == Some(false);
    let creates = authoring_state.pending_creates;
    let deletes = authoring_state.pending_deletes;
    let modifies = authoring_state
        .dirty_placements
        .saturating_sub(creates)
        .saturating_sub(deletes);
    let (status_label, status_tone) = if save_failed {
        ("Save failed", UiNodeTone::Danger)
    } else if authoring_state.dirty_placements > 0 {
        ("Unsaved map changes", UiNodeTone::Accent)
    } else {
        ("Live World ready", UiNodeTone::Normal)
    };
    let status_tooltip = if authoring_state.last_save_message.trim().is_empty() {
        "Map changes are saved back to authored sources and rebuilt for runtime".to_owned()
    } else {
        authoring_state.last_save_message.clone()
    };

    out.push(with_rect(
        UiComponentNode::row("editor.status", status_label)
            .with_value(format!(
                "Create {creates} · Modify {modifies} · Delete {deletes}"
            ))
            .with_detail(
                "Hold RMB: WASD/Q/E Fly · Release RMB: Q/W/E/R Tools · Ctrl+S Save · F2 Exit",
            )
            .with_tone(status_tone)
            .with_tooltip(status_tooltip)
            .tagged("status")
            .tagged("live-world"),
        8.0,
        (layout.screen_h - layout.status_h - 4.0).max(0.0),
        (layout.screen_w - 16.0).max(32.0),
        layout.status_h,
    ));
}
