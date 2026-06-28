use super::*;

pub(super) unsafe fn handle_click(hwnd: Hwnd, x: i32, y: i32) {
    let client = client_rect(hwnd);
    let layout = make_layout(client);
    let rows = sample_rows();
    mutate_state(|state| {
        if let Some((menu, item)) = hit_menu_dropdown(x, y, state.menu_open) {
            state.menu_active = Some(menu);
            state.menu_item_hover = Some(item);
            state.toolbar_active = None;
            state.filter_focus = false;
            handle_menu_dropdown_action(state, menu, item, &rows);
            state.menu_open = None;
            state.menu_item_hover = None;
            return;
        }
        if let Some(menu) = hit_menu(x, y) {
            state.menu_active = Some(menu);
            state.menu_open = Some(menu);
            state.menu_item_hover = None;
            state.toolbar_active = None;
            state.filter_focus = false;
            state.status = format!("{menu} menu opened");
            return;
        }
        if point_in(toolbar_filter_rect(client), x, y) {
            state.filter_focus = true;
            state.menu_open = None;
            state.menu_item_hover = None;
            state.toolbar_active = None;
            state.status = "Search field focused".to_owned();
            return;
        }
        if let Some(action) = hit_toolbar(x, y) {
            state.filter_focus = false;
            state.menu_active = None;
            state.menu_open = None;
            state.menu_item_hover = None;
            state.toolbar_active = Some(action);
            handle_toolbar_action(state, action, &rows);
            return;
        }
        state.menu_active = None;
        state.menu_open = None;
        state.menu_item_hover = None;
        if point_in(layout.left, x, y) {
            let local_idx = ((y - (layout.left.top + 44)) / 24).max(0) as usize;
            let idx = state.tree_scroll_rows + local_idx;
            if let Some(node) = state.tree_nodes.get(idx).cloned() {
                state.selected_path = node.path.clone();
                state.selected_row = 0;
                state.scroll_rows = 0;
                state.inspector_scroll_rows = 0;
                if node.has_children {
                    toggle_expanded_path(&mut state.expanded_paths, &node.path);
                }
                state.tree_nodes = load_tree_nodes(Path::new(&state.root), &state.expanded_paths);
                state.selected_tree = find_tree_index(&state.tree_nodes, &state.selected_path)
                    .unwrap_or(idx.min(state.tree_nodes.len().saturating_sub(1)));
                state.status = if node.has_children {
                    format!("Toggled tree node: {}", node.label)
                } else {
                    format!("Selected tree node: {}", node.label)
                };
            }
            return;
        }
        if let Some(row) = hit_file_row(layout.center, x, y, state.scroll_rows, rows.len()) {
            state.selected_row = row;
            if let Some(item) = rows.get(row) {
                state.status = format!(
                    "Selected {} ({}) via provider {}",
                    item.name, item.kind, item.provider
                );
            }
            return;
        }
        if point_in(layout.right, x, y) {
            state.xml_search_focus = y >= layout.right.top + 44 && y <= layout.right.top + 70;
            state.status = if state.xml_search_focus {
                "XML search focused".to_owned()
            } else {
                "Inspector/Preview panel focused".to_owned()
            };
        }
    });
    process_pending_ui_requests(hwnd);
    sync_modal_window(hwnd);
    apply_ui_update(hwnd, UiUpdateRequest::Full);
}

pub(super) unsafe fn handle_double_click(hwnd: Hwnd, x: i32, y: i32) {
    let client = client_rect(hwnd);
    let layout = make_layout(client);
    let rows = sample_rows();
    mutate_state(|state| {
        if let Some(row) = hit_file_row(layout.center, x, y, state.scroll_rows, rows.len()) {
            if let Some(item) = rows.get(row) {
                enter_row_or_select(state, item);
            }
        }
    });
    sync_modal_window(hwnd);
    apply_ui_update(hwnd, UiUpdateRequest::Full);
}

pub(super) unsafe fn handle_mouse_up(_hwnd: Hwnd) {}

pub(super) unsafe fn handle_hover_panel(hwnd: Hwnd, x: i32, y: i32) {
    let client = client_rect(hwnd);
    let layout = make_layout(client);
    let rows_len = sample_rows().len();
    if cloned_state()
        .and_then(|state| state.modal_dialog)
        .is_some()
    {
        return;
    }
    let panel = if point_in(layout.left, x, y) {
        HoverPanel::Tree
    } else if point_in(layout.center, x, y) {
        HoverPanel::List
    } else if point_in(layout.right, x, y) {
        HoverPanel::Inspector
    } else {
        HoverPanel::None
    };
    let menu_hover = hit_menu(x, y);
    let toolbar_hover = hit_toolbar(x, y);

    let mut repaint_menu = false;
    let mut repaint_toolbar = false;
    let mut old_row = None;
    let mut new_row = None;
    let mut old_menu_rect = None;
    let mut new_menu_rect = None;

    mutate_state(|state| {
        let previous_open_menu = state.menu_open;
        let dropdown_hover = hit_menu_dropdown(x, y, state.menu_open).map(|(_, item)| item);
        if state.menu_hover != menu_hover || state.menu_item_hover != dropdown_hover {
            repaint_menu = true;
        }
        if state.menu_open.is_some() && menu_hover.is_some() && state.menu_open != menu_hover {
            old_menu_rect = previous_open_menu.and_then(menu_dropdown_rect);
            state.menu_active = menu_hover;
            state.menu_open = menu_hover;
            state.menu_item_hover = None;
            new_menu_rect = state.menu_open.and_then(menu_dropdown_rect);
            repaint_menu = true;
        } else {
            state.menu_item_hover = dropdown_hover;
        }

        if state.toolbar_hover != toolbar_hover {
            repaint_toolbar = true;
        }

        let next_row = hit_file_row(layout.center, x, y, state.scroll_rows, rows_len);
        if state.hover_row != next_row {
            old_row = state.hover_row;
            new_row = next_row;
            state.hover_row = next_row;
        }

        state.hover_panel = panel;
        state.menu_hover = menu_hover;
        state.toolbar_hover = toolbar_hover;
    });

    let mut update = UiUpdateRequest::None;
    if repaint_menu {
        update.push_region(menu_bar_rect(client));
        if let Some(rect) = old_menu_rect {
            update.push_region(expand_rect(rect, 2));
        }
        if let Some(rect) = new_menu_rect {
            update.push_region(expand_rect(rect, 2));
        }
        if old_menu_rect.is_none() && new_menu_rect.is_none() {
            update.push_region(menu_repaint_rect(client));
        }
    }
    if repaint_toolbar {
        update.push_region(toolbar_repaint_rect(client));
    }
    if old_row.is_some() || new_row.is_some() {
        if let Some(row) = old_row {
            update.push_region(file_row_rect(layout.center, row));
        }
        if let Some(row) = new_row {
            update.push_region(file_row_rect(layout.center, row));
        }
    }
    apply_ui_update(hwnd, update);
}

pub(super) unsafe fn handle_mouse_move(hwnd: Hwnd, x: i32, y: i32) {
    let client = client_rect(hwnd);
    let layout = make_layout(client);
    let rows_len = sample_rows().len();
    let mut old_row = None;
    let mut new_row = None;
    mutate_state(|state| {
        let next = hit_file_row(layout.center, x, y, state.scroll_rows, rows_len);
        if state.hover_row != next {
            old_row = state.hover_row;
            new_row = next;
            state.hover_row = next;
        }
    });
    let mut update = UiUpdateRequest::None;
    if let Some(row) = old_row {
        update.push_region(file_row_rect(layout.center, row));
    }
    if let Some(row) = new_row {
        update.push_region(file_row_rect(layout.center, row));
    }
    apply_ui_update(hwnd, update);
}

pub(super) unsafe fn handle_mouse_wheel(hwnd: Hwnd, delta: i32) {
    let list_max_scroll = sample_rows().len().saturating_sub(1);
    mutate_state(|state| {
        let down = delta < 0;
        match state.hover_panel {
            HoverPanel::Tree => {
                let max_scroll = state.tree_nodes.len().saturating_sub(1);
                if down {
                    state.tree_scroll_rows = (state.tree_scroll_rows + 1).min(max_scroll);
                } else {
                    state.tree_scroll_rows = state.tree_scroll_rows.saturating_sub(1);
                }
                state.status = format!("Scrolled tree to row offset {}", state.tree_scroll_rows);
            }
            HoverPanel::List => {
                if down {
                    state.scroll_rows = (state.scroll_rows + 1).min(list_max_scroll);
                } else {
                    state.scroll_rows = state.scroll_rows.saturating_sub(1);
                }
                state.status = format!("Scrolled asset list to row offset {}", state.scroll_rows);
            }
            HoverPanel::Inspector => {
                if down {
                    state.inspector_scroll_rows =
                        state.inspector_scroll_rows.saturating_add(1).min(64);
                } else {
                    state.inspector_scroll_rows = state.inspector_scroll_rows.saturating_sub(1);
                }
                state.status = format!(
                    "Scrolled inspector to row offset {}",
                    state.inspector_scroll_rows
                );
            }
            HoverPanel::None => {
                if down {
                    state.scroll_rows = (state.scroll_rows + 1).min(list_max_scroll);
                } else {
                    state.scroll_rows = state.scroll_rows.saturating_sub(1);
                }
                state.status = format!("Scrolled asset list to row offset {}", state.scroll_rows);
            }
        }
    });
    sync_modal_window(hwnd);
    apply_ui_update(hwnd, UiUpdateRequest::Full);
}

pub(super) unsafe fn handle_key(hwnd: Hwnd, key: Wparam) {
    let rows = sample_rows();
    mutate_state(|state| {
        state.status = format!(
            "WM_KEYDOWN main: key=0x{key:X} xml_path={} filter_focus={}",
            state.xml_path.is_some(),
            state.filter_focus
        );
        if ctrl_key_down() && state.xml_path.is_some() && handle_editor_hotkey(state, key) {
            return;
        }
        if state.modal_dialog.is_some() && key == VK_ESCAPE {
            state.modal_dialog = None;
            state.modal_dragging = false;
            state.status = "Dialog closed".to_owned();
            return;
        }
        if (state.preview_path.is_some() || state.xml_path.is_some()) && key == VK_ESCAPE {
            state.modal_dialog = None;
            clear_preview_editor(state);
            state.status = "Preview/editor closed".to_owned();
            return;
        }
        if state.filter_focus && handle_filter_key(state, key) {
            return;
        }
        if state.xml_path.is_some() && handle_xml_key(state, key) {
            return;
        }
        match key {
            VK_UP => state.selected_row = state.selected_row.saturating_sub(1),
            VK_DOWN => {
                state.selected_row = (state.selected_row + 1).min(rows.len().saturating_sub(1))
            }
            13 => {
                if let Some(item) = rows.get(state.selected_row) {
                    enter_row_or_select(state, item);
                }
                return;
            }
            VK_F5 => {
                state.status = "Reload Types requested".to_owned();
                return;
            }
            _ => return,
        }
        if let Some(item) = rows.get(state.selected_row) {
            state.status = format!("Keyboard selection: {}", item.name);
        }
    });
    sync_modal_window(hwnd);
    apply_ui_update(hwnd, UiUpdateRequest::Full);
}

pub(super) unsafe fn handle_char(hwnd: Hwnd, ch: Wparam) {
    mutate_state(|state| {
        let Some(ch) = char::from_u32(ch as u32) else {
            return;
        };
        state.status = format!(
            "WM_CHAR main: ch={:?} filter_focus={} xml_path={} xml_search_focus={}",
            ch,
            state.filter_focus,
            state.xml_path.is_some(),
            state.xml_search_focus
        );
        if ch == '\r' || ch == '\n' || ch == '\u{8}' || ch == '\u{1b}' {
            return;
        }
        if state.filter_focus {
            if !ch.is_control() {
                state.filter_query.push(ch);
                state.selected_row = 0;
                state.scroll_rows = 0;
                state.status = format!("Search: {}", state.filter_query);
            }
            return;
        }
        if state.xml_path.is_none() {
            return;
        }
        if state.xml_search_focus {
            if !ch.is_control() {
                state.xml_search_query.push(ch);
                state.status = format!("XML search: {}", state.xml_search_query);
            }
            return;
        }
        if !ch.is_control() {
            insert_char_into_active_document_or_xml_cache(state, ch);
            state.status = "Text buffer edited".to_owned();
        }
    });
    sync_modal_window(hwnd);
    apply_ui_update(hwnd, UiUpdateRequest::Full);
}
