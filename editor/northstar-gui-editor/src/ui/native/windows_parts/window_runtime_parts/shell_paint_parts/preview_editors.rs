use super::*;

pub(super) unsafe fn draw_asset_preview_editor(hdc: Hdc, rect: Rect, state: &UiState) {
    let mut y = rect.top + 44 - (state.inspector_scroll_rows as i32 * 24);
    draw_text(
        hdc,
        Rect {
            left: rect.left + 12,
            top: y,
            right: rect.right - 12,
            bottom: y + 22,
        },
        "Asset Preview Editor",
        rgb(20, 77, 138),
        true,
    );
    y += 30;
    draw_kv(hdc, rect, &mut y, "Name", &state.preview_name);
    draw_kv(hdc, rect, &mut y, "Type", &state.preview_kind);
    draw_kv(hdc, rect, &mut y, "Provider", &state.preview_provider);
    draw_kv(
        hdc,
        rect,
        &mut y,
        "File size",
        &format_size(state.preview_size),
    );
    if let Some(path) = &state.preview_path {
        draw_kv(hdc, rect, &mut y, "Path", path);
    }
    y += 18;
    draw_text(
        hdc,
        Rect {
            left: rect.left + 12,
            top: y,
            right: rect.right - 12,
            bottom: y + 22,
        },
        "Preview Surface",
        rgb(20, 77, 138),
        true,
    );
    y += 26;
    let surface = preview_surface_for_kind(
        &state.preview_kind,
        state.preview_path.as_deref().unwrap_or_default(),
    );
    draw_kv(hdc, rect, &mut y, "Surface", surface);
    draw_kv(
        hdc,
        rect,
        &mut y,
        "Edit mode",
        if state.preview_kind == "Text" {
            "text buffer"
        } else {
            "provider-backed preview"
        },
    );
    draw_kv(
        hdc,
        rect,
        &mut y,
        "Status",
        "Opened by double click / Enter",
    );
}

pub(super) unsafe fn draw_xml_editor(hdc: Hdc, rect: Rect, state: &UiState) {
    let theme = &state.editor_theme;
    let title = state.xml_path.as_deref().unwrap_or("XML Document");
    fill(
        hdc,
        Rect {
            left: rect.left + 1,
            top: rect.top + 33,
            right: rect.right - 1,
            bottom: rect.bottom - 1,
        },
        theme_color(theme.background),
    );
    fill(
        hdc,
        Rect {
            left: rect.left + 1,
            top: rect.top + 33,
            right: rect.right - 1,
            bottom: rect.top + 78,
        },
        theme_color(theme.active_line_background),
    );
    line_frame(
        hdc,
        Rect {
            left: rect.left + 1,
            top: rect.top + 77,
            right: rect.right - 1,
            bottom: rect.top + 78,
        },
        theme_color(theme.folding_line),
    );
    draw_text(
        hdc,
        Rect {
            left: rect.left + 12,
            top: rect.top + 42,
            right: rect.right - 12,
            bottom: rect.top + 62,
        },
        "XML Preview / Editor",
        theme_color(theme.reserved_word),
        true,
    );
    let dirty = if state.xml_dirty { " *modified" } else { "" };
    draw_text(
        hdc,
        Rect {
            left: rect.left + 12,
            top: rect.top + 62,
            right: rect.right - 12,
            bottom: rect.top + 76,
        },
        &format!("{}{} — {}", title, dirty, theme.name),
        theme_color(theme.comment),
        false,
    );

    let search_top = rect.top + 84;
    fill(
        hdc,
        Rect {
            left: rect.left + 12,
            top: search_top,
            right: rect.right - 12,
            bottom: search_top + 26,
        },
        if state.xml_search_focus {
            theme_color(theme.background)
        } else {
            theme_color(theme.active_line_background)
        },
    );
    line_frame(
        hdc,
        Rect {
            left: rect.left + 12,
            top: search_top,
            right: rect.right - 12,
            bottom: search_top + 26,
        },
        if state.xml_search_focus {
            theme_color(theme.selection_background)
        } else {
            theme_color(theme.folding_line)
        },
    );
    let search_label = if state.xml_search_query.is_empty() {
        "Search XML...".to_owned()
    } else {
        format!("Search: {}", state.xml_search_query)
    };
    draw_text(
        hdc,
        Rect {
            left: rect.left + 20,
            top: search_top + 5,
            right: rect.right - 20,
            bottom: search_top + 23,
        },
        &search_label,
        theme_color(theme.editor_foreground),
        false,
    );

    let gutter_left = rect.left + 8;
    let line_left = rect.left + 66;
    let first_line = state.inspector_scroll_rows;
    let mut y = rect.top + 122;
    let max_y = rect.bottom - 8;
    for (idx, line) in state.xml_lines.iter().enumerate().skip(first_line) {
        if y > max_y {
            break;
        }
        let line_no = idx + 1;
        if !state.xml_search_query.is_empty()
            && line
                .to_ascii_lowercase()
                .contains(&state.xml_search_query.to_ascii_lowercase())
        {
            fill(
                hdc,
                Rect {
                    left: rect.left + 1,
                    top: y - 2,
                    right: rect.right - 1,
                    bottom: y + 18,
                },
                theme_color(theme.search_background),
            );
        }
        if idx == state.xml_cursor_line {
            fill(
                hdc,
                Rect {
                    left: rect.left + 1,
                    top: y - 2,
                    right: rect.right - 1,
                    bottom: y + 18,
                },
                theme_color(theme.active_line_background),
            );
        }
        draw_text(
            hdc,
            Rect {
                left: gutter_left,
                top: y,
                right: rect.left + 48,
                bottom: y + 18,
            },
            &line_no.to_string(),
            theme_color(theme.line_numbers),
            false,
        );
        draw_text(
            hdc,
            Rect {
                left: rect.left + 50,
                top: y,
                right: rect.left + 64,
                bottom: y + 18,
            },
            if line.trim_start().starts_with("</") {
                "└"
            } else {
                "·"
            },
            theme_color(theme.folding_line),
            false,
        );
        draw_editor_line_tokens(hdc, line_left, y, rect.right - 10, state, idx, line, theme);
        y += 20;
    }
    if state.xml_lines.is_empty() {
        draw_text(
            hdc,
            Rect {
                left: line_left,
                top: y,
                right: rect.right - 12,
                bottom: y + 22,
            },
            "Empty XML document",
            theme_color(theme.comment),
            false,
        );
    }
}

pub(super) unsafe fn draw_editor_line_tokens(
    hdc: Hdc,
    x: i32,
    y: i32,
    right: i32,
    state: &UiState,
    line_index: usize,
    line: &str,
    theme: &EditorColorDictionary,
) {
    if let Some(document) = state.active_document.as_ref() {
        if !state.cached_spans.is_empty() {
            draw_highlighted_document_line_tokens(
                hdc,
                x,
                y,
                right,
                document.buffer.as_str(),
                &state.cached_spans,
                line_index,
                theme,
            );
            return;
        }
    }
    draw_legacy_line_tokens(hdc, x, y, right, line, theme);
}
