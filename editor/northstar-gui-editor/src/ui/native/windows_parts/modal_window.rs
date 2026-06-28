use super::*;

pub(super) unsafe extern "system" fn modal_window_proc(
    hwnd: Hwnd,
    msg: Uint,
    w_param: Wparam,
    l_param: Lparam,
) -> Lresult {
    match msg {
        WM_PAINT => {
            paint_modal(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        WM_LBUTTONDOWN => {
            SetFocus(hwnd);
            modal_handle_click(hwnd, lparam_x(l_param), lparam_y(l_param));
            0
        }
        WM_LBUTTONUP => {
            modal_handle_mouse_up(hwnd);
            0
        }
        WM_MOUSEMOVE => {
            modal_handle_mouse_move(hwnd, lparam_x(l_param), lparam_y(l_param));
            0
        }
        WM_MOUSEWHEEL => {
            modal_mouse_wheel(hwnd, wheel_delta(w_param));
            0
        }
        WM_KEYDOWN => {
            if w_param == VK_ESCAPE {
                close_modal_popup();
                0
            } else {
                modal_handle_key(hwnd, w_param);
                0
            }
        }
        WM_CHAR => {
            modal_handle_char(hwnd, w_param);
            0
        }
        WM_CLOSE => {
            close_modal_popup();
            0
        }
        _ => DefWindowProcW(hwnd, msg, w_param, l_param),
    }
}

pub(super) unsafe fn modal_handle_key(hwnd: Hwnd, key: Wparam) {
    mutate_state(|state| {
        state.status = format!(
            "WM_KEYDOWN modal: key=0x{key:X} xml_path={}",
            state.xml_path.is_some()
        );
        if ctrl_key_down() && handle_editor_hotkey(state, key) {
            return;
        }
        if state.xml_path.is_none() {
            return;
        }
        let _ = handle_xml_key(state, key);
        state.preview_lines = state.xml_lines.clone();
    });
    apply_ui_update(hwnd, UiUpdateRequest::Full);
}

pub(super) unsafe fn modal_handle_char(hwnd: Hwnd, ch: Wparam) {
    mutate_state(|state| {
        let Some(ch) = char::from_u32(ch as u32) else {
            return;
        };
        state.status = format!(
            "WM_CHAR modal: ch={:?} xml_path={} xml_search_focus={}",
            ch,
            state.xml_path.is_some(),
            state.xml_search_focus
        );
        if ch == '\r' || ch == '\n' || ch == '\u{8}' || ch == '\u{1b}' {
            return;
        }
        if state.xml_path.is_none() {
            return;
        }
        if state.xml_search_focus {
            if !ch.is_control() {
                state.xml_search_query.push(ch);
                state.status = format!("Text search: {}", state.xml_search_query);
            }
            return;
        }
        if !ch.is_control() {
            insert_char_into_active_document_or_xml_cache(state, ch);
            state.status = "Text buffer edited".to_owned();
        }
    });
    apply_ui_update(hwnd, UiUpdateRequest::Full);
}

pub(super) unsafe fn paint_modal(hwnd: Hwnd) {
    let mut ps: PaintStruct = zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let client = client_rect(hwnd);
    let width = (client.right - client.left).max(1);
    let height = (client.bottom - client.top).max(1);
    let paint_w = (ps.rc_paint.right - ps.rc_paint.left).max(1);
    let paint_h = (ps.rc_paint.bottom - ps.rc_paint.top).max(1);

    let mem_dc = CreateCompatibleDC(hdc);
    if mem_dc.is_null() {
        IntersectClipRect(
            hdc,
            ps.rc_paint.left,
            ps.rc_paint.top,
            ps.rc_paint.right,
            ps.rc_paint.bottom,
        );
        draw_modal_surface(hdc, client);
        EndPaint(hwnd, &ps);
        return;
    }

    let bitmap = CreateCompatibleBitmap(hdc, width, height);
    if bitmap.is_null() {
        DeleteDC(mem_dc);
        IntersectClipRect(
            hdc,
            ps.rc_paint.left,
            ps.rc_paint.top,
            ps.rc_paint.right,
            ps.rc_paint.bottom,
        );
        draw_modal_surface(hdc, client);
        EndPaint(hwnd, &ps);
        return;
    }

    let old = SelectObject(mem_dc, bitmap);
    IntersectClipRect(
        mem_dc,
        ps.rc_paint.left,
        ps.rc_paint.top,
        ps.rc_paint.right,
        ps.rc_paint.bottom,
    );
    draw_modal_surface(mem_dc, client);
    BitBlt(
        hdc,
        ps.rc_paint.left,
        ps.rc_paint.top,
        paint_w,
        paint_h,
        mem_dc,
        ps.rc_paint.left,
        ps.rc_paint.top,
        SRCCOPY,
    );
    SelectObject(mem_dc, old);
    DeleteObject(bitmap);
    DeleteDC(mem_dc);
    EndPaint(hwnd, &ps);
}

pub(super) unsafe fn draw_modal_surface(hdc: Hdc, client: Rect) {
    fill(hdc, client, rgb(255, 255, 255));
    line_frame(
        hdc,
        Rect {
            left: 0,
            top: 0,
            right: client.right,
            bottom: client.bottom,
        },
        rgb(226, 232, 240),
    );

    if let Some(state) = cloned_state() {
        if state.preview_path.is_some() || state.xml_path.is_some() {
            draw_modal_preview_editor(hdc, client, &state);
        } else if let Some(dialog) = state.modal_dialog {
            draw_text(
                hdc,
                Rect {
                    left: 24,
                    top: 22,
                    right: client.right - 24,
                    bottom: 48,
                },
                &dialog.title,
                rgb(17, 24, 39),
                true,
            );
            line_frame(
                hdc,
                Rect {
                    left: 24,
                    top: 56,
                    right: client.right - 24,
                    bottom: 57,
                },
                rgb(229, 231, 235),
            );
            draw_wrapped_dialog_text(
                hdc,
                Rect {
                    left: 24,
                    top: 76,
                    right: client.right - 24,
                    bottom: client.bottom - 72,
                },
                &dialog.message,
            );
            let ok = modal_popup_primary_rect(client);
            fill(hdc, ok, rgb(239, 246, 255));
            line_frame(hdc, ok, rgb(37, 99, 235));
            draw_text(
                hdc,
                Rect {
                    left: ok.left + 28,
                    top: ok.top + 8,
                    right: ok.right - 18,
                    bottom: ok.bottom - 5,
                },
                &dialog.primary_action,
                rgb(30, 64, 175),
                true,
            );
        }
    }
}

pub(super) unsafe fn draw_modal_preview_editor(hdc: Hdc, client: Rect, state: &UiState) {
    fill(
        hdc,
        Rect {
            left: 0,
            top: 0,
            right: client.right,
            bottom: 52,
        },
        rgb(248, 250, 252),
    );
    draw_text(
        hdc,
        Rect {
            left: 24,
            top: 16,
            right: client.right - 260,
            bottom: 42,
        },
        &state.preview_name,
        rgb(15, 23, 42),
        true,
    );
    draw_text(
        hdc,
        Rect {
            left: client.right - 246,
            top: 18,
            right: client.right - 24,
            bottom: 40,
        },
        preview_surface_for_kind(
            &state.preview_kind,
            state.preview_path.as_deref().unwrap_or_default(),
        ),
        rgb(37, 99, 235),
        true,
    );
    line_frame(
        hdc,
        Rect {
            left: 0,
            top: 51,
            right: client.right,
            bottom: 52,
        },
        rgb(226, 232, 240),
    );

    let geometry = EditorGeometry::from_modal_client(client);
    let content_rect = geometry.content;
    let metadata = geometry.metadata;
    let editor = geometry.editor;

    if is_ytd_preview_state(state) {
        draw_modal_ytd_content(hdc, content_rect, state);
    } else {
        if state.xml_path.is_some() {
            draw_modal_xml_content(hdc, editor, state);
        } else if !state.preview_lines.is_empty() {
            draw_modal_text_content(hdc, editor, state);
        } else {
            draw_modal_empty_preview(hdc, editor, state);
        }
        draw_modal_metadata_panel(hdc, metadata, state);
    }

    let ok = modal_popup_primary_rect(client);
    fill(hdc, ok, rgb(239, 246, 255));
    line_frame(hdc, ok, rgb(37, 99, 235));
    draw_text(
        hdc,
        Rect {
            left: ok.left + 26,
            top: ok.top + 8,
            right: ok.right - 18,
            bottom: ok.bottom - 5,
        },
        "Close",
        rgb(30, 64, 175),
        true,
    );
}

pub(super) unsafe fn draw_modal_metadata_panel(hdc: Hdc, rect: Rect, state: &UiState) {
    fill(hdc, rect, rgb(248, 250, 252));
    line_frame(hdc, rect, rgb(226, 232, 240));
    fill(
        hdc,
        Rect {
            left: rect.left + 1,
            top: rect.top + 1,
            right: rect.right - 1,
            bottom: rect.top + 38,
        },
        rgb(241, 245, 249),
    );
    draw_text(
        hdc,
        Rect {
            left: rect.left + 12,
            top: rect.top + 10,
            right: rect.right - 12,
            bottom: rect.top + 30,
        },
        "Asset metadata",
        rgb(20, 77, 138),
        true,
    );
    let mut y = rect.top + 52;
    draw_kv(hdc, rect, &mut y, "Type", &state.preview_kind);
    draw_kv(hdc, rect, &mut y, "Provider", &state.preview_provider);
    draw_kv(hdc, rect, &mut y, "Size", &format_size(state.preview_size));
    draw_kv(
        hdc,
        rect,
        &mut y,
        "Lines",
        &modal_preview_line_count(state).to_string(),
    );
    draw_kv(
        hdc,
        rect,
        &mut y,
        "Scroll",
        &state.inspector_scroll_rows.to_string(),
    );
    y += 12;
    draw_text(
        hdc,
        Rect {
            left: rect.left + 12,
            top: y,
            right: rect.right - 12,
            bottom: y + 18,
        },
        "Path",
        rgb(20, 77, 138),
        true,
    );
    y += 22;
    if let Some(path) = &state.preview_path {
        draw_wrapped_dialog_text(
            hdc,
            Rect {
                left: rect.left + 12,
                top: y,
                right: rect.right - 12,
                bottom: rect.bottom - 16,
            },
            path,
        );
    }
}
