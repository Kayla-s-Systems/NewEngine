use crate::editors::text_editor::syntax::SyntaxProfile;
use crate::editors::text_editor::token::{TokenKind, TokenSpan};

pub fn highlight(text: &str, profile: &SyntaxProfile) -> Vec<TokenSpan> {
    let mut spans = Vec::new();
    let mut i = 0;

    while i < text.len() {
        let Some(ch) = text[i..].chars().next() else { break; };

        if let Some(prefix) = profile
            .line_comment_prefixes
            .iter()
            .find(|prefix| text[i..].starts_with(prefix.as_str()))
        {
            let _prefix = prefix;
            let end = text[i..].find('\n').map(|offset| i + offset).unwrap_or(text.len());
            spans.push(TokenSpan::new(i, end, TokenKind::Comment));
            i = end;
        } else if ch.is_whitespace() {
            let start = i;
            i += ch.len_utf8();
            while i < text.len() {
                let Some(next) = text[i..].chars().next() else { break; };
                if !next.is_whitespace() { break; }
                i += next.len_utf8();
            }
            spans.push(TokenSpan::new(start, i, TokenKind::Whitespace));
        } else if matches!(ch, '"' | '\'') {
            let quote = ch;
            let start = i;
            i += ch.len_utf8();
            while i < text.len() {
                let Some(next) = text[i..].chars().next() else { break; };
                if next == '\\' {
                    i += next.len_utf8();
                    if i < text.len() {
                        let Some(escaped) = text[i..].chars().next() else { break; };
                        i += escaped.len_utf8();
                    }
                    continue;
                }
                i += next.len_utf8();
                if next == quote { break; }
            }
            spans.push(TokenSpan::new(start, i, TokenKind::String));
        } else if ch.is_ascii_digit() {
            let start = i;
            i += ch.len_utf8();
            while i < text.len() {
                let Some(next) = text[i..].chars().next() else { break; };
                if !(next.is_ascii_digit() || next == '.') { break; }
                i += next.len_utf8();
            }
            spans.push(TokenSpan::new(start, i, TokenKind::Number));
        } else if is_identifier_start(ch) {
            let start = i;
            i += ch.len_utf8();
            while i < text.len() {
                let Some(next) = text[i..].chars().next() else { break; };
                if !is_identifier_continue(next) { break; }
                i += next.len_utf8();
            }
            let word = &text[start..i];
            let kind = if profile.keywords.contains(word) {
                TokenKind::Keyword
            } else {
                TokenKind::Identifier
            };
            spans.push(TokenSpan::new(start, i, kind));
        } else {
            spans.push(TokenSpan::new(i, i + ch.len_utf8(), TokenKind::Operator));
            i += ch.len_utf8();
        }
    }

    spans
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}
