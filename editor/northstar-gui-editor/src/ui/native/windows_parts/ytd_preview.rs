use super::*;

pub(super) fn is_ytd_preview_state(state: &UiState) -> bool {
    let path = state
        .preview_path
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    path.ends_with(".ytd")
        || state.preview_kind.eq_ignore_ascii_case(".ytd")
        || state.preview_provider.to_ascii_lowercase().contains("ytd")
}

pub(super) unsafe fn draw_modal_ytd_content(hdc: Hdc, main_rect: Rect, state: &UiState) {
    fill(hdc, main_rect, rgb(232, 238, 244));
    line_frame(hdc, main_rect, rgb(185, 195, 205));

    let list = Rect {
        left: main_rect.left,
        top: main_rect.top,
        right: main_rect.left + 248,
        bottom: main_rect.bottom,
    };
    let viewport = Rect {
        left: list.right + 1,
        top: main_rect.top,
        right: main_rect.right,
        bottom: main_rect.bottom,
    };
    fill(hdc, list, rgb(235, 238, 240));
    line_frame(hdc, list, rgb(190, 198, 206));
    fill(
        hdc,
        Rect {
            left: list.left + 8,
            top: list.top + 8,
            right: list.right - 8,
            bottom: list.top + 38,
        },
        rgb(255, 255, 255),
    );
    line_frame(
        hdc,
        Rect {
            left: list.left + 8,
            top: list.top + 8,
            right: list.right - 8,
            bottom: list.top + 38,
        },
        rgb(152, 163, 175),
    );
    draw_text(
        hdc,
        Rect {
            left: list.left + 18,
            top: list.top + 15,
            right: list.right - 36,
            bottom: list.top + 32,
        },
        "Search texture",
        rgb(100, 116, 139),
        false,
    );
    draw_text(
        hdc,
        Rect {
            left: list.right - 30,
            top: list.top + 14,
            right: list.right - 12,
            bottom: list.top + 32,
        },
        "⌕",
        rgb(75, 85, 99),
        true,
    );

    let entries = ytd_texture_entries(state);
    let mut y = list.top + 46;
    for (index, entry) in entries.iter().enumerate().skip(state.inspector_scroll_rows) {
        if y + 36 > list.bottom - 38 {
            break;
        }
        let selected = index == 0;
        if selected {
            fill(
                hdc,
                Rect {
                    left: list.left + 1,
                    top: y - 2,
                    right: list.right - 1,
                    bottom: y + 34,
                },
                rgb(222, 230, 238),
            );
        }
        draw_text(
            hdc,
            Rect {
                left: list.left + 10,
                top: y,
                right: list.right - 12,
                bottom: y + 18,
            },
            &entry.name,
            if selected {
                rgb(0, 145, 170)
            } else {
                rgb(31, 41, 55)
            },
            true,
        );
        draw_text(
            hdc,
            Rect {
                left: list.left + 10,
                top: y + 18,
                right: list.right - 12,
                bottom: y + 34,
            },
            &entry.details,
            rgb(75, 85, 99),
            false,
        );
        y += 38;
    }
    let count_text = entries.len().to_string();
    draw_text(
        hdc,
        Rect {
            left: list.right - 94,
            top: list.bottom - 72,
            right: list.right - 18,
            bottom: list.bottom - 20,
        },
        &count_text,
        rgb(160, 166, 172),
        false,
    );

    fill(hdc, viewport, rgb(54, 64, 74));
    draw_text(
        hdc,
        Rect {
            left: viewport.left + 14,
            top: viewport.top + 12,
            right: viewport.right - 14,
            bottom: viewport.top + 36,
        },
        entries
            .first()
            .map(|e| e.name.as_str())
            .unwrap_or("Texture"),
        rgb(15, 23, 42),
        true,
    );
    let first_details = entries
        .first()
        .map(|e| e.details.as_str())
        .unwrap_or("No texture entries returned by provider");
    draw_text(
        hdc,
        Rect {
            left: viewport.left + 14,
            top: viewport.top + 36,
            right: viewport.right - 14,
            bottom: viewport.top + 58,
        },
        first_details,
        rgb(31, 41, 55),
        true,
    );

    let image = Rect {
        left: viewport.left,
        top: viewport.top + 64,
        right: viewport.right,
        bottom: viewport.bottom,
    };
    fill(hdc, image, rgb(50, 61, 72));
    let cx = (image.left + image.right) / 2;
    let cy = (image.top + image.bottom) / 2;
    let icon = Rect {
        left: cx - 92,
        top: cy - 64,
        right: cx + 92,
        bottom: cy + 64,
    };
    fill(hdc, icon, rgb(70, 75, 82));
    fill(
        hdc,
        Rect {
            left: icon.left + 24,
            top: icon.top - 22,
            right: icon.left + 92,
            bottom: icon.top + 44,
        },
        rgb(70, 75, 82),
    );
    fill(
        hdc,
        Rect {
            left: icon.right - 92,
            top: icon.top - 22,
            right: icon.right - 24,
            bottom: icon.top + 44,
        },
        rgb(70, 75, 82),
    );
    draw_text(
        hdc,
        Rect {
            left: image.left + 18,
            top: image.bottom - 32,
            right: image.right - 18,
            bottom: image.bottom - 12,
        },
        "Texture pixels require provider thumbnail/decode output",
        rgb(210, 216, 222),
        false,
    );
}

pub(super) fn ytd_texture_entries(state: &UiState) -> Vec<YtdTextureEntry> {
    parse_ytd_inspect_entries(&state.preview_lines)
}

pub(super) unsafe fn draw_modal_xml_content(hdc: Hdc, rect: Rect, state: &UiState) {
    let theme = &state.editor_theme;
    let scrollbar = modal_editor_scrollbar_rect(rect);
    let text_right = scrollbar.left - 8;
    fill(hdc, rect, sublime_background(theme));
    line_frame(hdc, rect, sublime_border(theme));
    fill(
        hdc,
        Rect {
            left: rect.left + 1,
            top: rect.top + 1,
            right: rect.right - 1,
            bottom: rect.top + 36,
        },
        sublime_header_background(theme),
    );
    draw_text(
        hdc,
        Rect {
            left: rect.left + 12,
            top: rect.top + 9,
            right: text_right,
            bottom: rect.top + 30,
        },
        if state
            .preview_content_kind
            .as_deref()
            .unwrap_or("")
            .contains("xml")
        {
            "XML content"
        } else {
            "Text Editor"
        },
        theme_color(theme.reserved_word),
        true,
    );
    let gutter_left = rect.left + 8;
    let line_left = rect.left + 66;
    fill(
        hdc,
        Rect {
            left: rect.left + 1,
            top: rect.top + 36,
            right: line_left - 8,
            bottom: rect.bottom - 1,
        },
        sublime_gutter_background(theme),
    );
    line_frame(
        hdc,
        Rect {
            left: line_left - 8,
            top: rect.top + 36,
            right: line_left - 7,
            bottom: rect.bottom - 1,
        },
        sublime_border(theme),
    );
    let mut y = rect.top + 46;
    let max_y = rect.bottom - 8;
    for (idx, line) in state
        .xml_lines
        .iter()
        .enumerate()
        .skip(state.inspector_scroll_rows)
    {
        if y > max_y {
            break;
        }
        if idx == state.xml_cursor_line {
            fill(
                hdc,
                Rect {
                    left: line_left - 7,
                    top: y - 2,
                    right: text_right,
                    bottom: y + 18,
                },
                sublime_current_line(theme),
            );
        }
        draw_editor_line_selection_background(hdc, line_left, y, text_right, state, idx, theme);
        draw_text(
            hdc,
            Rect {
                left: gutter_left,
                top: y,
                right: rect.left + 52,
                bottom: y + 18,
            },
            &(idx + 1).to_string(),
            theme_color(theme.line_numbers),
            false,
        );
        draw_editor_line_tokens(hdc, line_left, y, text_right, state, idx, line, theme);
        draw_editor_cursor(hdc, line_left, y, text_right, state, idx, theme);
        y += 20;
    }
    if state.xml_lines.is_empty() {
        draw_text(
            hdc,
            Rect {
                left: line_left,
                top: y,
                right: text_right,
                bottom: y + 22,
            },
            "Empty XML document",
            theme_color(theme.comment),
            false,
        );
    }
    draw_modal_editor_scrollbar(
        hdc,
        rect,
        state.xml_lines.len(),
        state.inspector_scroll_rows,
    );
}

pub(super) unsafe fn draw_modal_text_content(hdc: Hdc, rect: Rect, state: &UiState) {
    let scrollbar = modal_editor_scrollbar_rect(rect);
    let text_right = scrollbar.left - 8;
    fill(hdc, rect, rgb(255, 255, 255));
    line_frame(hdc, rect, rgb(203, 213, 225));
    fill(
        hdc,
        Rect {
            left: rect.left + 1,
            top: rect.top + 1,
            right: rect.right - 1,
            bottom: rect.top + 36,
        },
        rgb(248, 250, 252),
    );
    draw_text(
        hdc,
        Rect {
            left: rect.left + 12,
            top: rect.top + 9,
            right: text_right,
            bottom: rect.top + 30,
        },
        "File content",
        rgb(20, 77, 138),
        true,
    );
    let gutter_right = rect.left + 58;
    fill(
        hdc,
        Rect {
            left: rect.left + 1,
            top: rect.top + 36,
            right: gutter_right,
            bottom: rect.bottom - 1,
        },
        rgb(248, 250, 252),
    );
    line_frame(
        hdc,
        Rect {
            left: gutter_right,
            top: rect.top + 36,
            right: gutter_right + 1,
            bottom: rect.bottom - 1,
        },
        rgb(226, 232, 240),
    );
    let mut y = rect.top + 46;
    let max_y = rect.bottom - 8;
    for (idx, line) in state
        .preview_lines
        .iter()
        .enumerate()
        .skip(state.inspector_scroll_rows)
    {
        if y > max_y {
            break;
        }
        draw_text(
            hdc,
            Rect {
                left: rect.left + 8,
                top: y,
                right: gutter_right - 6,
                bottom: y + 18,
            },
            &(idx + 1).to_string(),
            rgb(100, 116, 139),
            false,
        );
        draw_text(
            hdc,
            Rect {
                left: gutter_right + 12,
                top: y,
                right: text_right,
                bottom: y + 18,
            },
            line,
            rgb(17, 24, 39),
            false,
        );
        y += 20;
    }
    if state.preview_lines.is_empty() {
        draw_text(
            hdc,
            Rect {
                left: gutter_right + 12,
                top: y,
                right: text_right,
                bottom: y + 22,
            },
            "Empty file",
            rgb(100, 116, 139),
            false,
        );
    }
    draw_modal_editor_scrollbar(
        hdc,
        rect,
        state.preview_lines.len(),
        state.inspector_scroll_rows,
    );
}

pub(super) unsafe fn draw_modal_empty_preview(hdc: Hdc, rect: Rect, state: &UiState) {
    fill(hdc, rect, rgb(248, 250, 252));
    line_frame(hdc, rect, rgb(203, 213, 225));
    draw_text(
        hdc,
        Rect {
            left: rect.left + 18,
            top: rect.top + 18,
            right: rect.right - 18,
            bottom: rect.top + 42,
        },
        "Provider-backed preview",
        rgb(20, 77, 138),
        true,
    );
    let message = format!(
        "The asset route is registered, but this tool has not exposed an inline content preview yet.\n\nProvider: {}\nType: {}",
        state.preview_provider,
        state.preview_kind,
    );
    draw_wrapped_dialog_text(
        hdc,
        Rect {
            left: rect.left + 18,
            top: rect.top + 58,
            right: rect.right - 18,
            bottom: rect.bottom - 18,
        },
        &message,
    );
}

pub(super) unsafe fn draw_modal_editor_scrollbar(
    hdc: Hdc,
    editor_rect: Rect,
    total_lines: usize,
    first_line: usize,
) {
    let rect = modal_editor_scrollbar_rect(editor_rect);
    fill(hdc, rect, rgb(241, 245, 249));
    line_frame(hdc, rect, rgb(203, 213, 225));
    let visible_lines = modal_visible_editor_lines(editor_rect);
    if total_lines <= visible_lines || total_lines == 0 {
        fill(
            hdc,
            Rect {
                left: rect.left + 3,
                top: rect.top + 3,
                right: rect.right - 3,
                bottom: rect.bottom - 3,
            },
            rgb(203, 213, 225),
        );
        return;
    }
    let track_h = (rect.bottom - rect.top - 6).max(24);
    let thumb_h = ((track_h as f32 * visible_lines as f32 / total_lines as f32) as i32)
        .max(26)
        .min(track_h);
    let max_scroll = total_lines.saturating_sub(visible_lines).max(1);
    let scroll_ratio = first_line.min(max_scroll) as f32 / max_scroll as f32;
    let thumb_top = rect.top + 3 + ((track_h - thumb_h) as f32 * scroll_ratio) as i32;
    fill(
        hdc,
        Rect {
            left: rect.left + 3,
            top: thumb_top,
            right: rect.right - 3,
            bottom: thumb_top + thumb_h,
        },
        rgb(100, 116, 139),
    );
}
