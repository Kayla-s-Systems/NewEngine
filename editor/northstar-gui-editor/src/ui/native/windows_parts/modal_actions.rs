use super::*;

pub(super) fn undo_active_editor_buffer(state: &mut UiState) {
    let Some(document) = state.active_document.as_mut() else {
        state.status = "Undo: no active text document".to_owned();
        return;
    };
    if document.undo() {
        sync_text_document_edit_cache(state, "Undo");
    } else {
        state.status = "Undo: nothing to undo".to_owned();
    }
}

pub(super) fn redo_active_editor_buffer(state: &mut UiState) {
    let Some(document) = state.active_document.as_mut() else {
        state.status = "Redo: no active text document".to_owned();
        return;
    };
    if document.redo() {
        sync_text_document_edit_cache(state, "Redo");
    } else {
        state.status = "Redo: nothing to redo".to_owned();
    }
}

pub(super) fn save_active_editor_buffer(state: &mut UiState) {
    let Some(path) = state.xml_path.clone() else {
        state.modal_dialog = Some(ModalDialogModel::message(
            "Save",
            "No editable text buffer is open.",
        ));
        state.status = "Save: no editable buffer".to_owned();
        return;
    };
    let text = state
        .active_document
        .as_ref()
        .map(|doc| doc.buffer.as_str().to_owned())
        .unwrap_or_else(|| state.xml_lines.join("\n"));
    match fs::write(&path, text) {
        Ok(()) => {
            state.xml_dirty = false;
            state.status = format!("Saved: {path}");
        }
        Err(err) => state.status = format!("Save failed: {err}"),
    }
}

pub(super) fn clear_preview_editor(state: &mut UiState) {
    state.preview_path = None;
    state.preview_name.clear();
    state.preview_kind.clear();
    state.preview_provider.clear();
    state.preview_size = None;
    state.preview_type_id = None;
    state.preview_content_kind = None;
    state.preview_surface = None;
    state.preview_lines.clear();
    state.active_document = None;
    state.xml_path = None;
    state.xml_lines.clear();
    state.xml_cursor_line = 0;
    state.xml_cursor_col = 0;
    state.xml_search_query.clear();
    state.xml_search_focus = false;
    state.xml_dirty = false;
}

pub(super) unsafe fn draw_builtin_tree_icon(hdc: Hdc, x: i32, y: i32, node: &TreeNode) {
    let color = if node.is_package {
        rgb(168, 85, 247)
    } else if node.has_children {
        rgb(234, 179, 8)
    } else {
        rgb(148, 163, 184)
    };
    fill(
        hdc,
        Rect {
            left: x,
            top: y,
            right: x + 14,
            bottom: y + 12,
        },
        color,
    );
    if node.is_expanded {
        line_frame(
            hdc,
            Rect {
                left: x + 2,
                top: y + 2,
                right: x + 12,
                bottom: y + 10,
            },
            rgb(255, 255, 255),
        );
    }
}

pub(super) unsafe fn draw_builtin_row_icon(hdc: Hdc, x: i32, y: i32, row: &UiRow, selected: bool) {
    let color = if row.kind == "Folder" {
        rgb(234, 179, 8)
    } else if row.kind.eq_ignore_ascii_case(".ytd") {
        rgb(14, 165, 233)
    } else {
        rgb(148, 163, 184)
    };
    let icon = Rect {
        left: x,
        top: y,
        right: x + 14,
        bottom: y + 14,
    };
    fill(hdc, icon, color);
    if selected {
        line_frame(hdc, icon, rgb(30, 64, 175));
    }
}

pub(super) unsafe fn draw_kv(hdc: Hdc, rect: Rect, y: &mut i32, key: &str, value: &str) {
    draw_text(
        hdc,
        Rect {
            left: rect.left + 12,
            top: *y,
            right: rect.left + 118,
            bottom: *y + 18,
        },
        key,
        rgb(51, 65, 85),
        true,
    );
    draw_text(
        hdc,
        Rect {
            left: rect.left + 120,
            top: *y,
            right: rect.right - 12,
            bottom: *y + 18,
        },
        value,
        rgb(15, 23, 42),
        false,
    );
    *y += 22;
}

pub(super) unsafe fn process_pending_ui_requests(owner: Hwnd) {
    let mut should_load_tools = false;
    mutate_state(|state| {
        if state.pending_load_tools_request {
            state.pending_load_tools_request = false;
            should_load_tools = true;
        }
    });
    if should_load_tools {
        run_load_tools_dialog(owner);
    }
}

pub(super) unsafe fn run_load_tools_dialog(owner: Hwnd) {
    let selected = pick_tool_directory(owner);
    match selected {
        Some(dir) => load_tools_from_directory(owner, dir),
        None => {
            mutate_state(|state| {
                state.modal_dialog = Some(ModalDialogModel::message(
                    "Load Tools",
                    "Tool loading cancelled.",
                ));
                state.status = "Load Tools cancelled".to_owned();
            });
        }
    }
}

pub(super) unsafe fn pick_tool_directory(owner: Hwnd) -> Option<PathBuf> {
    let title = to_wide("Select a directory with self-describing NorthStar tools");
    let mut display_name = [0u16; MAX_PATH];
    let mut browse = BrowseInfoW {
        hwnd_owner: owner,
        pidl_root: null(),
        psz_display_name: display_name.as_mut_ptr(),
        lpsz_title: title.as_ptr(),
        ul_flags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
        lpfn: None,
        l_param: 0,
        i_image: 0,
    };
    let pidl = SHBrowseForFolderW(&mut browse);
    if pidl.is_null() {
        return None;
    }
    let mut path_buf = [0u16; MAX_PATH];
    let ok = SHGetPathFromIDListW(pidl, path_buf.as_mut_ptr());
    CoTaskMemFree(pidl);
    if ok == 0 {
        return None;
    }
    let len = path_buf
        .iter()
        .position(|ch| *ch == 0)
        .unwrap_or(path_buf.len());
    if len == 0 {
        return None;
    }
    Some(PathBuf::from(String::from_utf16_lossy(&path_buf[..len])))
}

pub(super) unsafe fn load_tools_from_directory(_owner: Hwnd, dir: PathBuf) {
    let root = cloned_state().map(|state| state.root).unwrap_or_default();
    match discover_self_describing_tools(&dir) {
        Ok(result) => {
            let remember = ToolMountStore::remember_result(Path::new(&root), &dir, &result);
            mutate_state(|state| {
                let provider_count = result.providers.len();
                let registration_count = result.registrations.len();
                let capability_count: usize = result
                    .providers
                    .iter()
                    .map(|provider| provider.capabilities.len())
                    .sum();
                let preview_count = result
                    .providers
                    .iter()
                    .filter(|provider| {
                        provider
                            .capabilities
                            .iter()
                            .any(|capability| capability.contains("preview"))
                    })
                    .count();

                state.provider_count += provider_count;
                state.capability_count += capability_count;
                state.preview_provider_count += preview_count;
                state
                    .tool_routes
                    .extend(routes_from_providers(&result.providers));
                state.tool_routes.sort_by(|a, b| {
                    a.extension
                        .cmp(&b.extension)
                        .then(a.provider_id.cmp(&b.provider_id))
                });
                state.tool_routes.dedup_by(|a, b| {
                    a.extension == b.extension
                        && a.provider_id == b.provider_id
                        && a.executable == b.executable
                });
                state
                    .provider_ids
                    .extend(result.providers.iter().map(|provider| provider.id.clone()));
                state.provider_ids.sort();
                state.provider_ids.dedup();
                state.format_type_ids.extend(
                    result
                        .registrations
                        .iter()
                        .map(|registration| registration.type_id.clone()),
                );
                state.format_type_ids.sort();
                state.format_type_ids.dedup();
                state.format_type_count = state.format_type_ids.len();

                let remember_line = match remember {
                    Ok(()) => "mount remembered".to_owned(),
                    Err(err) => format!("mount remember failed: {err}"),
                };
                state.modal_dialog = Some(ModalDialogModel::message(
                    "Load Tools",
                    format!(
                        "Loaded tools from:\n{}\n\nAccepted tools: {}\nRuntime types: {}\nCapabilities: {}\n{}",
                        dir.display(),
                        provider_count,
                        registration_count,
                        capability_count,
                        remember_line,
                    ),
                ));
                state.status = format!("Loaded {} tools from {}", provider_count, dir.display());
            });
        }
        Err(err) => {
            mutate_state(|state| {
                state.modal_dialog = Some(ModalDialogModel::message(
                    "Load Tools failed",
                    format!("{}\n\n{}", dir.display(), err),
                ));
                state.status = "Load Tools failed".to_owned();
            });
        }
    }
}
