use super::profiles::{classify_line, normalize_language};
use super::*;

pub(crate) fn highlight_preview_page(
    lines: &[String],
    language: &str,
    start_line: usize,
) -> SyntaxPreviewPage {
    highlight_page(
        lines,
        language,
        start_line,
        SYNTAX_PREVIEW_ROWS,
        SYNTAX_PREVIEW_COLUMNS,
    )
}

pub(crate) fn highlight_editor_page(
    lines: &[String],
    language: &str,
    start_line: usize,
) -> SyntaxPreviewPage {
    highlight_page(
        lines,
        language,
        start_line,
        SYNTAX_EDITOR_ROWS,
        SYNTAX_EDITOR_COLUMNS,
    )
}

fn highlight_page(
    lines: &[String],
    language: &str,
    start_line: usize,
    row_limit: usize,
    column_limit: usize,
) -> SyntaxPreviewPage {
    let start_line = start_line.min(lines.len());
    let end_line = (start_line + row_limit.max(1)).min(lines.len());
    let mut state = LexerState::default();
    let mut rows = Vec::with_capacity(end_line.saturating_sub(start_line));

    // Scan preceding lines to preserve multiline comment state without storing a
    // full highlighted copy of a potentially 1 MiB document.
    for line in lines.iter().take(end_line).enumerate() {
        let (index, line) = line;
        let expanded = expand_tabs(line, 4);
        let (chars, truncated) = clipped_chars(&expanded, column_limit.max(2));
        let classes = classify_line(&chars, language, &mut state);
        if index >= start_line {
            rows.push(SyntaxPreviewRow {
                line_number: index + 1,
                layers: build_masks(&chars, &classes, truncated),
            });
        }
    }

    SyntaxPreviewPage {
        language: normalize_language(language).to_owned(),
        start_line,
        total_lines: lines.len(),
        rows,
    }
}

fn build_masks(
    chars: &[char],
    classes: &[SyntaxClass],
    truncated: bool,
) -> [String; SYNTAX_LAYER_COUNT] {
    let mut masks: [String; SYNTAX_LAYER_COUNT] =
        std::array::from_fn(|_| String::with_capacity(chars.len().saturating_add(1)));
    for (ch, class) in chars.iter().zip(classes.iter()) {
        for (layer, mask) in masks.iter_mut().enumerate() {
            mask.push(if layer == *class as usize { *ch } else { ' ' });
        }
    }
    if truncated {
        for mask in &mut masks {
            mask.push(' ');
        }
        masks[SyntaxClass::Plain as usize].pop();
        masks[SyntaxClass::Plain as usize].push('…');
    }
    masks
}

fn expand_tabs(value: &str, tab_width: usize) -> String {
    let mut out = String::with_capacity(value.len());
    let mut column = 0usize;
    for ch in value.chars() {
        if ch == '\t' {
            let count = tab_width - column % tab_width;
            out.extend(std::iter::repeat_n(' ', count));
            column += count;
        } else if !ch.is_control() {
            out.push(ch);
            column += 1;
        }
    }
    out
}

fn clipped_chars(value: &str, max_columns: usize) -> (Vec<char>, bool) {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= max_columns {
        (chars, false)
    } else {
        (chars[..max_columns.saturating_sub(1)].to_vec(), true)
    }
}
