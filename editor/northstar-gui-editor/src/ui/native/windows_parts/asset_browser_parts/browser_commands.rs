use super::*;

pub(super) unsafe fn ctrl_key_down() -> bool {
    (GetKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0
}

pub(super) fn handle_editor_hotkey(state: &mut UiState, key: Wparam) -> bool {
    match key {
        VK_KEY_S => {
            save_active_editor_buffer(state);
            true
        }
        VK_KEY_Z => {
            undo_active_editor_buffer(state);
            true
        }
        VK_KEY_Y => {
            redo_active_editor_buffer(state);
            true
        }
        VK_KEY_A => {
            let Some(document) = state.active_document.as_mut() else {
                state.status = "Select all: no active text document".to_owned();
                return true;
            };
            let len = document.buffer.len();
            document.set_selections([TextSelection::range(0, len)]);
            reset_caret_blink(state);
            sync_legacy_cursor_from_document(state);
            state.status = format!("Selected all: {} bytes", len);
            true
        }
        _ => false,
    }
}

pub(super) fn handle_filter_key(state: &mut UiState, key: Wparam) -> bool {
    match key {
        VK_BACK => {
            state.filter_query.pop();
            state.selected_row = 0;
            state.scroll_rows = 0;
            state.status = format!("Filter: {}", state.filter_query);
            true
        }
        VK_ESCAPE => {
            state.filter_focus = false;
            state.filter_query.clear();
            state.status = "Search cleared".to_owned();
            true
        }
        _ => false,
    }
}

pub(super) fn handle_menu_dropdown_action(
    state: &mut UiState,
    _menu: &'static str,
    item: &'static str,
    rows: &[UiRow],
) {
    let command = menu_model::classify_menu_item(item);
    match command {
        MenuCommand::Toolbar(action) => handle_toolbar_action(state, action, rows),
        MenuCommand::SetViewMode(mode) => {
            state.view_mode = mode;
            state.status = format!("View mode: {mode}");
        }
        MenuCommand::FocusFilter => {
            state.filter_focus = true;
            state.status = "Filter field focused".to_owned();
        }
        MenuCommand::ClearFilter => {
            state.filter_query.clear();
            state.selected_row = 0;
            state.scroll_rows = 0;
            state.status = "Filter cleared".to_owned();
        }
        MenuCommand::ResetLayout => {
            state.scroll_rows = 0;
            state.tree_scroll_rows = 0;
            state.inspector_scroll_rows = 0;
            state.status = "Layout reset".to_owned();
        }
        MenuCommand::OpenModal(target) => {
            state.modal_dialog = Some(ModalDialogModel::message(
                "Editor",
                format!("Modal target: {target:?}"),
            ));
        }
        _ => state.status = format!("Menu action: {item}"),
    }
}

pub(super) fn handle_toolbar_action(state: &mut UiState, action: &str, rows: &[UiRow]) {
    match action {
        "Back" => {
            if state.preview_path.is_some() || state.xml_path.is_some() {
                clear_preview_editor(state);
                state.status = "Preview/editor closed".to_owned();
            } else if let Some(parent) = Path::new(&state.selected_path).parent() {
                state.selected_path = parent.display().to_string();
                state.tree_nodes = load_tree_nodes(Path::new(&state.root), &state.expanded_paths);
                state.status = format!("Back: {}", state.selected_path);
            }
        }
        "Open" | "Preview" => {
            if let Some(item) = rows.get(state.selected_row).or_else(|| rows.first()) {
                enter_row_or_select(state, item);
            } else {
                state.modal_dialog = Some(ModalDialogModel::message(
                    "Open",
                    "Nothing to open in current view.",
                ));
                state.status = "Open: nothing selected".to_owned();
            }
        }
        "Save" => save_active_editor_buffer(state),
        "Undo" => undo_active_editor_buffer(state),
        "Redo" => redo_active_editor_buffer(state),
        "Validate" => validate_selected_asset(state, rows),
        "Reload Types" => {
            state.tree_nodes = load_tree_nodes(Path::new(&state.root), &state.expanded_paths);
            state.status = "Workspace and runtime tool/type routes refreshed".to_owned();
        }
        "Theme" => cycle_editor_theme(state),
        _ => state.status = format!("Toolbar action: {action}"),
    }
}

pub(super) fn enter_row_or_select(state: &mut UiState, item: &UiRow) {
    let path = Path::new(item.path);
    if path.is_dir() {
        state.selected_path = item.path.to_owned();
        state.selected_row = 0;
        state.scroll_rows = 0;
        if !state.expanded_paths.iter().any(|p| p == item.path) {
            state.expanded_paths.push(item.path.to_owned());
        }
        state.tree_nodes = load_tree_nodes(Path::new(&state.root), &state.expanded_paths);
        state.status = format!("Opened folder: {}", item.name);
        return;
    }
    open_asset_in_preview(state, item);
}

pub(super) fn open_asset_in_preview(state: &mut UiState, item: &UiRow) {
    let asset_path = Path::new(item.path);
    let route = resolve_asset_route(state, asset_path);
    state.preview_path = Some(item.path.to_owned());
    state.preview_name = item.name.to_owned();
    state.preview_kind = item.kind.to_owned();
    state.preview_provider = route.provider_label.clone();
    state.preview_type_id = route.type_id.clone();
    state.preview_content_kind = route.content_kind.clone();
    state.preview_surface = route.preview_surface.clone();
    state.preview_size = item.size;
    state.inspector_scroll_rows = 0;
    state.modal_dialog = Some(ModalDialogModel::message(
        format!(
            "{} — {}",
            if is_text_editor_surface(&route) {
                "Text Editor"
            } else {
                "Preview Editor"
            },
            item.name
        ),
        format!(
            "Provider: {}\nType: {}\nPath: {}",
            state.preview_provider, state.preview_kind, item.path
        ),
    ));

    if let Some(tool_route) = route.tool_route.as_ref() {
        let output = run_tool_preview(tool_route, asset_path);
        state.preview_provider = output.provider_id;
        state.preview_lines = output.lines;
        if !output.command.is_empty() {
            state
                .preview_lines
                .insert(0, format!("Tool command: {}", output.command));
        }
        for diagnostic in output.diagnostics.into_iter().take(12).rev() {
            state
                .preview_lines
                .insert(1, format!("diagnostic: {diagnostic}"));
        }
        state.active_document = None;
        state.cached_spans.clear();
        state.xml_path = None;
        state.xml_lines.clear();
        state.xml_cursor_line = 0;
        state.xml_cursor_col = 0;
        state.xml_search_focus = false;
        state.xml_dirty = false;
        state.status = format!(
            "Opened provider preview: {} via {}",
            item.name, state.preview_provider
        );
        return;
    }

    if is_text_editor_surface(&route) {
        let content_kind = state.preview_content_kind.clone().unwrap_or_else(|| {
            builtin_content_kind_for_path(asset_path).unwrap_or_else(|| "text_document".to_owned())
        });
        match fs::read_to_string(asset_path) {
            Ok(text) => {
                let document = TextDocument::new(content_kind, text);
                state.active_document = Some(document);
                rebuild_cached_spans(state);
                sync_xml_render_cache_from_active_document(state);
                state.preview_lines = state.xml_lines.clone();
                state.xml_path = Some(item.path.to_owned());
                state.xml_cursor_line = 0;
                state.xml_cursor_col = 0;
                state.xml_search_focus = false;
                state.xml_dirty = false;
                state.status = format!(
                    "Opened text editor: {} type={} content_kind={} surface={} via {}",
                    item.name,
                    state.preview_type_id.as_deref().unwrap_or("<unknown>"),
                    state.preview_content_kind.as_deref().unwrap_or("<unknown>"),
                    state.preview_surface.as_deref().unwrap_or("<unknown>"),
                    state.preview_provider
                );
            }
            Err(err) => {
                state.active_document = None;
                state.xml_lines = vec![format!("Unable to read text document: {err}")];
                state.preview_lines = state.xml_lines.clone();
                state.xml_path = Some(item.path.to_owned());
                state.xml_dirty = false;
                state.status = format!("Text editor open failed: {err}");
            }
        }
    } else if is_package_path(asset_path) {
        state.active_document = None;
        state.cached_spans.clear();
        state.xml_path = None;
        state.xml_lines.clear();
        state.preview_lines = load_package_preview_lines(asset_path);
        state.status = format!(
            "Opened package preview: {} via {}",
            item.name, state.preview_provider
        );
    } else {
        state.active_document = None;
        state.cached_spans.clear();
        state.xml_path = None;
        state.xml_lines.clear();
        state.preview_lines =
            load_binary_preview_lines(asset_path, &state.preview_kind, &state.preview_provider);
        state.status = format!(
            "Opened provider preview: {} via {}",
            item.name, state.preview_provider
        );
    }
}
