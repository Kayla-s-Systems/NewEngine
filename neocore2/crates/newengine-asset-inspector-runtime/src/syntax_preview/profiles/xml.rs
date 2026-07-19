use super::super::scanner::*;
use super::super::*;

pub(super) fn classify_xml(chars: &[char], state: &mut LexerState) -> Vec<SyntaxClass> {
    let mut classes = vec![SyntaxClass::Plain; chars.len()];
    let mut index = 0;
    while index < chars.len() {
        if state.in_xml_comment {
            let close = find_sequence(chars, index, &['-', '-', '>']);
            let end = close.map(|value| value + 3).unwrap_or(chars.len());
            paint(&mut classes, index, end, SyntaxClass::Comment);
            state.in_xml_comment = close.is_none();
            index = end;
            continue;
        }
        if starts_with(chars, index, &['<', '!', '-', '-']) {
            let close = find_sequence(chars, index + 4, &['-', '-', '>']);
            let end = close.map(|value| value + 3).unwrap_or(chars.len());
            paint(&mut classes, index, end, SyntaxClass::Comment);
            state.in_xml_comment = close.is_none();
            index = end;
            continue;
        }
        if chars[index] != '<' {
            if chars[index].is_ascii_digit() {
                let end = take_number(chars, index);
                paint(&mut classes, index, end, SyntaxClass::Number);
                index = end;
            } else {
                index += 1;
            }
            continue;
        }

        classes[index] = SyntaxClass::Symbol;
        index += 1;
        while index < chars.len() && chars[index].is_whitespace() {
            index += 1;
        }
        if index < chars.len() && matches!(chars[index], '/' | '?' | '!') {
            classes[index] = SyntaxClass::Symbol;
            index += 1;
        }
        let name_start = index;
        while index < chars.len() && is_xml_name_char(chars[index]) {
            index += 1;
        }
        paint(&mut classes, name_start, index, SyntaxClass::Reserved);

        while index < chars.len() {
            match chars[index] {
                '>' => {
                    classes[index] = SyntaxClass::Symbol;
                    index += 1;
                    break;
                }
                '/' | '?' => {
                    classes[index] = SyntaxClass::Symbol;
                    index += 1;
                }
                '"' | '\'' => {
                    let quote = chars[index];
                    let end = take_quoted(chars, index, quote);
                    paint(&mut classes, index, end, SyntaxClass::String);
                    index = end;
                }
                '=' => {
                    classes[index] = SyntaxClass::Symbol;
                    index += 1;
                }
                ch if ch.is_ascii_digit() => {
                    let end = take_number(chars, index);
                    paint(&mut classes, index, end, SyntaxClass::Number);
                    index = end;
                }
                ch if is_xml_name_char(ch) => {
                    let start = index;
                    while index < chars.len() && is_xml_name_char(chars[index]) {
                        index += 1;
                    }
                    paint(&mut classes, start, index, SyntaxClass::Attribute);
                }
                _ => index += 1,
            }
        }
    }
    classes
}
