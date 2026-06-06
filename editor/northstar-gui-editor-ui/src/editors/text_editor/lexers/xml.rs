use crate::editors::text_editor::token::{TokenKind, TokenSpan};

pub fn highlight(text: &str) -> Vec<TokenSpan> {
    let mut spans = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if text[i..].starts_with("<!--") {
            let end = text[i + 4..]
                .find("-->")
                .map(|offset| i + 4 + offset + 3)
                .unwrap_or(bytes.len());
            spans.push(TokenSpan::new(i, end, TokenKind::Comment));
            i = end;
        } else if matches!(bytes[i], b'<' | b'>' | b'/') {
            spans.push(TokenSpan::new(i, i + 1, TokenKind::Tag));
            i += 1;
        } else if matches!(bytes[i], b'"' | b'\'') {
            let quote = bytes[i];
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != quote {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            spans.push(TokenSpan::new(start, i, TokenKind::String));
        } else if bytes[i].is_ascii_alphabetic() || matches!(bytes[i], b'_' | b':') {
            let start = i;
            i += 1;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'-' | b':' | b'.'))
            {
                i += 1;
            }
            let kind = if text[i..].trim_start().starts_with('=') {
                TokenKind::Attribute
            } else {
                TokenKind::Identifier
            };
            spans.push(TokenSpan::new(start, i, kind));
        } else {
            let start = i;
            i += 1;
            while i < bytes.len()
                && !matches!(bytes[i], b'<' | b'>' | b'/' | b'"' | b'\'')
                && !bytes[i].is_ascii_alphanumeric()
            {
                i += 1;
            }
            spans.push(TokenSpan::new(start, i, TokenKind::Text));
        }
    }

    spans
}
