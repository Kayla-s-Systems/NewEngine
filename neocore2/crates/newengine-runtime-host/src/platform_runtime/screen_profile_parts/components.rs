fn editor_components(descriptor: &UiScreenProfileDescriptor, runtime_mode: UiEditorRuntimeMode, layout: &EditorLayoutMetrics, active_menu_id: Option<&str>) -> Vec<UiComponentNode> {
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
    let chrome_x = 16.0;
    let mut x = chrome_x;

    push_editor_regions(&mut out, layout);

    out.push(
        with_rect(
            UiComponentNode::row("editor.identity", EDITOR_CHROME.product_title)
                .with_value("Editor")
                .with_detail("engine.ui composition; render viewport is contained")
                .with_tone(UiNodeTone::Accent)
                .with_tooltip("Editor shell is a UI composition profile, not a backend domain")
                .tagged("identity")
                .tagged("top")
                .tagged("menu"),
            x,
            menu_y,
            118.0,
            menu_h,
        ),
    );
    x += 128.0;

    let mut active_popup_x = None;
    for menu in EDITOR_CHROME.menu {
        let hovered = hovered_menu(layout, menu.id);
        let active = active_menu_id == Some(menu.id);
        let menu_w = menu_width(menu.label);
        let mut row = UiComponentNode::action(format!("editor.menu.{}", menu.id), menu.label, format!("editor.menu.{}", menu.id))
            .with_tone(if hovered || active { UiNodeTone::Accent } else { UiNodeTone::Normal })
            .with_tooltip(menu.tooltip)
            .tagged("menu")
            .tagged("top")
            .tagged(if hovered { "hovered" } else { "idle" })
            .tagged(if active { "active" } else { "inactive" });
        row.props.insert("draw_panel".to_owned(), serde_json::json!(false));
        out.push(with_rect(row, x, menu_y, menu_w, menu_h));
        if active {
            active_popup_x = Some(x);
        }
        x += menu_w + 4.0;
    }

    if let Some(active_menu) = active_menu_id.and_then(|id| EDITOR_CHROME.menu.iter().find(|menu| menu.id == id)) {
        let popup_x = active_popup_x.unwrap_or(chrome_x + 128.0).min(layout.screen_w - 236.0).max(8.0);
        out.push(
            with_rect(
                UiComponentNode::row(format!("editor.menu_popup.{}", active_menu.id), active_menu.label)
                    .with_detail(active_menu.tooltip)
                    .with_tone(UiNodeTone::Accent)
                    .with_prop("padding_px", serde_json::json!(4.0))
                    .tagged("menu-popup")
                    .tagged("floating")
                    .with_child(UiComponentNode::action(format!("editor.menu_popup.{}.primary", active_menu.id), "Open Command Palette", "editor.command_palette.open").tagged("button"))
                    .with_child(UiComponentNode::action(format!("editor.menu_popup.{}.settings", active_menu.id), "Panel Settings", "editor.panel.settings").tagged("button")),
                popup_x,
                layout.menu_h + 2.0,
                224.0,
                58.0,
            ),
        );
    }

    let mut toolbar_x = chrome_x;
    for action in EDITOR_CHROME.runtime_actions {
        let hovered = layout.hovered_runtime_mode == Some(action.mode);
        let active = runtime_mode == action.mode;
        out.push(
            with_rect(
                UiComponentNode::action(format!("editor.toolbar.{}", action.id), action.label, action.action_id)
                    .with_value(action.hotkey)
                    .with_detail(action.tooltip)
                    .with_tone(if active { UiNodeTone::Accent } else { UiNodeTone::Normal })
                    .with_tooltip(action.tooltip)
                    .with_prop("hotkey", serde_json::json!(action.hotkey))
                    .tagged("toolbar")
                    .tagged("runtime-control")
                    .tagged(if active { "active" } else { "inactive" })
                    .tagged(if hovered { "hovered" } else { "idle" }),
                toolbar_x,
                toolbar_y,
                104.0,
                toolbar_h,
            ),
        );
        toolbar_x += 112.0;
    }
    out.push(
        with_rect(
            UiComponentNode::row("editor.toolbar.mode", "Mode")
                .with_value(runtime_mode.id())
                .with_detail("Editor boot default keeps simulation stopped")
                .with_tone(UiNodeTone::Accent)
                .tagged("toolbar")
                .tagged("runtime-mode"),
            toolbar_x + 4.0,
            toolbar_y,
            178.0,
            toolbar_h,
        ),
    );

    let dock_y = layout.viewport_y;
    if layout.left_visible {
        if let Some(panel) = descriptor.panels.iter().find(|panel| panel.slot_id == "left.scene_tree") {
            out.push(with_rect(panel_component(panel, true, layout.hovered_dock_slot == Some("left.scene_tree")), 8.0, dock_y, layout.left_w - 12.0, 28.0));
        }
        out.push(
            with_rect(
                UiComponentNode::row("editor.scene_tree.empty", EDITOR_CHROME.empty_outliner_title)
                    .with_value(SCENE_TREE_NEUI_REF)
                    .with_detail(EDITOR_CHROME.empty_outliner_detail)
                    .with_tone(UiNodeTone::Normal)
                    .with_tooltip("Scene Tree renders engine.scene/world snapshot DTO rows; no raw ECS traversal")
                    .tagged("scene-tree")
                    .tagged("empty-state"),
                14.0,
                dock_y + 34.0,
                (layout.left_w - 24.0).max(120.0),
                44.0,
            ),
        );
    }

    if layout.right_visible {
        let right_x = layout.screen_w - layout.right_w + 4.0;
        if let Some(panel) = descriptor.panels.iter().find(|panel| panel.slot_id == "right.inspector") {
            out.push(with_rect(panel_component(panel, true, layout.hovered_dock_slot == Some("right.inspector")), right_x, dock_y, layout.right_w - 12.0, 28.0));
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
                .with_value(DEFAULT_VIEWPORT_SURFACE)
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
    out.push(
        with_rect(
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
        ),
    );

    if layout.bottom_visible {
        let bottom_tabs = [
            ("bottom.asset_browser", "Asset Browser", ASSET_BROWSER_NEUI_REF, "engine.assets document/catalog DTO"),
            ("bottom.import_queue", "Import Queue", IMPORT_QUEUE_NEUI_REF, "engine.assets import/reimport job snapshot"),
            ("bottom.output_log", "Output Log", OUTPUT_LOG_NEUI_REF, "diagnostic/output log snapshot"),
            ("bottom.profiler_diagnostics", "Profiler / Diagnostics", PROFILER_DIAGNOSTICS_NEUI_REF, "route/job/profile diagnostics snapshot"),
        ];
        let mut tab_x = 8.0;
        for (slot, label, neui_ref, detail) in bottom_tabs {
            if let Some(panel) = descriptor.panels.iter().find(|panel| panel.slot_id == slot) {
                let mut component = panel_component(panel, true, layout.hovered_dock_slot == Some(slot));
                component.value = Some(neui_ref.to_owned());
                component.detail = Some(detail.to_owned());
                out.push(with_rect(component, tab_x, layout.bottom_y, 204.0, 28.0));
                tab_x += 212.0;
            }
        }
        out.push(
            with_rect(
                UiComponentNode::row("editor.bottom.placeholder", "Editor bottom dock")
                    .with_value("Asset Browser · Import Queue · Output Log · Profiler/Diagnostics")
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

    out.push(
        with_rect(
            UiComponentNode::row("editor.status", "Ready")
                .with_value(format!("mode={}", runtime_mode.id()))
                .with_detail("1 Stop · 2 Simulate · 3 Play · hover controls for hints")
                .with_tone(UiNodeTone::Normal)
                .tagged("status"),
            8.0,
            (layout.screen_h - layout.status_h - 4.0).max(0.0),
            (layout.screen_w - 16.0).max(32.0),
            layout.status_h,
        ),
    );
    out
}



fn push_editor_regions(out: &mut Vec<UiComponentNode>, layout: &EditorLayoutMetrics) {
    out.push(region_panel("editor.region.menu_bar", "", 0.0, 0.0, layout.screen_w, layout.menu_h, [11, 16, 24, 255]));
    out.push(region_panel("editor.region.toolbar", "", 0.0, layout.menu_h, layout.screen_w, layout.toolbar_h, [8, 12, 18, 255]));
    if layout.left_visible {
        out.push(region_panel("editor.region.left_dock", "", 6.0, layout.viewport_y, (layout.left_w - 8.0).max(1.0), layout.viewport_h, [8, 13, 20, 245]));
    }
    out.push(region_panel("editor.region.viewport", "", layout.viewport_x, layout.viewport_y, layout.viewport_w, layout.viewport_h, [5, 8, 12, 250]));
    if layout.right_visible {
        out.push(region_panel("editor.region.right_dock", "", layout.screen_w - layout.right_w + 2.0, layout.viewport_y, (layout.right_w - 8.0).max(1.0), layout.viewport_h, [8, 13, 20, 245]));
    }
    if layout.bottom_visible {
        out.push(region_panel("editor.region.bottom_dock", "", 6.0, layout.bottom_y, (layout.screen_w - 12.0).max(1.0), (layout.bottom_h - 4.0).max(1.0), [5, 9, 15, 252]));
    }
    out.push(region_panel("editor.region.status", "", 0.0, (layout.screen_h - layout.status_h).max(0.0), layout.screen_w, layout.status_h, [9, 13, 19, 255]));
}

fn region_panel(id: &str, text: &str, x: f32, y: f32, w: f32, h: f32, fill: [u8; 4]) -> UiComponentNode {
    let mut component = UiComponentNode::row(id, text)
        .with_tone(UiNodeTone::Normal)
        .tagged("region")
        .tagged("panel-region");
    component.component_id = UI_COMPONENT_PANEL.to_owned();
    component.props.insert("interactive".to_owned(), serde_json::json!(false));
    component.props.insert("draw_panel".to_owned(), serde_json::json!(true));
    component.props.insert("fill_rgba".to_owned(), serde_json::json!(fill));
    component.props.insert("border_rgba".to_owned(), serde_json::json!([53, 68, 90, 155]));
    component.props.insert("radius_px".to_owned(), serde_json::json!(if h < 40.0 { 0.0 } else { 8.0 }));
    with_rect(component, x, y, w, h)
}

fn with_rect(mut component: UiComponentNode, x: f32, y: f32, w: f32, h: f32) -> UiComponentNode {
    set_rect(&mut component, x, y, w, h);
    component
}

fn set_rect(component: &mut UiComponentNode, x: f32, y: f32, w: f32, h: f32) {
    component.props.insert("x_px".to_owned(), serde_json::json!(x.max(0.0)));
    component.props.insert("y_px".to_owned(), serde_json::json!(y.max(0.0)));
    component.props.insert("w_px".to_owned(), serde_json::json!(w.max(1.0)));
    component.props.insert("h_px".to_owned(), serde_json::json!(h.max(1.0)));
}

fn menu_width(label: &str) -> f32 {
    (label.chars().count() as f32 * 8.0 + 24.0).clamp(44.0, 94.0)
}

fn sort_components_by_layout_y(components: &mut [UiComponentNode]) {
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

fn component_paint_rank(component: &UiComponentNode) -> u8 {
    if component.state_tags.iter().any(|tag| tag == "region" || tag == "panel-region") { 0 } else { 1 }
}

fn component_layout_number(component: &UiComponentNode, key: &str) -> Option<f32> {
    component.props.get(key).and_then(|value| value.as_f64()).map(|value| value as f32)
}

fn hovered_menu(layout: &EditorLayoutMetrics, id: &str) -> bool {
    layout.hovered_menu_id == Some(id)
}
