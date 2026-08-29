use super::super::scanner::*;
use super::super::*;

pub(super) fn classify_json(chars: &[char]) -> Vec<SyntaxClass> {
    let mut classes = vec![SyntaxClass::Plain; chars.len()];
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '"' => {
                let end = take_json_string(chars, index);
                let is_key = chars[end..]
                    .iter()
                    .find(|ch| !ch.is_whitespace())
                    .is_some_and(|ch| *ch == ':');
                paint(
                    &mut classes,
                    index,
                    end,
                    if is_key {
                        SyntaxClass::Attribute
                    } else {
                        SyntaxClass::String
                    },
                );
                index = end;
            }
            '/' if starts_with(chars, index, &['/', '/']) => {
                paint(&mut classes, index, chars.len(), SyntaxClass::Comment);
                break;
            }
            '/' if starts_with(chars, index, &['/', '*']) => {
                let close = find_sequence(chars, index + 2, &['*', '/']);
                let end = close.map(|value| value + 2).unwrap_or(chars.len());
                paint(&mut classes, index, end, SyntaxClass::Comment);
                index = end;
            }
            ch if ch.is_ascii_digit() || matches!(ch, '-' | '+') => {
                let end = take_number(chars, index);
                paint(&mut classes, index, end, SyntaxClass::Number);
                index = end.max(index + 1);
            }
            ':' | ',' | '[' | ']' | '{' | '}' => {
                classes[index] = SyntaxClass::Symbol;
                index += 1;
            }
            ch if is_identifier_start(ch) => {
                let end = take_identifier(chars, index);
                let word = chars[index..end].iter().collect::<String>();
                if matches!(
                    word.as_str(),
                    "true" | "false" | "null" | "NaN" | "Infinity"
                ) {
                    paint(&mut classes, index, end, SyntaxClass::Reserved);
                }
                index = end;
            }
            _ => index += 1,
        }
    }
    classes
}
