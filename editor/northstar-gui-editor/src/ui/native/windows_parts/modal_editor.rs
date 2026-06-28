use super::*;

pub(super) fn modal_editor_scrollbar_rect(editor_rect: Rect) -> Rect {
    let client = Rect {
        left: 0,
        top: 0,
        right: editor_rect.right + 24,
        bottom: editor_rect.bottom + 72,
    };
    let geometry = EditorGeometry::from_modal_client(client);
    if geometry.editor.left == editor_rect.left && geometry.editor.top == editor_rect.top {
        geometry.scrollbar_rect()
    } else {
        Rect {
            left: editor_rect.right - 14,
            top: editor_rect.top + 38,
            right: editor_rect.right - 4,
            bottom: editor_rect.bottom - 4,
        }
    }
}

pub(super) fn modal_visible_editor_lines(editor_rect: Rect) -> usize {
    ((editor_rect.bottom - editor_rect.top - 54).max(20) / 20) as usize
}

pub(super) fn modal_preview_line_count(state: &UiState) -> usize {
    if state.xml_path.is_some() {
        state.xml_lines.len()
    } else {
        state.preview_lines.len()
    }
}

pub(super) unsafe fn modal_mouse_wheel(hwnd: Hwnd, delta: i32) {
    let client = client_rect(hwnd);
    let geometry = EditorGeometry::from_modal_client(client);
    let editor = geometry.editor;
    let visible = geometry.visible_lines();
    mutate_state(|state| {
        let total = modal_preview_line_count(state);
        let max_scroll = total.saturating_sub(visible);
        if delta < 0 {
            state.inspector_scroll_rows = (state.inspector_scroll_rows + 3).min(max_scroll);
        } else {
            state.inspector_scroll_rows = state.inspector_scroll_rows.saturating_sub(3);
        }
        state.status = format!("Preview scroll: {}", state.inspector_scroll_rows);
    });
    invalidate_rect(hwnd, editor);
}

pub(super) fn modal_editor_rect(client: Rect) -> Rect {
    EditorGeometry::from_modal_client(client).editor
}

pub(super) fn modal_document_offset_from_point(
    state: &UiState,
    editor: Rect,
    x: i32,
    y: i32,
) -> Option<usize> {
    if state.active_document.is_none() || state.xml_path.is_none() {
        return None;
    }
    let geometry = EditorGeometry {
        editor,
        metadata: editor,
        content: editor,
        text_top: editor.top + 46,
        line_left: editor.left + 66,
        gutter_left: editor.left + 8,
        line_height: 20,
        char_width: 8,
    };
    let (line, col) = geometry.line_col_from_point(x, y, state.inspector_scroll_rows)?;
    let document = state.active_document.as_ref()?;
    let offset = document.buffer.offset_for_line_column(line, col);
    Some(clamp_modal_text_offset(document.buffer.as_str(), offset))
}

pub(super) fn set_editor_caret_from_modal_point(
    state: &mut UiState,
    editor: Rect,
    x: i32,
    y: i32,
) -> Option<usize> {
    let offset = modal_document_offset_from_point(state, editor, x, y)?;
    let Some(document) = state.active_document.as_mut() else {
        return None;
    };
    document.set_carets([offset]);
    reset_caret_blink(state);
    sync_legacy_cursor_from_document(state);
    state.status = format!(
        "Caret: line={} col={}",
        state.xml_cursor_line + 1,
        state.xml_cursor_col
    );
    Some(offset)
}

pub(super) fn clamp_modal_text_offset(text: &str, mut offset: usize) -> usize {
    offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

pub(super) unsafe fn draw_editor_cursor(
    hdc: Hdc,
    line_left: i32,
    y: i32,
    right: i32,
    state: &UiState,
    line_index: usize,
    theme: &EditorColorDictionary,
) {
    if state.modal_text_selection_dragging {
        return;
    }
    let Some(document) = state.active_document.as_ref() else {
        return;
    };
    let Some(selection) = document.selections.first() else {
        return;
    };
    if !selection.is_caret() {
        return;
    }
    if !state.caret_visible {
        return;
    }
    let text = document.buffer.as_str();
    let Some((line_start, line_end)) = line_byte_range(text, line_index) else {
        return;
    };
    let cursor = clamp_modal_text_offset(text, selection.cursor);
    if cursor < line_start || cursor > line_end {
        return;
    }
    let col = text_columns(&text[line_start..cursor]);
    let x = (line_left + (col as i32).saturating_mul(8))
        .min(right - 2)
        .max(line_left);
    fill(
        hdc,
        Rect {
            left: x,
            top: y - 2,
            right: x + 2,
            bottom: y + 18,
        },
        sublime_foreground(theme),
    );
}

pub(super) fn text_columns(text: &str) -> usize {
    text.chars().map(|ch| if ch == '\t' { 4 } else { 1 }).sum()
}

pub(super) unsafe fn draw_editor_line_selection_background(
    hdc: Hdc,
    line_left: i32,
    y: i32,
    right: i32,
    state: &UiState,
    line_index: usize,
    theme: &EditorColorDictionary,
) {
    let Some(document) = state.active_document.as_ref() else {
        return;
    };
    let text = document.buffer.as_str();
    let Some((line_start, line_end)) = line_byte_range(text, line_index) else {
        return;
    };
    for selection in &document.selections {
        let (selection_start, selection_end) = selection.normalized();
        if selection_start == selection_end
            || selection_end <= line_start
            || selection_start > line_end
        {
            continue;
        }
        let start = clamp_modal_text_offset(text, selection_start.max(line_start));
        let end = clamp_modal_text_offset(text, selection_end.min(line_end));
        if start > line_end || end < line_start {
            continue;
        }
        if start == end && selection_end <= line_end {
            continue;
        }
        let start_col = text_columns(&text[line_start..start]);
        let end_col = if end > start {
            text_columns(&text[line_start..end])
        } else {
            text_columns(&text[line_start..line_end]).saturating_add(1)
        };
        let selection_left = line_left + (start_col as i32).saturating_mul(8);
        let selection_right = (line_left + (end_col as i32).saturating_mul(8))
            .min(right)
            .max(selection_left + 8);
        fill(
            hdc,
            Rect {
                left: selection_left,
                top: y - 2,
                right: selection_right,
                bottom: y + 18,
            },
            theme_color(theme.selection_background),
        );
    }
}

pub(super) fn modal_editor_line_band_rect(
    editor: Rect,
    state: &UiState,
    start_line: usize,
    end_line: usize,
) -> Option<Rect> {
    let visible_start = state.inspector_scroll_rows;
    let visible_count = modal_visible_editor_lines(editor);
    let visible_end = visible_start
        .saturating_add(visible_count)
        .saturating_sub(1);
    let start_line = start_line.max(visible_start);
    let end_line = end_line.min(visible_end);
    if start_line > end_line {
        return None;
    }
    let top =
        editor.top + 46 + (start_line.saturating_sub(visible_start) as i32).saturating_mul(20) - 4;
    let bottom =
        editor.top + 46 + (end_line.saturating_sub(visible_start) as i32).saturating_mul(20) + 22;
    Some(Rect {
        left: editor.left + 1,
        top: top.max(editor.top + 36),
        right: editor.right - 1,
        bottom: bottom.min(editor.bottom - 1),
    })
}

pub(super) fn union_optional_rect(a: Option<Rect>, b: Option<Rect>) -> Option<Rect> {
    match (a, b) {
        (Some(a), Some(b)) => Some(union_rect(a, b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

pub(super) fn modal_selection_repaint_rect(state: &UiState, editor: Rect) -> Option<Rect> {
    let Some(document) = state.active_document.as_ref() else {
        return None;
    };
    let mut repaint = None;
    for selection in &document.selections {
        let (start, end) = selection.normalized();
        let end_for_line = if end > start {
            end.saturating_sub(1)
        } else {
            end
        };
        let (start_line, _) = document.buffer.line_column_for_offset(start);
        let (end_line, _) = document.buffer.line_column_for_offset(end_for_line);
        repaint = union_optional_rect(
            repaint,
            modal_editor_line_band_rect(
                editor,
                state,
                start_line.min(end_line),
                start_line.max(end_line),
            ),
        );
    }
    repaint
}

pub(super) unsafe fn modal_handle_click(hwnd: Hwnd, x: i32, y: i32) {
    SetFocus(hwnd);
    let client = client_rect(hwnd);
    if point_in(modal_popup_primary_rect(client), x, y) {
        close_modal_popup();
        return;
    }
    let editor = modal_editor_rect(client);
    if point_in(editor, x, y) {
        let mut did_start_selection = false;
        let mut repaint_rect = None;
        mutate_state(|state| {
            let old_repaint = modal_selection_repaint_rect(state, editor);
            if let Some(offset) = set_editor_caret_from_modal_point(state, editor, x, y) {
                state.modal_text_selection_dragging = true;
                state.modal_text_selection_drag_anchor = Some(offset);
                did_start_selection = true;
                state.status = format!("Selection anchor: {}", offset);
                repaint_rect =
                    union_optional_rect(old_repaint, modal_selection_repaint_rect(state, editor));
            }
        });
        if did_start_selection {
            SetCapture(hwnd);
        }
        if let Some(rect) = repaint_rect {
            invalidate_rect(hwnd, rect);
        }
    }
}

pub(super) unsafe fn modal_handle_mouse_move(hwnd: Hwnd, x: i32, y: i32) {
    let client = client_rect(hwnd);
    let editor = modal_editor_rect(client);
    let mut repaint_rect = None;
    mutate_state(|state| {
        if !state.modal_text_selection_dragging {
            return;
        }
        let Some(anchor) = state.modal_text_selection_drag_anchor else {
            return;
        };
        let Some(current) = modal_document_offset_from_point(state, editor, x, y) else {
            return;
        };
        let old_repaint = modal_selection_repaint_rect(state, editor);
        let Some(document) = state.active_document.as_mut() else {
            return;
        };
        document.set_selections([TextSelection::range(anchor, current)]);
        sync_legacy_cursor_from_document(state);
        state.status = format!(
            "Selection: {}..{}",
            anchor.min(current),
            anchor.max(current)
        );
        repaint_rect =
            union_optional_rect(old_repaint, modal_selection_repaint_rect(state, editor));
    });
    if let Some(rect) = repaint_rect {
        invalidate_rect(hwnd, rect);
    }
}

pub(super) unsafe fn modal_handle_mouse_up(hwnd: Hwnd) {
    let client = client_rect(hwnd);
    let editor = modal_editor_rect(client);
    let mut was_dragging = false;
    let mut repaint_rect = None;
    mutate_state(|state| {
        repaint_rect = modal_selection_repaint_rect(state, editor);
        was_dragging = state.modal_text_selection_dragging;
        state.modal_text_selection_dragging = false;
        state.modal_text_selection_drag_anchor = None;
        if was_dragging {
            state.status = "Selection drag finished".to_owned();
        }
    });
    if was_dragging {
        ReleaseCapture();
        if let Some(rect) = repaint_rect {
            invalidate_rect(hwnd, rect);
        }
    }
}

pub(super) unsafe fn modal_drag_move(hwnd: Hwnd, x: i32, y: i32) {
    let mut move_to = None;
    mutate_state(|state| {
        if state.modal_dragging {
            let mut rect: Rect = zeroed();
            GetWindowRect(hwnd, &mut rect);
            let new_x = rect.left + x - state.modal_drag_dx;
            let new_y = rect.top + y - state.modal_drag_dy;
            move_to = Some((new_x, new_y));
        }
    });
    if let Some((new_x, new_y)) = move_to {
        MoveWindow(hwnd, new_x, new_y, MODAL_WIDTH, MODAL_HEIGHT, 1);
    }
}

pub(super) unsafe fn modal_finish_drag(_hwnd: Hwnd) {
    let mut was_dragging = false;
    mutate_state(|state| {
        was_dragging = state.modal_dragging;
        state.modal_dragging = false;
        if was_dragging {
            state.status = "Dialog drag finished".to_owned();
        }
    });
    if was_dragging {
        ReleaseCapture();
    }
}

pub(super) unsafe fn sync_modal_window(owner: Hwnd) {
    let dialog = cloned_state().and_then(|state| state.modal_dialog);
    match dialog {
        Some(dialog) => show_modal_popup(owner, &dialog),
        None => destroy_modal_popup(),
    }
}

pub(super) unsafe fn show_modal_popup(owner: Hwnd, dialog: &ModalDialogModel) {
    let mut existing_raw = 0usize;
    mutate_state(|state| existing_raw = state.modal_hwnd);
    let existing = existing_raw as Hwnd;
    if existing.is_null() {
        let h_instance = GetModuleHandleW(null());
        let class_name = to_wide("NorthStarGuiEditorModalWindow");
        let title = to_wide(&dialog.title);
        let (x, y) = centered_popup_position(owner);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_POPUP | WS_CAPTION | WS_SYSMENU,
            x,
            y,
            MODAL_WIDTH,
            MODAL_HEIGHT,
            owner,
            null_mut(),
            h_instance,
            null_mut(),
        );
        if !hwnd.is_null() {
            mutate_state(|state| state.modal_hwnd = hwnd as usize);
            ShowWindow(hwnd, SW_SHOW);
            SetFocus(hwnd);
            UpdateWindow(hwnd);
        }
    } else {
        let title = to_wide(&dialog.title);
        SetWindowTextW(existing, title.as_ptr());
        ShowWindow(existing, SW_SHOW);
        SetFocus(existing);
        InvalidateRect(existing, null(), 0);
        UpdateWindow(existing);
    }
}

pub(super) unsafe fn destroy_modal_popup() {
    let mut hwnd_raw = 0usize;
    mutate_state(|state| {
        hwnd_raw = state.modal_hwnd;
        state.modal_hwnd = 0;
        state.modal_dragging = false;
    });
    let hwnd = hwnd_raw as Hwnd;
    if !hwnd.is_null() {
        DestroyWindow(hwnd);
    }
}

pub(super) unsafe fn close_modal_popup() {
    mutate_state(|state| {
        state.modal_dialog = None;
        state.modal_dragging = false;
        if state.preview_path.is_some() || state.xml_path.is_some() {
            clear_preview_editor(state);
            state.status = "Preview/editor closed".to_owned();
        } else {
            state.status = "Dialog closed".to_owned();
        }
    });
    destroy_modal_popup();
}

pub(super) unsafe fn centered_popup_position(owner: Hwnd) -> (i32, i32) {
    let mut rect: Rect = zeroed();
    if GetWindowRect(owner, &mut rect) == 0 {
        return (CW_USEDEFAULT, CW_USEDEFAULT);
    }
    let owner_w = rect.right - rect.left;
    let owner_h = rect.bottom - rect.top;
    (
        rect.left + (owner_w - MODAL_WIDTH) / 2,
        rect.top + (owner_h - MODAL_HEIGHT) / 2,
    )
}
