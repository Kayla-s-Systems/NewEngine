use super::*;

pub(super) fn insert_char_into_active_document_or_xml_cache(state: &mut UiState, ch: char) {
    ensure_xml_line(state);
    if let Some(document) = state.active_document.as_mut() {
        document.insert_text(&ch.to_string());
        sync_text_document_edit_cache(state, "Text buffer edited");
        return;
    }

    let line_index = state.xml_cursor_line;
    let line = &mut state.xml_lines[line_index];
    let col = clamp_modal_text_offset(line, state.xml_cursor_col.min(line.len()));
    line.insert(col, ch);
    state.xml_cursor_col = col + ch.len_utf8();
    state.xml_dirty = true;
    state.preview_lines = state.xml_lines.clone();
}

pub(super) fn sync_text_document_edit_cache(state: &mut UiState, status: &str) {
    sync_legacy_cursor_from_document(state);
    state.xml_dirty = true;
    reset_caret_blink(state);
    rebuild_cached_spans(state);
    sync_xml_render_cache_from_active_document(state);
    state.preview_lines = state.xml_lines.clone();
    state.status = status.to_owned();
}

pub(super) fn next_visual_tab_stop(column: usize) -> usize {
    ((column / 4) + 1) * 4
}

pub(super) fn visual_column_for_line_offset(
    text: &str,
    line_start: usize,
    line_end: usize,
    offset: usize,
) -> usize {
    let offset = clamp_modal_text_offset(text, offset)
        .min(line_end)
        .max(line_start);
    let mut column = 0usize;
    for ch in text[line_start..offset].chars() {
        column = if ch == '\t' {
            next_visual_tab_stop(column)
        } else {
            column.saturating_add(1)
        };
    }
    column
}

pub(super) fn offset_for_visual_column(
    text: &str,
    line_start: usize,
    line_end: usize,
    target_column: usize,
) -> usize {
    let mut column = 0usize;
    for (relative, ch) in text[line_start..line_end].char_indices() {
        let offset = line_start + relative;
        let next_column = if ch == '\t' {
            next_visual_tab_stop(column)
        } else {
            column.saturating_add(1)
        };
        if next_column >= target_column {
            let distance_before = target_column.saturating_sub(column);
            let distance_after = next_column.saturating_sub(target_column);
            return if distance_before < distance_after {
                offset
            } else {
                offset + ch.len_utf8()
            };
        }
        column = next_column;
    }
    line_end
}

pub(super) fn move_active_document_caret_vertical(state: &mut UiState, delta: isize) -> bool {
    let Some(document) = state.active_document.as_mut() else {
        return false;
    };
    let Some(selection) = document.selections.first().cloned() else {
        return false;
    };
    let cursor = selection.normalized().1;
    let (line, _) = document.buffer.line_column_for_offset(cursor);
    let max_line = document.buffer.line_count().saturating_sub(1);
    let target_line = if delta < 0 {
        line.saturating_sub(delta.unsigned_abs())
    } else {
        line.saturating_add(delta as usize).min(max_line)
    };
    let text = document.buffer.as_str();
    let Some((line_start, line_end)) = line_byte_range(text, line) else {
        return false;
    };
    let visual_column = visual_column_for_line_offset(text, line_start, line_end, cursor);
    let Some((target_start, target_end)) = line_byte_range(text, target_line) else {
        return false;
    };
    let offset = offset_for_visual_column(text, target_start, target_end, visual_column);
    document.set_carets([offset]);
    reset_caret_blink(state);
    sync_legacy_cursor_from_document(state);
    state.inspector_scroll_rows = state.inspector_scroll_rows.min(state.xml_cursor_line);
    let visible_budget = 24usize;
    if state.xml_cursor_line >= state.inspector_scroll_rows.saturating_add(visible_budget) {
        state.inspector_scroll_rows = state
            .xml_cursor_line
            .saturating_sub(visible_budget.saturating_sub(1));
    }
    state.status = format!(
        "Caret: line={} col={}",
        state.xml_cursor_line + 1,
        state.xml_cursor_col
    );
    true
}

pub(super) fn sync_legacy_cursor_from_document(state: &mut UiState) {
    let Some(document) = state.active_document.as_ref() else {
        return;
    };
    let Some(selection) = document.selections.first() else {
        return;
    };
    let (line, col) = document.buffer.line_column_for_offset(selection.cursor);
    state.xml_cursor_line = line.min(state.xml_lines.len().saturating_sub(1));
    state.xml_cursor_col = col;
}

pub(super) fn rebuild_cached_spans(state: &mut UiState) {
    state.cached_spans = state
        .active_document
        .as_ref()
        .map(highlighted_spans_for_document)
        .unwrap_or_default();
}

pub(super) fn sync_xml_render_cache_from_active_document(state: &mut UiState) {
    let Some(document) = state.active_document.as_ref() else {
        state.xml_lines.clear();
        return;
    };
    state.xml_lines = document
        .buffer
        .as_str()
        .lines()
        .map(ToOwned::to_owned)
        .collect();
    if state.xml_lines.is_empty() {
        state.xml_lines.push(String::new());
    }
}

pub(super) fn is_xml_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("xml"))
        .unwrap_or(false)
}

pub(super) fn load_xml_lines(path: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return vec!["Unable to read XML file as UTF-8".to_owned()];
    };
    text.lines().map(ToOwned::to_owned).collect()
}

pub(super) fn is_text_preview_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "txt"
                    | "md"
                    | "markdown"
                    | "json"
                    | "toml"
                    | "ini"
                    | "cfg"
                    | "log"
                    | "rs"
                    | "py"
                    | "cpp"
                    | "c"
                    | "hpp"
                    | "h"
                    | "cs"
                    | "glsl"
                    | "hlsl"
                    | "lua"
                    | "yaml"
                    | "yml"
            )
        })
        .unwrap_or(false)
}

pub(super) fn load_text_preview_lines(path: &Path) -> Vec<String> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let mut lines: Vec<String> = text.lines().take(600).map(ToOwned::to_owned).collect();
            if lines.is_empty() {
                lines.push(String::new());
            }
            lines
        }
        Err(err) => vec![format!("Unable to read text preview: {err}")],
    }
}

pub(super) fn load_package_preview_lines(path: &Path) -> Vec<String> {
    vec![
        format!("Package: {}", path.display()),
        "Inline package entry listing is provider-backed.".to_owned(),
        "The package route is registered, but the provider must expose an entries/list preview command to render real contents here.".to_owned(),
    ]
}

pub(super) fn load_binary_preview_lines(path: &Path, kind: &str, provider: &str) -> Vec<String> {
    let metadata = fs::metadata(path).ok();
    vec![
        format!("Binary asset: {}", path.display()),
        format!("Type: {kind}"),
        format!("Provider: {provider}"),
        format!("Size: {}", format_size(metadata.map(|value| value.len()))),
        "Inline binary preview requires provider-rendered content/thumbnail/viewport output."
            .to_owned(),
    ]
}

pub(super) fn ensure_xml_line(state: &mut UiState) {
    if state.xml_lines.is_empty() {
        state.xml_lines.push(String::new());
        state.xml_cursor_line = 0;
        state.xml_cursor_col = 0;
    }
    if state.xml_cursor_line >= state.xml_lines.len() {
        state.xml_cursor_line = state.xml_lines.len().saturating_sub(1);
    }
}

pub(super) fn handle_xml_key(state: &mut UiState, key: Wparam) -> bool {
    if state.xml_search_focus {
        match key {
            VK_BACK => {
                state.xml_search_query.pop();
                state.status = format!("XML search: {}", state.xml_search_query);
                true
            }
            VK_ESCAPE => {
                state.xml_search_focus = false;
                state.status = "XML editor focused".to_owned();
                true
            }
            _ => false,
        }
    } else {
        match key {
            VK_UP => {
                if move_active_document_caret_vertical(state, -1) {
                    return true;
                }
                state.xml_cursor_line = state.xml_cursor_line.saturating_sub(1);
                state.inspector_scroll_rows =
                    state.inspector_scroll_rows.min(state.xml_cursor_line);
                true
            }
            VK_DOWN => {
                if move_active_document_caret_vertical(state, 1) {
                    return true;
                }
                state.xml_cursor_line =
                    (state.xml_cursor_line + 1).min(state.xml_lines.len().saturating_sub(1));
                true
            }
            VK_BACK => {
                xml_backspace(state);
                true
            }
            VK_DELETE_FORWARD => {
                xml_delete_next_char(state);
                true
            }
            VK_RETURN => {
                xml_newline(state);
                true
            }
            VK_ESCAPE => {
                state.xml_search_focus = true;
                state.status = "XML search focused".to_owned();
                true
            }
            _ => false,
        }
    }
}

pub(super) fn xml_backspace(state: &mut UiState) {
    ensure_xml_line(state);
    if let Some(document) = state.active_document.as_mut() {
        document.backspace();
        sync_text_document_edit_cache(state, "Text buffer edited");
        return;
    }
    let line = &mut state.xml_lines[state.xml_cursor_line];
    let cursor = clamp_modal_text_offset(line, state.xml_cursor_col.min(line.len()));
    if cursor > 0 {
        let prev = line[..cursor]
            .chars()
            .last()
            .map(char::len_utf8)
            .unwrap_or(1);
        let from = cursor.saturating_sub(prev);
        line.replace_range(from..cursor, "");
        state.xml_cursor_col = from;
        state.xml_dirty = true;
        state.status = "XML buffer edited".to_owned();
    }
}

pub(super) fn xml_delete_next_char(state: &mut UiState) {
    ensure_xml_line(state);
    if let Some(document) = state.active_document.as_mut() {
        document.delete_forward();
        sync_text_document_edit_cache(state, "Text buffer edited");
        return;
    }
    let line = &mut state.xml_lines[state.xml_cursor_line];
    let cursor = clamp_modal_text_offset(line, state.xml_cursor_col.min(line.len()));
    if cursor < line.len() {
        let next = line[cursor..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(0);
        line.replace_range(cursor..cursor + next, "");
        state.xml_cursor_col = cursor;
        state.xml_dirty = true;
        state.status = "XML buffer edited".to_owned();
    }
}

pub(super) fn xml_newline(state: &mut UiState) {
    ensure_xml_line(state);
    if let Some(document) = state.active_document.as_mut() {
        document.insert_newline();
        sync_text_document_edit_cache(state, "Text buffer edited");
        return;
    }
    let current = state.xml_lines[state.xml_cursor_line].clone();
    let col = clamp_modal_text_offset(&current, state.xml_cursor_col.min(current.len()));
    let left = current[..col].to_owned();
    let right = current[col..].to_owned();
    state.xml_lines[state.xml_cursor_line] = left;
    state.xml_lines.insert(state.xml_cursor_line + 1, right);
    state.xml_cursor_line += 1;
    state.xml_cursor_col = 0;
    state.xml_dirty = true;
    state.status = "XML buffer edited".to_owned();
}
