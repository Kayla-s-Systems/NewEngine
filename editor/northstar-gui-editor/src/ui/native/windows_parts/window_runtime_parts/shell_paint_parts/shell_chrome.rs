use super::*;

pub(super) unsafe fn draw_editor_shell(hdc: Hdc, client: Rect) {
    SetBkMode(hdc, TRANSPARENT);
    fill(hdc, client, rgb(246, 248, 250));
    draw_menu(hdc, client);
    draw_toolbar(hdc, client);
    let layout = make_layout(client);
    draw_status(hdc, layout.status);
    draw_panel(hdc, layout.left, "Asset Workspace");
    if let Some(state) = cloned_state() {
        draw_panel(hdc, layout.center, "Files and Assets");
        draw_panel(hdc, layout.right, "Inspector");
        draw_workspace_tree(hdc, layout.left, &state);
        draw_file_table(hdc, layout.center, &state);
        draw_asset_details_panel(hdc, layout.right, &state);
        draw_menu_dropdown(hdc, &state);
    } else {
        draw_panel(hdc, layout.center, "Files and Assets");
        draw_panel(hdc, layout.right, "Inspector");
    }
}

pub(super) unsafe fn draw_menu(hdc: Hdc, client: Rect) {
    let menu = Rect {
        left: 0,
        top: 0,
        right: client.right,
        bottom: 32,
    };
    fill(hdc, menu, rgb(252, 252, 252));
    line_frame(
        hdc,
        Rect {
            left: 0,
            top: 31,
            right: client.right,
            bottom: 32,
        },
        rgb(214, 219, 224),
    );
    let state = cloned_state();
    let hover = state.as_ref().and_then(|state| state.menu_hover);
    let active = state.as_ref().and_then(|state| state.menu_active);
    let open = state.as_ref().and_then(|state| state.menu_open);
    for item in menu_model::top_menu_items() {
        draw_menu_item(
            hdc,
            item.x,
            item.label,
            hover == Some(item.label),
            active == Some(item.label) || open == Some(item.label),
        );
    }
}

pub(super) unsafe fn draw_toolbar(hdc: Hdc, client: Rect) {
    fill(
        hdc,
        Rect {
            left: 0,
            top: 32,
            right: client.right,
            bottom: 78,
        },
        rgb(239, 243, 247),
    );
    line_frame(
        hdc,
        Rect {
            left: 0,
            top: 77,
            right: client.right,
            bottom: 78,
        },
        rgb(204, 211, 219),
    );
    let state = cloned_state();
    let hover = state.as_ref().and_then(|state| state.toolbar_hover);
    let active = state.as_ref().and_then(|state| state.toolbar_active);
    for button in toolbar_model::buttons() {
        draw_toolbar_button(
            hdc,
            button.x,
            42,
            button,
            hover == Some(button.label),
            active == Some(button.label),
        );
    }

    let filter = toolbar_filter_rect(client);
    let filter_focus = state
        .as_ref()
        .map(|state| state.filter_focus)
        .unwrap_or(false);
    fill(
        hdc,
        filter,
        if filter_focus {
            rgb(255, 255, 255)
        } else {
            rgb(248, 250, 252)
        },
    );
    line_frame(
        hdc,
        filter,
        if filter_focus {
            rgb(37, 99, 235)
        } else {
            rgb(185, 195, 207)
        },
    );
    let query = state
        .as_ref()
        .map(|state| state.filter_query.as_str())
        .unwrap_or("");
    let label = if query.is_empty() {
        "Search assets, providers, types".to_owned()
    } else {
        query.to_owned()
    };
    draw_text(
        hdc,
        Rect {
            left: filter.left + 12,
            top: filter.top + 5,
            right: filter.left + 58,
            bottom: filter.bottom - 3,
        },
        "Search",
        rgb(37, 99, 235),
        true,
    );
    draw_text(
        hdc,
        Rect {
            left: filter.left + 58,
            top: filter.top + 6,
            right: filter.right - 8,
            bottom: filter.bottom - 3,
        },
        &label,
        if query.is_empty() {
            rgb(90, 102, 118)
        } else {
            rgb(31, 41, 55)
        },
        false,
    );
}

pub(super) unsafe fn draw_status(hdc: Hdc, rect: Rect) {
    fill(hdc, rect, rgb(235, 239, 244));
    line_frame(
        hdc,
        Rect {
            left: 0,
            top: rect.top,
            right: rect.right,
            bottom: rect.top + 1,
        },
        rgb(204, 211, 219),
    );
    if let Some(state) = cloned_state() {
        let s = format!(
            "{}   |   Theme: {}   |   Providers: {}   Types: {}   Preview providers: {}",
            state.status,
            state.editor_theme.name,
            state.provider_count,
            state.format_type_count,
            state.preview_provider_count
        );
        draw_text(
            hdc,
            Rect {
                left: 14,
                top: rect.top + 5,
                right: rect.right - 14,
                bottom: rect.bottom,
            },
            &s,
            rgb(72, 83, 96),
            false,
        );
    }
}

pub(super) unsafe fn draw_panel(hdc: Hdc, rect: Rect, title: &str) {
    fill(hdc, rect, rgb(255, 255, 255));
    line_frame(hdc, rect, rgb(197, 205, 214));
    fill(
        hdc,
        Rect {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.top + 32,
        },
        rgb(241, 245, 249),
    );
    line_frame(
        hdc,
        Rect {
            left: rect.left,
            top: rect.top + 31,
            right: rect.right,
            bottom: rect.top + 32,
        },
        rgb(214, 220, 228),
    );
    draw_text(
        hdc,
        Rect {
            left: rect.left + 10,
            top: rect.top + 8,
            right: rect.right - 8,
            bottom: rect.top + 28,
        },
        title,
        rgb(31, 41, 55),
        true,
    );
}
