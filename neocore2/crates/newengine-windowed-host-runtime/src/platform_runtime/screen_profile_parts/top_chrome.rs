use super::*;

fn style_transport_button(
    component: UiComponentNode,
    icon: &str,
    active: bool,
    enabled: bool,
) -> UiComponentNode {
    lively_editor_action(component)
        .with_tone(if active {
            UiNodeTone::Accent
        } else {
            UiNodeTone::Normal
        })
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
}

pub(super) fn push_top_chrome(
    out: &mut Vec<UiComponentNode>,
    runtime_mode: UiEditorRuntimeMode,
    runtime_paused: bool,
    runtime_possessed: bool,
    command_registry: &EditorCommandRegistry,
    layout: &EditorLayoutMetrics,
    active_menu_id: Option<&str>,
) {
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
        let popup_y = layout.menu_h + 2.0;
        if active_menu.id == "edit" {
            let popup = UiComponentNode::row("editor.menu_popup.edit", "Edit")
                .with_detail("Transaction history and actor editing")
                .with_tone(UiNodeTone::Accent)
                .with_prop("padding_px", serde_json::json!(4.0))
                .tagged("menu-popup")
                .tagged("floating")
                .with_child(
                    UiComponentNode::action(
                        "editor.menu_popup.edit.undo",
                        "Undo    Ctrl+Z",
                        "editor.history.undo",
                    )
                    .with_tooltip("Undo the last editor transform transaction")
                    .tagged("button"),
                )
                .with_child(
                    UiComponentNode::action(
                        "editor.menu_popup.edit.redo",
                        "Redo    Ctrl+Y",
                        "editor.history.redo",
                    )
                    .with_tooltip("Redo the last editor transform transaction")
                    .tagged("button"),
                )
                .with_child(
                    UiComponentNode::action(
                        "editor.menu_popup.edit.duplicate",
                        "Duplicate    Ctrl+D",
                        "editor.actor.duplicate",
                    )
                    .with_tooltip("Duplicate selected actor(s)")
                    .tagged("button"),
                )
                .with_child(
                    UiComponentNode::action(
                        "editor.menu_popup.edit.delete",
                        "Delete    Del",
                        "editor.actor.delete",
                    )
                    .with_tooltip("Delete selected actor(s)")
                    .tagged("button"),
                );
            out.push(with_rect(popup, popup_x, popup_y, 224.0, 112.0));
        } else {
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
                popup_y,
                224.0,
                58.0,
            ));
        }
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
        let button = style_transport_button(
            UiComponentNode::action(format!("editor.toolbar.{id}"), "", command_id),
            icon,
            active,
            enabled,
        )
        .with_tooltip(tooltip)
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
    let more = style_transport_button(
        UiComponentNode::action("editor.toolbar.more", "", "editor.runtime.more"),
        "more",
        false,
        true,
    )
    .with_tooltip("More runtime/editor commands")
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
        popup = popup.with_child(
            UiComponentNode::action(
                "editor.runtime_overflow.restart",
                "Restart Runtime",
                editor_command::RUNTIME_RESTART,
            )
            .tagged("button"),
        );
        let rows = if runtime_mode == UiEditorRuntimeMode::Play {
            3.0
        } else {
            2.0
        };
        out.push(with_rect(
            popup,
            overflow_x,
            layout.menu_h + layout.toolbar_h + 2.0,
            overflow_w,
            34.0 + rows * 24.0,
        ));
    }
}
