use super::super::scanner::*;
use super::super::*;

pub(super) fn classify_ini(chars: &[char]) -> Vec<SyntaxClass> {
    let mut classes = vec![SyntaxClass::Plain; chars.len()];
    let first = chars.iter().position(|ch| !ch.is_whitespace());
    let Some(first) = first else {
        return classes;
    };
    if chars[first] == ';' {
        paint(&mut classes, first, chars.len(), SyntaxClass::Comment);
        return classes;
    }
    if chars[first] == '[' {
        classes[first] = SyntaxClass::Symbol;
        let close = chars[first + 1..]
            .iter()
            .position(|ch| *ch == ']')
            .map(|offset| first + 1 + offset);
        let end = close.unwrap_or(chars.len());
        paint(&mut classes, first + 1, end, SyntaxClass::Reserved);
        if let Some(close) = close {
            classes[close] = SyntaxClass::Symbol;
        }
        return classes;
    }
    if let Some(equal) = chars.iter().position(|ch| *ch == '=') {
        let key_end = (0..equal)
            .rev()
            .find(|index| !chars[*index].is_whitespace())
            .map(|index| index + 1)
            .unwrap_or(0);
        paint(&mut classes, first, key_end, SyntaxClass::Attribute);
        classes[equal] = SyntaxClass::Symbol;
        let value_start = (equal + 1..chars.len())
            .find(|index| !chars[*index].is_whitespace())
            .unwrap_or(chars.len());
        if value_start < chars.len()
            && (chars[value_start].is_ascii_digit()
                || matches!(chars[value_start], '+' | '-' | '.'))
        {
            let end = take_number(chars, value_start);
            paint(&mut classes, value_start, end, SyntaxClass::Number);
            if end < chars.len() {
                paint(&mut classes, end, chars.len(), SyntaxClass::String);
            }
        } else {
            paint(&mut classes, value_start, chars.len(), SyntaxClass::String);
        }
    }
    classes
}
