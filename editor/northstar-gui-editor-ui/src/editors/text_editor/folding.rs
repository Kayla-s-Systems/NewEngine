use crate::editors::text_editor::syntax::SyntaxProfile;
use crate::editors::text_editor::token::{TokenKind, TokenSpan};
use crate::editors::text_editor::TextBuffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoldRegion {
    pub start_line: usize,
    pub end_line: usize,
}

pub struct FoldingBuilder;

impl FoldingBuilder {
    pub fn build(buffer: &TextBuffer, tokens: &[TokenSpan], profile: &SyntaxProfile) -> Vec<FoldRegion> {
        match profile.grammar.as_str() {
            "xml" => build_xml_folds(buffer, tokens),
            _ => build_brace_folds(buffer),
        }
    }
}

fn build_xml_folds(buffer: &TextBuffer, tokens: &[TokenSpan]) -> Vec<FoldRegion> {
    let text = buffer.as_str();
    let mut stack: Vec<(String, usize)> = Vec::new();
    let mut regions = Vec::new();

    for window in tokens.windows(2) {
        let tag = &window[0];
        let name = &window[1];
        if tag.kind != TokenKind::Tag || name.kind != TokenKind::Identifier {
            continue;
        }
        let tag_text = &text[tag.start..tag.end];
        let name_text = &text[name.start..name.end];
        if tag_text == "<" && !name_text.starts_with('/') {
            let (line, _) = buffer.line_column_for_offset(tag.start);
            stack.push((name_text.to_owned(), line));
        } else if tag_text == "<" && name_text.starts_with('/') {
            let closing = name_text.trim_start_matches('/');
            if let Some(index) = stack.iter().rposition(|(open, _)| open == closing) {
                let (_, start_line) = stack.remove(index);
                let (end_line, _) = buffer.line_column_for_offset(name.end);
                if end_line > start_line {
                    regions.push(FoldRegion { start_line, end_line });
                }
            }
        }
    }

    regions
}

fn build_brace_folds(buffer: &TextBuffer) -> Vec<FoldRegion> {
    let text = buffer.as_str();
    let mut stack = Vec::new();
    let mut regions = Vec::new();

    for (offset, ch) in text.char_indices() {
        match ch {
            '{' | '[' | '(' => {
                let (line, _) = buffer.line_column_for_offset(offset);
                stack.push(line);
            }
            '}' | ']' | ')' => {
                let Some(start_line) = stack.pop() else {
                    continue;
                };
                let (end_line, _) = buffer.line_column_for_offset(offset);
                if end_line > start_line {
                    regions.push(FoldRegion { start_line, end_line });
                }
            }
            _ => {}
        }
    }

    regions
}
