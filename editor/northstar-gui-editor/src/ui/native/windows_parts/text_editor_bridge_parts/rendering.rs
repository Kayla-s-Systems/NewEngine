use super::*;

pub(super) fn highlighted_spans_for_document(document: &TextDocument) -> Vec<TokenSpan> {
    let registry = SyntaxRegistry::with_builtin_profiles();
    let widget = TextEditorWidget {
        document: document.clone(),
    };
    widget.highlighted_spans(&registry)
}

pub(super) unsafe fn draw_highlighted_document_line_tokens(
    hdc: Hdc,
    mut x: i32,
    y: i32,
    right: i32,
    text: &str,
    spans: &[northstar_gui_editor_ui::editors::text_editor::TokenSpan],
    line_index: usize,
    theme: &EditorColorDictionary,
) {
    let Some((line_start, line_end)) = line_byte_range(text, line_index) else {
        return;
    };
    for span in spans {
        let start = span.start.max(line_start);
        let end = span.end.min(line_end);
        if start >= end {
            continue;
        }
        let fragment = &text[start..end];
        let color = token_color(theme, span.kind);
        draw_text(
            hdc,
            Rect {
                left: x,
                top: y,
                right,
                bottom: y + 18,
            },
            fragment,
            color,
            false,
        );
        x += measured_text_width(fragment);
        if x > right {
            break;
        }
    }
}

pub(super) fn line_byte_range(text: &str, target_line: usize) -> Option<(usize, usize)> {
    let mut start = 0;
    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        let mut end = start + line.len();
        if line.ends_with('\n') {
            end = end.saturating_sub(1);
            if end > start && line.as_bytes().get(line.len().saturating_sub(2)) == Some(&b'\r') {
                end = end.saturating_sub(1);
            }
        }
        if line_index == target_line {
            return Some((start, end));
        }
        start += line.len();
    }
    if target_line == 0 && text.is_empty() {
        return Some((0, 0));
    }
    None
}

pub(super) fn sublime_background(theme: &EditorColorDictionary) -> Dword {
    if theme.name == "Visual Studio [TM]" {
        rgb(30, 32, 30)
    } else {
        theme_color(theme.background)
    }
}

pub(super) fn sublime_header_background(theme: &EditorColorDictionary) -> Dword {
    if theme.name == "Visual Studio [TM]" {
        rgb(38, 40, 38)
    } else {
        theme_color(theme.active_line_background)
    }
}

pub(super) fn sublime_gutter_background(theme: &EditorColorDictionary) -> Dword {
    if theme.name == "Visual Studio [TM]" {
        rgb(26, 28, 26)
    } else {
        theme_color(theme.active_line_background)
    }
}

pub(super) fn sublime_current_line(theme: &EditorColorDictionary) -> Dword {
    if theme.name == "Visual Studio [TM]" {
        rgb(42, 45, 42)
    } else {
        theme_color(theme.active_line_background)
    }
}

pub(super) fn sublime_border(theme: &EditorColorDictionary) -> Dword {
    if theme.name == "Visual Studio [TM]" {
        rgb(64, 65, 58)
    } else {
        theme_color(theme.folding_line)
    }
}

pub(super) fn sublime_foreground(theme: &EditorColorDictionary) -> Dword {
    if theme.name == "Visual Studio [TM]" {
        rgb(210, 213, 205)
    } else {
        theme_color(theme.editor_foreground)
    }
}

pub(super) fn token_color(theme: &EditorColorDictionary, kind: TokenKind) -> Dword {
    match kind {
        TokenKind::Keyword => theme_color(theme.reserved_word),
        TokenKind::String => theme_color(theme.string),
        TokenKind::Number => theme_color(theme.number),
        TokenKind::Comment => theme_color(theme.comment),
        TokenKind::Tag => theme_color(theme.reserved_word),
        TokenKind::Attribute => theme_color(theme.attribute),
        TokenKind::Identifier => sublime_foreground(theme),
        TokenKind::Operator => theme_color(theme.symbol),
        TokenKind::Error => rgb(220, 38, 38),
        TokenKind::Whitespace | TokenKind::Text => sublime_foreground(theme),
    }
}

pub(super) fn measured_text_width(text: &str) -> i32 {
    let columns = text
        .chars()
        .map(|ch| if ch == '\t' { 4 } else { 1 })
        .sum::<usize>();
    (columns as i32).saturating_mul(8)
}

pub(super) unsafe fn draw_legacy_line_tokens(
    hdc: Hdc,
    mut x: i32,
    y: i32,
    right: i32,
    line: &str,
    theme: &EditorColorDictionary,
) {
    let mut token = String::new();
    let mut in_tag = false;
    let mut in_quote = false;
    for ch in line.chars() {
        let color = if ch == '<' || ch == '>' || ch == '/' || ch == '=' {
            theme_color(theme.symbol)
        } else if in_quote || ch == '"' {
            theme_color(theme.string)
        } else if in_tag {
            theme_color(theme.reserved_word)
        } else if ch.is_ascii_digit() {
            theme_color(theme.number)
        } else {
            theme_color(theme.editor_foreground)
        };
        token.clear();
        token.push(ch);
        draw_text(
            hdc,
            Rect {
                left: x,
                top: y,
                right,
                bottom: y + 18,
            },
            &token,
            color,
            false,
        );
        x += 8;
        if x > right {
            break;
        }
        if ch == '<' {
            in_tag = true;
        }
        if ch == '>' {
            in_tag = false;
        }
        if ch == '"' {
            in_quote = !in_quote;
        }
    }
}
