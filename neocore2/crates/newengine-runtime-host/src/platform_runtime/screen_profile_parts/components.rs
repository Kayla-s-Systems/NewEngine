use super::*;

pub(super) fn editor_components(
    descriptor: &UiScreenProfileDescriptor,
    runtime_mode: UiEditorRuntimeMode,
    runtime_paused: bool,
    runtime_possessed: bool,
    runtime_diff_count: usize,
    command_registry: &EditorCommandRegistry,
    layout: &EditorLayoutMetrics,
    active_menu_id: Option<&str>,
) -> Vec<UiComponentNode> {
    let mut out = Vec::new();

    // The editor shell is an absolute screen composition. Without explicit
    // regions the generic retained UI provider treats every top-level node as a
    // vertical row, which turns the menu/toolbar/dock tabs into one long opened
    // list. Keep the product structure in the runtime DTO, not in provider
    // branches.
    let menu_y = 6.0;
    let menu_h = (layout.menu_h - 8.0).max(20.0);
    let toolbar_y = layout.menu_h + 4.0;
    let toolbar_h = (layout.toolbar_h - 8.0).max(24.0);
    let chrome_x = if layout.screen_w < 760.0 { 8.0 } else { 16.0 };
    let compact_chrome = layout.screen_w < 900.0;
    let identity_w = if compact_chrome { 72.0 } else { 118.0 };
    let identity_label = if compact_chrome {
        "NS"
    } else {
        EDITOR_CHROME.product_title
    };
    let mut x = chrome_x;

    push_editor_regions(&mut out, layout);

    out.push(with_rect(
        UiComponentNode::row("editor.identity", identity_label)
            .with_value("Editor")
            .with_detail("engine.ui composition; render viewport is contained")
            .with_tone(UiNodeTone::Accent)
            .with_tooltip("Editor shell is a UI composition profile, not a backend domain")
            .tagged("identity")
            .tagged("top")
            .tagged("menu"),
        x,
        menu_y,
        identity_w,
        menu_h,
    ));
    x += identity_w + 10.0;

    let natural_menu_width = EDITOR_CHROME
        .menu
        .iter()
        .map(|menu| menu_width(menu.label) + 4.0)
        .sum::<f32>();
    let menu_scale = ((layout.screen_w - x - 8.0) / natural_menu_width.max(1.0)).clamp(0.68, 1.0);

    let mut active_popup_x = None;
    for menu in EDITOR_CHROME.menu {
        let hovered = hovered_menu(layout, menu.id);
        let active = active_menu_id == Some(menu.id);
        let menu_w = menu_width(menu.label) * menu_scale;
        let mut row = lively_editor_action(UiComponentNode::action(
            format!("editor.menu.{}", menu.id),
            menu.label,
            format!("editor.menu.{}", menu.id),
        ))
        .with_tone(if hovered || active {
            UiNodeTone::Accent
        } else {
            UiNodeTone::Normal
        })
        .with_tooltip(menu.tooltip)
        .tagged("menu")
        .tagged("top")
        .tagged(if hovered { "hovered" } else { "idle" })
        .tagged(if active { "active" } else { "inactive" });
        row.props
            .insert("draw_panel".to_owned(), serde_json::json!(false));
        out.push(with_rect(row, x, menu_y, menu_w, menu_h));
        if active {
            active_popup_x = Some(x);
        }
        x += menu_w + 4.0;
    }

    if let Some(active_menu) =
        active_menu_id.and_then(|id| EDITOR_CHROME.menu.iter().find(|menu| menu.id == id))
    {
        let popup_x = active_popup_x
            .unwrap_or(chrome_x + 128.0)
            .min(layout.screen_w - 236.0)
            .max(8.0);
        out.push(with_rect(
            UiComponentNode::row(
                format!("editor.menu_popup.{}", active_menu.id),
                active_menu.label,
            )
            .with_detail(active_menu.tooltip)
            .with_tone(UiNodeTone::Accent)
            .with_prop("padding_px", serde_json::json!(4.0))
            .tagged("menu-popup")
            .tagged("floating")
            .with_child(
                UiComponentNode::action(
                    format!("editor.menu_popup.{}.primary", active_menu.id),
                    "Open Command Palette",
                    "editor.command_palette.open",
                )
                .tagged("button"),
            )
            .with_child(
                UiComponentNode::action(
                    format!("editor.menu_popup.{}.settings", active_menu.id),
                    "Panel Settings",
                    "editor.panel.settings",
                )
                .tagged("button"),
            ),
            popup_x,
            layout.menu_h + 2.0,
            224.0,
            58.0,
        ));
    }

    // Compact transport strip: icon-only controls centered in the editor toolbar.
    // Runtime semantics stay in EditorCommandRegistry/RuntimeSession; this block owns
    // presentation only. Simulate/Restart remain available through shortcuts, console and the transport overflow.
    let command_context = EditorCommandContext {
        runtime_active: runtime_mode != UiEditorRuntimeMode::Edit,
        runtime_paused,
        runtime_playing: runtime_mode == UiEditorRuntimeMode::Play,
        runtime_possessed,
    };
    let transport_button_w = 25.0;
    let transport_gap = 1.0;
    let transport_pad = 3.0;
    let transport_count = 5.0;
    let transport_w = transport_pad * 2.0
        + transport_button_w * transport_count
        + transport_gap * (transport_count - 1.0);
    let transport_h = toolbar_h.min(27.0);
    let transport_y = toolbar_y + ((toolbar_h - transport_h) * 0.5).max(0.0);
    let transport_x = ((layout.screen_w - transport_w) * 0.5).max(chrome_x);

    let mut strip = region_panel(
        "editor.toolbar.transport",
        "",
        transport_x,
        transport_y,
        transport_w,
        transport_h,
        [48, 49, 51, 255],
    );
    strip.props.insert(
        "border_rgba".to_owned(),
        serde_json::json!([34, 35, 37, 255]),
    );
    strip
        .props
        .insert("radius_px".to_owned(), serde_json::json!(4.0));
    strip = strip.tagged("transport-strip");
    out.push(strip);

    let transport_specs = [
        (
            "play",
            editor_command::RUNTIME_PLAY,
            "play",
            Some(UiEditorRuntimeMode::Play),
            false,
        ),
        ("pause", editor_command::RUNTIME_PAUSE, "pause", None, true),
        ("stop", editor_command::RUNTIME_STOP, "stop", None, false),
        ("step", editor_command::RUNTIME_STEP, "step", None, false),
    ];
    let mut button_x = transport_x + transport_pad;
    for (id, command_id, icon, mode, pause_toggle) in transport_specs {
        let command = command_registry.get(command_id);
        let tooltip = command.map(|it| it.tooltip.as_str()).unwrap_or(command_id);
        let shortcut = command
            .and_then(|it| it.shortcut.as_ref())
            .map(|it| it.display.as_str())
            .unwrap_or("");
        let enabled = command
            .map(|it| it.enabled(command_context))
            .unwrap_or(false);
        let active = mode
            .map(|it| runtime_mode == it)
            .unwrap_or(pause_toggle && runtime_paused);
        let tooltip = if shortcut.is_empty() {
            tooltip.to_owned()
        } else {
            format!("{tooltip} ({shortcut})")
        };
        let button = lively_editor_action(UiComponentNode::action(
            format!("editor.toolbar.{id}"),
            "",
            command_id,
        ))
        .with_tone(if active {
            UiNodeTone::Accent
        } else {
            UiNodeTone::Normal
        })
        .with_tooltip(tooltip)
        .with_prop("transport_icon", serde_json::json!(icon))
        .with_prop("enabled", serde_json::json!(enabled))
        .with_prop("fill_rgba", serde_json::json!([58, 59, 61, 255]))
        .with_prop("hover_fill_rgba", serde_json::json!([70, 71, 74, 255]))
        .with_prop("pressed_fill_rgba", serde_json::json!([43, 44, 46, 255]))
        .with_prop("active_fill_rgba", serde_json::json!([64, 65, 67, 255]))
        .with_prop("disabled_fill_rgba", serde_json::json!([48, 49, 51, 255]))
        .with_prop("text_rgba", serde_json::json!([166, 168, 172, 255]))
        .with_prop(
            "disabled_text_rgba",
            serde_json::json!([101, 103, 107, 180]),
        )
        .with_prop("accent_rgba", serde_json::json!([111, 181, 67, 255]))
        .with_prop("radius_px", serde_json::json!(2.0))
        .tagged("toolbar")
        .tagged("runtime-control")
        .tagged("transport-button")
        .tagged(if active { "active" } else { "inactive" })
        .tagged(if enabled { "enabled" } else { "disabled" });
        out.push(with_rect(
            button,
            button_x,
            transport_y + 2.0,
            transport_button_w,
            (transport_h - 4.0).max(18.0),
        ));
        button_x += transport_button_w + transport_gap;
    }

    // Runtime overflow is deliberately independent from the application menu bar.
    // Transport controls must never mutate File/Edit/Create/Scene/Assets/Tools/Window/Help semantics.
    let more = lively_editor_action(UiComponentNode::action(
        "editor.toolbar.more",
        "",
        "editor.runtime.more",
    ))
    .with_tooltip("More runtime/editor commands")
    .with_prop("transport_icon", serde_json::json!("more"))
    .with_prop("enabled", serde_json::json!(true))
    .with_prop("fill_rgba", serde_json::json!([58, 59, 61, 255]))
    .with_prop("hover_fill_rgba", serde_json::json!([70, 71, 74, 255]))
    .with_prop("pressed_fill_rgba", serde_json::json!([43, 44, 46, 255]))
    .with_prop("text_rgba", serde_json::json!([166, 168, 172, 255]))
    .with_prop("radius_px", serde_json::json!(2.0))
    .tagged("toolbar")
    .tagged("transport-button")
    .tagged("overflow");
    out.push(with_rect(
        more,
        button_x,
        transport_y + 2.0,
        transport_button_w,
        (transport_h - 4.0).max(18.0),
    ));

    if active_menu_id == Some("__runtime_more") {
        let overflow_w = 224.0;
        let overflow_x = (button_x + transport_button_w - overflow_w)
            .max(8.0)
            .min((layout.screen_w - overflow_w - 8.0).max(8.0));
        let mut popup = UiComponentNode::row("editor.runtime_overflow", "Runtime")
            .with_detail(format!("PIE authored changes: {runtime_diff_count}"))
            .with_tone(UiNodeTone::Accent)
            .with_prop("padding_px", serde_json::json!(4.0))
            .tagged("menu-popup")
            .tagged("runtime-overflow")
            .tagged("floating")
            .with_child(
                UiComponentNode::action(
                    "editor.runtime_overflow.simulate",
                    "Simulate",
                    editor_command::RUNTIME_SIMULATE,
                )
                .tagged("button"),
            );
        if runtime_mode == UiEditorRuntimeMode::Play {
            let (id, label, command_id) = if runtime_possessed {
                ("eject", "Eject", editor_command::RUNTIME_EJECT)
            } else {
                ("possess", "Possess", editor_command::RUNTIME_POSSESS)
            };
            popup = popup.with_child(
                UiComponentNode::action(format!("editor.runtime_overflow.{id}"), label, command_id)
                    .tagged("button"),
            );
        }
        popup = popup
            .with_child(
                UiComponentNode::action(
                    "editor.runtime_overflow.restart",
                    "Restart Runtime",
                    editor_command::RUNTIME_RESTART,
                )
                .tagged("button"),
            )
            .with_child(
                UiComponentNode::action(
                    "editor.runtime_overflow.apply_changes",
                    if runtime_diff_count == 0 {
                        "Apply Changes & Stop".to_owned()
                    } else {
                        format!("Apply Changes & Stop ({runtime_diff_count})")
                    },
                    editor_command::RUNTIME_APPLY_CHANGES,
                )
                .tagged("button")
                .tagged(if runtime_diff_count == 0 {
                    "no-diff"
                } else {
                    "has-diff"
                }),
            );
        let rows = if runtime_mode == UiEditorRuntimeMode::Play {
            4.0
        } else {
            3.0
        };
        out.push(with_rect(
            popup,
            overflow_x,
            layout.menu_h + layout.toolbar_h + 2.0,
            overflow_w,
            34.0 + rows * 24.0,
        ));
    }

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
        out.push(with_rect(
            UiComponentNode::row(
                "editor.scene_tree.empty",
                EDITOR_CHROME.empty_outliner_title,
            )
            .with_value(SCENE_TREE_NEUI_REF)
            .with_detail(EDITOR_CHROME.empty_outliner_detail)
            .with_tone(UiNodeTone::Normal)
            .with_tooltip(
                "Scene Tree renders engine.scene/world snapshot DTO rows; no raw ECS traversal",
            )
            .tagged("scene-tree")
            .tagged("empty-state"),
            14.0,
            dock_y + 34.0,
            (layout.left_w - 24.0).max(120.0),
            44.0,
        ));
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
        out.push(
            with_rect(
                UiComponentNode::row("editor.inspector.empty", EDITOR_CHROME.empty_inspector_title)
                    .with_value(INSPECTOR_NEUI_REF)
                    .with_detail(EDITOR_CHROME.empty_inspector_detail)
                    .with_tone(UiNodeTone::Normal)
                    .with_tooltip("Select an entity, asset, asset entry, material or world item to populate this panel")
                    .tagged("inspector")
                    .tagged("right")
                    .tagged("empty-state"),
                right_x + 6.0,
                dock_y + 34.0,
                (layout.right_w - 24.0).max(160.0),
                44.0,
            ),
        );
    }

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
    out.push(with_rect(
        UiComponentNode::row("editor.viewport.gizmos", "Viewport Gizmos")
            .with_value(VIEWPORT_GIZMOS_NEUI_REF)
            .with_detail("translate/rotate/scale overlay consumes UiViewportSlot + selection DTO")
            .with_tone(UiNodeTone::Accent)
            .tagged("viewport")
            .tagged("viewport-gizmos")
            .tagged("schema-driven"),
        layout.viewport_x + 10.0,
        layout.viewport_y + 38.0,
        (layout.viewport_w * 0.34).clamp(220.0, 420.0),
        30.0,
    ));

    if layout.bottom_visible {
        let bottom_tabs = [
            (
                "bottom.asset_browser",
                "Asset Browser",
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
        out.push(
            with_rect(
                UiComponentNode::row("editor.bottom.placeholder", "Editor bottom dock")
                    .with_value("Asset Browser | Import Queue | Output Log | Profiler/Diagnostics")
                    .with_detail("All panels are UiNodeTreeRequest data and authored .neui surfaces; no provider-special product renderer")
                    .with_tone(UiNodeTone::Normal)
                    .tagged("bottom")
                    .tagged("editor-panels")
                    .tagged("neui-backed"),
                14.0,
                layout.bottom_y + 34.0,
                (layout.screen_w - 28.0).max(260.0),
                38.0,
            ),
        );
    }

    out.push(with_rect(
        UiComponentNode::row("editor.status", "Ready")
            .with_value(format!(
                "mode={}{}",
                runtime_mode.id(),
                if runtime_paused { " paused" } else { "" }
            ))
            .with_detail(
                "1 Stop | 2 Simulate | 3 Play | Space Pause/Resume | hover controls for hints",
            )
            .with_tone(UiNodeTone::Normal)
            .tagged("status"),
        8.0,
        (layout.screen_h - layout.status_h - 4.0).max(0.0),
        (layout.screen_w - 16.0).max(32.0),
        layout.status_h,
    ));
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
}
