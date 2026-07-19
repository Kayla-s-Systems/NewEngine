use super::super::scanner::*;
use super::super::*;

pub(super) fn classify_text(chars: &[char]) -> Vec<SyntaxClass> {
    let mut classes = vec![SyntaxClass::Plain; chars.len()];
    let prefixes = [
        "https://",
        "http://",
        "news://",
        "news:",
        "gopher://",
        "prospero://",
        "nntp://",
        "ftp://",
        "wais://",
        "telnet://",
        "mailto:",
        "www",
    ];
    let mut index = 0;
    while index < chars.len() {
        let matched = prefixes
            .iter()
            .find(|prefix| starts_with_str(chars, index, prefix));
        if matched.is_some() {
            let mut end = index;
            while end < chars.len()
                && !chars[end].is_whitespace()
                && !matches!(chars[end], '<' | '>' | '{' | '}' | '^' | '|')
            {
                end += 1;
            }
            paint(&mut classes, index, end, SyntaxClass::Link);
            index = end;
        } else {
            index += 1;
        }
    }
    classes
}
