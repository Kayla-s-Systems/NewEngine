use super::*;

pub(super) fn push_top_chrome(
    out: &mut Vec<UiComponentNode>,
    authoring_state: &UiInGameEditorState,
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

    // This is one paused live World, not a PIE clone or a second editor runtime.
    // Keep that invariant visible in the toolbar so entering the editor is never
    // mistaken for switching the game into another transport state.
    let save_failed = authoring_state.last_save_succeeded == Some(false);
    let unsaved_changes = authoring_state.dirty_placements;
    let (live_value, live_tone) = if save_failed {
        ("SAVE FAILED".to_owned(), UiNodeTone::Danger)
    } else if unsaved_changes > 0 {
        (
            format!("{unsaved_changes} UNSAVED CHANGE(S)"),
            UiNodeTone::Accent,
        )
    } else {
        ("ALL CHANGES SAVED".to_owned(), UiNodeTone::Normal)
    };
    let live_w = if layout.screen_w < 980.0 {
        200.0
    } else {
        290.0
    };
    let live_x = ((layout.screen_w - live_w) * 0.5).max(chrome_x);
    out.push(with_rect(
        UiComponentNode::row("editor.toolbar.live_world", "LIVE WORLD EDITOR")
            .with_value(live_value)
            .with_detail("Simulation paused · same running World")
            .with_tone(live_tone)
            .with_tooltip("Edits apply directly to the current game World; Ctrl+S writes authored map sources")
            .tagged("toolbar")
            .tagged("live-world")
            .tagged("authoring"),
        live_x,
        toolbar_y,
        live_w,
        toolbar_h,
    ));

    // Unified Editor Mode controls live on the right side of the primary toolbar.
    // Save/Exit are always visible so the user never has to guess whether changes
    // are transient or how to return to gameplay. Fly and noclip are mode status,
    // not separate debug toggles.
    let exit_w = 82.0;
    let save_w = 92.0;
    let button_gap = 6.0;
    let exit_x = (layout.screen_w - 12.0 - exit_w).max(8.0);
    let save_x = (exit_x - button_gap - save_w).max(8.0);
    let save_label = if unsaved_changes > 0 {
        format!("Save ({unsaved_changes})")
    } else {
        "Save".to_owned()
    };
    let save_tooltip = if authoring_state.last_save_message.trim().is_empty() {
        "Save authored map changes (Ctrl+S)".to_owned()
    } else {
        format!(
            "Save authored map changes (Ctrl+S) · {}",
            authoring_state.last_save_message
        )
    };
    let mut save_button = lively_editor_action(UiComponentNode::action(
        "editor.toolbar.save_map",
        save_label,
        "game.editor.save",
    ))
    .with_tone(if save_failed {
        UiNodeTone::Danger
    } else {
        UiNodeTone::Accent
    })
    .with_tooltip(save_tooltip)
    .tagged("toolbar")
    .tagged("authoring")
    .tagged("save")
    .tagged(if unsaved_changes > 0 {
        "dirty"
    } else {
        "clean"
    });
    save_button.props.insert(
        "enabled".to_owned(),
        serde_json::json!(authoring_state.save_available),
    );
    out.push(with_rect(save_button, save_x, toolbar_y, save_w, toolbar_h));
    out.push(with_rect(
        lively_editor_action(UiComponentNode::action(
            "editor.toolbar.exit",
            "Exit F2",
            "game.editor.close",
        ))
        .with_tooltip("Exit World Editor and return to gameplay (F2)")
        .tagged("toolbar")
        .tagged("authoring")
        .tagged("exit"),
        exit_x,
        toolbar_y,
        exit_w,
        toolbar_h,
    ));

    if layout.screen_w >= 1180.0 {
        let status_w = 236.0;
        let status_x = (save_x - 10.0 - status_w).max(8.0);
        let fly_label = if authoring_state.free_fly {
            "FLY ACTIVE"
        } else {
            "FLY OFF"
        };
        let noclip_label = if authoring_state.noclip {
            "NOCLIP ACTIVE"
        } else {
            "NOCLIP OFF"
        };
        out.push(with_rect(
            UiComponentNode::row("editor.toolbar.navigation_status", fly_label)
                .with_value(noclip_label)
                .with_detail("Hold RMB + WASD/Q/E · Shift faster")
                .with_tone(if authoring_state.free_fly && authoring_state.noclip {
                    UiNodeTone::Accent
                } else {
                    UiNodeTone::Danger
                })
                .with_tooltip(
                    "Editor camera is detached from player physics and gameplay collision",
                )
                .tagged("toolbar")
                .tagged("editor-camera")
                .tagged("noclip"),
            status_x,
            toolbar_y,
            status_w,
            toolbar_h,
        ));
    }
}
