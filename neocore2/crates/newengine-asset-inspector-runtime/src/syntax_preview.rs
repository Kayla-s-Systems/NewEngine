//! Lightweight syntax-highlighted source preview for Asset Inspector.
//!
//! The right-side preview is intentionally read-only. Editing remains owned by
//! the main text editor, while this module produces fixed-width color masks for
//! Aurelia UI. Every mask has exactly the same character count, so the layers
//! align when rendered with the bundled `@debug` monospace face.

pub(crate) const SYNTAX_PREVIEW_ROWS: usize = 12;
pub(crate) const SYNTAX_PREVIEW_COLUMNS: usize = 70;
pub(crate) const SYNTAX_EDITOR_ROWS: usize = 16;
pub(crate) const SYNTAX_EDITOR_COLUMNS: usize = 116;
pub(crate) const SYNTAX_LAYER_COUNT: usize = 8;
pub(crate) const SYNTAX_LAYER_NAMES: [&str; SYNTAX_LAYER_COUNT] = [
    "plain",
    "comment",
    "string",
    "attribute",
    "number",
    "symbol",
    "reserved",
    "link",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum SyntaxClass {
    #[default]
    Plain = 0,
    Comment = 1,
    String = 2,
    Attribute = 3,
    Number = 4,
    Symbol = 5,
    Reserved = 6,
    Link = 7,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SyntaxPreviewRow {
    pub(crate) line_number: usize,
    pub(crate) layers: [String; SYNTAX_LAYER_COUNT],
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SyntaxPreviewPage {
    pub(crate) language: String,
    pub(crate) start_line: usize,
    pub(crate) total_lines: usize,
    pub(crate) rows: Vec<SyntaxPreviewRow>,
}

impl SyntaxPreviewPage {
    pub(crate) fn page_label(&self) -> String {
        if self.total_lines == 0 || self.rows.is_empty() {
            return "0 / 0".to_owned();
        }
        let end = self.start_line + self.rows.len();
        format!("{}-{} / {}", self.start_line + 1, end, self.total_lines)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct LexerState {
    in_xml_comment: bool,
    in_block_comment: bool,
}

mod profiles;
mod projection;
mod scanner;
#[cfg(test)]
mod tests;

pub(crate) use projection::{highlight_editor_page, highlight_preview_page};
