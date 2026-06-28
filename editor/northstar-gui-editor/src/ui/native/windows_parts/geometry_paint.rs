use super::*;

pub(super) fn modal_popup_primary_rect(client: Rect) -> Rect {
    Rect {
        left: client.right - 128,
        top: client.bottom - 54,
        right: client.right - 24,
        bottom: client.bottom - 22,
    }
}

pub(super) unsafe fn draw_wrapped_dialog_text(hdc: Hdc, rect: Rect, text: &str) {
    let mut y = rect.top;
    let mut line = String::new();
    let max_chars = ((rect.right - rect.left).max(80) / 8) as usize;
    for word in text.split_whitespace() {
        if line.len() + word.len() + 1 > max_chars && !line.is_empty() {
            draw_text(
                hdc,
                Rect {
                    left: rect.left,
                    top: y,
                    right: rect.right,
                    bottom: y + 22,
                },
                &line,
                rgb(31, 41, 55),
                false,
            );
            y += 24;
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        draw_text(
            hdc,
            Rect {
                left: rect.left,
                top: y,
                right: rect.right,
                bottom: y + 22,
            },
            &line,
            rgb(31, 41, 55),
            false,
        );
    }
}

pub(super) unsafe fn draw_menu_dropdown(hdc: Hdc, state: &UiState) {
    let Some(menu) = state.menu_open else {
        return;
    };
    let Some(rect) = menu_dropdown_rect(menu) else {
        return;
    };
    fill(hdc, rect, rgb(255, 255, 255));
    line_frame(hdc, rect, rgb(156, 163, 175));
    fill(
        hdc,
        Rect {
            left: rect.left + 1,
            top: rect.top + 1,
            right: rect.left + 28,
            bottom: rect.bottom - 1,
        },
        rgb(244, 246, 248),
    );
    let mut y = rect.top + 4;
    for item in menu_model::dropdown_items(menu) {
        if menu_model::is_separator(item) {
            line_frame(
                hdc,
                Rect {
                    left: rect.left + 32,
                    top: y + 3,
                    right: rect.right - 6,
                    bottom: y + 4,
                },
                rgb(226, 232, 240),
            );
            y += 8;
            continue;
        }

        let hovered = state.menu_item_hover == Some(*item);
        if hovered {
            fill(
                hdc,
                Rect {
                    left: rect.left + 1,
                    top: y,
                    right: rect.right - 1,
                    bottom: y + 24,
                },
                rgb(219, 234, 254),
            );
        }

        if state.view_mode == *item {
            fill(
                hdc,
                Rect {
                    left: rect.left + 7,
                    top: y + 7,
                    right: rect.left + 13,
                    bottom: y + 13,
                },
                rgb(37, 99, 235),
            );
        }

        let clean_item = menu_model::clean_item_label(item);
        draw_text(
            hdc,
            Rect {
                left: rect.left + 34,
                top: y + 5,
                right: rect.right - 24,
                bottom: y + 22,
            },
            clean_item,
            if hovered {
                rgb(30, 64, 175)
            } else {
                rgb(31, 41, 55)
            },
            hovered,
        );
        if menu_model::is_submenu(item) {
            draw_text(
                hdc,
                Rect {
                    left: rect.right - 18,
                    top: y + 5,
                    right: rect.right - 6,
                    bottom: y + 22,
                },
                "›",
                rgb(31, 41, 55),
                false,
            );
        }
        y += 24;
    }
}

pub(super) fn hit_toolbar(x: i32, y: i32) -> Option<&'static str> {
    if !(42..=68).contains(&y) {
        return None;
    }
    for button in toolbar_model::buttons() {
        let width = button_width(button.label);
        if x >= button.x && x <= button.x + width {
            return Some(button.label);
        }
    }
    None
}

pub(super) fn toolbar_filter_rect(client: Rect) -> Rect {
    let search_box = toolbar_model::search_box(client.right);
    Rect {
        left: search_box.left,
        top: search_box.top,
        right: search_box.right,
        bottom: search_box.bottom,
    }
}

pub(super) unsafe fn apply_ui_update(hwnd: Hwnd, request: UiUpdateRequest) {
    match request {
        UiUpdateRequest::None => {}
        UiUpdateRequest::Region(rect) => invalidate_rect(hwnd, rect),
        UiUpdateRequest::Regions(regions) => {
            for rect in regions {
                invalidate_rect(hwnd, rect);
            }
        }
        UiUpdateRequest::Layout | UiUpdateRequest::Full => {
            InvalidateRect(hwnd, null(), 0);
        }
    }
}

pub(super) unsafe fn invalidate_rect(hwnd: Hwnd, rect: Rect) {
    let rect = clamp_rect(rect);
    if rect.right > rect.left && rect.bottom > rect.top {
        InvalidateRect(hwnd, &rect, 0);
    }
}

pub(super) fn clamp_rect(rect: Rect) -> Rect {
    Rect {
        left: rect.left.max(0),
        top: rect.top.max(0),
        right: rect.right.max(rect.left),
        bottom: rect.bottom.max(rect.top),
    }
}

pub(super) fn menu_bar_rect(client: Rect) -> Rect {
    Rect {
        left: 0,
        top: 0,
        right: client.right,
        bottom: 32,
    }
}

pub(super) fn expand_rect(rect: Rect, amount: i32) -> Rect {
    Rect {
        left: rect.left - amount,
        top: rect.top - amount,
        right: rect.right + amount,
        bottom: rect.bottom + amount,
    }
}

pub(super) fn menu_repaint_rect(client: Rect) -> Rect {
    if let Some(state) = cloned_state() {
        if let Some(menu) = state.menu_open {
            if let Some(dropdown) = menu_dropdown_rect(menu) {
                return union_rect(menu_bar_rect(client), dropdown);
            }
        }
    }
    Rect {
        left: 0,
        top: 0,
        right: client.right,
        bottom: 32,
    }
}

pub(super) fn toolbar_repaint_rect(client: Rect) -> Rect {
    Rect {
        left: 0,
        top: 32,
        right: client.right,
        bottom: 78,
    }
}

pub(super) fn file_row_rect(rect: Rect, row: usize) -> Rect {
    let local = row.saturating_sub(cloned_state().map(|state| state.scroll_rows).unwrap_or(0));
    let y = row_start(rect) + local as i32 * row_height();
    Rect {
        left: rect.left + 1,
        top: y - 1,
        right: rect.right - 1,
        bottom: y + row_height() + 1,
    }
}

pub(super) fn union_rect(a: Rect, b: Rect) -> Rect {
    Rect {
        left: a.left.min(b.left),
        top: a.top.min(b.top),
        right: a.right.max(b.right),
        bottom: a.bottom.max(b.bottom),
    }
}

pub(super) fn hit_file_row(rect: Rect, x: i32, y: i32, scroll: usize, len: usize) -> Option<usize> {
    if !point_in(rect, x, y) || y < row_start(rect) {
        return None;
    }
    let local = ((y - row_start(rect)) / row_height()) as usize;
    let row = scroll + local;
    (row < len).then_some(row)
}

pub(super) fn row_start(rect: Rect) -> i32 {
    rect.top + 66
}
pub(super) fn row_height() -> i32 {
    25
}
pub(super) fn point_in(rect: Rect, x: i32, y: i32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}
pub(super) fn button_width(label: &str) -> i32 {
    toolbar_model::button_width(label)
}

pub(super) fn reset_caret_blink(state: &mut UiState) {
    state.caret_visible = true;
}

pub(super) fn toggle_caret_blink() -> bool {
    let mut should_repaint = false;
    mutate_state(|state| {
        if state.xml_path.is_some() && !state.xml_search_focus {
            state.caret_visible = !state.caret_visible;
            should_repaint = true;
        }
    });
    should_repaint
}

pub(super) fn mutate_state(f: impl FnOnce(&mut UiState)) {
    if let Some(mutex) = UI_STATE.get() {
        if let Ok(mut state) = mutex.lock() {
            f(&mut state);
        }
    }
}

pub(super) fn cloned_state() -> Option<UiState> {
    UI_STATE
        .get()
        .and_then(|mutex| mutex.lock().ok().map(|state| state.clone()))
}

pub(super) unsafe fn client_rect(hwnd: Hwnd) -> Rect {
    let mut rect: Rect = zeroed();
    GetClientRect(hwnd, &mut rect);
    rect
}

pub(super) unsafe fn draw_toolbar_button(
    hdc: Hdc,
    x: i32,
    y: i32,
    button: &toolbar_model::ToolbarButton,
    hovered: bool,
    active: bool,
) {
    let width = button_width(button.label);
    let rect = Rect {
        left: x,
        top: y,
        right: x + width,
        bottom: y + 26,
    };
    let fill_color = if active {
        rgb(219, 234, 254)
    } else if hovered {
        rgb(255, 255, 255)
    } else {
        rgb(248, 250, 252)
    };
    let border_color = if active {
        rgb(37, 99, 235)
    } else if hovered {
        rgb(96, 165, 250)
    } else {
        rgb(185, 195, 207)
    };
    fill(hdc, rect, fill_color);
    line_frame(hdc, rect, border_color);
    if hovered || active {
        line_frame(
            hdc,
            Rect {
                left: rect.left + 1,
                top: rect.top + 1,
                right: rect.right - 1,
                bottom: rect.bottom - 1,
            },
            rgb(255, 255, 255),
        );
    }
    draw_text(
        hdc,
        Rect {
            left: x + 5,
            top: y + 6,
            right: x + width - 4,
            bottom: y + 23,
        },
        button.icon,
        if active {
            rgb(30, 64, 175)
        } else {
            rgb(35, 48, 64)
        },
        active,
    );
    if hovered {
        draw_text(
            hdc,
            Rect {
                left: x,
                top: y + 30,
                right: x + 180,
                bottom: y + 48,
            },
            button.hint,
            rgb(55, 65, 81),
            false,
        );
    }
}

pub(super) unsafe fn fill(hdc: Hdc, rect: Rect, color: Dword) {
    let brush = CreateSolidBrush(color);
    FillRect(hdc, &rect, brush);
    DeleteObject(brush as Hgdiobj);
}

pub(super) unsafe fn line_frame(hdc: Hdc, rect: Rect, color: Dword) {
    let brush = CreateSolidBrush(color);
    FrameRect(hdc, &rect, brush);
    DeleteObject(brush as Hgdiobj);
}

pub(super) unsafe fn draw_text(hdc: Hdc, mut rect: Rect, text: &str, color: Dword, bold: bool) {
    SetBkMode(hdc, TRANSPARENT);
    SetTextColor(hdc, color);
    let font = create_font("Segoe UI", 15, if bold { FW_SEMIBOLD } else { FW_NORMAL });
    let old = SelectObject(hdc, font as Hgdiobj);
    let wide = to_wide(text);
    DrawTextW(
        hdc,
        wide.as_ptr(),
        wide.len().saturating_sub(1) as i32,
        &mut rect,
        DT_LEFT | DT_TOP | DT_SINGLELINE | DT_END_ELLIPSIS | DT_VCENTER,
    );
    SelectObject(hdc, old);
    DeleteObject(font as Hgdiobj);
}

pub(super) unsafe fn create_font(face: &str, height: i32, weight: i32) -> Hfont {
    let face = to_wide(face);
    CreateFontW(
        -height,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        1,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        DEFAULT_QUALITY,
        DEFAULT_PITCH | FF_DONTCARE,
        face.as_ptr(),
    )
}

pub(super) fn lparam_x(lparam: Lparam) -> i32 {
    (lparam as u32 & 0xffff) as u16 as i16 as i32
}
pub(super) fn lparam_y(lparam: Lparam) -> i32 {
    ((lparam as u32 >> 16) & 0xffff) as u16 as i16 as i32
}
pub(super) fn wheel_delta(wparam: Wparam) -> i32 {
    ((wparam as u32 >> 16) & 0xffff) as u16 as i16 as i32
}
pub(super) fn theme_color(value: &str) -> Dword {
    if let Some(hex) = value.strip_prefix("$00") {
        return u32::from_str_radix(hex, 16).unwrap_or_else(|_| rgb(0, 0, 0));
    }
    if let Some(hex) = value.strip_prefix("00") {
        return u32::from_str_radix(hex, 16).unwrap_or_else(|_| rgb(0, 0, 0));
    }
    match value {
        "clNone" => rgb(0, 0, 0),
        "clBlack" => rgb(0, 0, 0),
        "clWhite" => rgb(255, 255, 255),
        "clRed" => rgb(255, 0, 0),
        "clGreen" => rgb(0, 128, 0),
        "clBlue" => rgb(0, 0, 255),
        "clYellow" => rgb(255, 255, 0),
        "clAqua" => rgb(0, 255, 255),
        "clFuchsia" => rgb(255, 0, 255),
        "clPurple" => rgb(128, 0, 128),
        "clNavy" => rgb(0, 0, 128),
        "clTeal" => rgb(0, 128, 128),
        "clOlive" => rgb(128, 128, 0),
        "clMaroon" => rgb(128, 0, 0),
        "clGray" => rgb(128, 128, 128),
        "clSilver" => rgb(192, 192, 192),
        "clLime" => rgb(0, 255, 0),
        "clHighlight" => rgb(0, 120, 215),
        "clHighlightText" => rgb(255, 255, 255),
        _ => rgb(0, 0, 0),
    }
}

pub(super) fn rgb(r: u8, g: u8, b: u8) -> Dword {
    (r as Dword) | ((g as Dword) << 8) | ((b as Dword) << 16)
}
pub(super) fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
