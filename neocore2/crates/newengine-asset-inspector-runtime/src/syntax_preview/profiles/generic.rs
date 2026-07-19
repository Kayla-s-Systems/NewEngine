use super::super::scanner::*;
use super::super::*;

pub(super) fn classify_generic(
    chars: &[char],
    language: &str,
    state: &mut LexerState,
) -> Vec<SyntaxClass> {
    let mut classes = vec![SyntaxClass::Plain; chars.len()];
    let mut index = 0;
    let line_comment = match language {
        "python" | "yaml" | "toml" | "shell" | "powershell" => Some("#"),
        "lua" => Some("--"),
        _ => Some("//"),
    };
    while index < chars.len() {
        if state.in_block_comment {
            let close = find_sequence(chars, index, &['*', '/']);
            let end = close.map(|value| value + 2).unwrap_or(chars.len());
            paint(&mut classes, index, end, SyntaxClass::Comment);
            state.in_block_comment = close.is_none();
            index = end;
            continue;
        }
        if let Some(marker) = line_comment {
            if starts_with_str(chars, index, marker) {
                paint(&mut classes, index, chars.len(), SyntaxClass::Comment);
                break;
            }
        }
        if starts_with(chars, index, &['/', '*']) {
            let close = find_sequence(chars, index + 2, &['*', '/']);
            let end = close.map(|value| value + 2).unwrap_or(chars.len());
            paint(&mut classes, index, end, SyntaxClass::Comment);
            state.in_block_comment = close.is_none();
            index = end;
            continue;
        }
        match chars[index] {
            '"' | '\'' | '`' => {
                let quote = chars[index];
                let end = take_quoted_escaped(chars, index, quote);
                paint(&mut classes, index, end, SyntaxClass::String);
                index = end;
            }
            ch if ch.is_ascii_digit() => {
                let end = take_number(chars, index);
                paint(&mut classes, index, end, SyntaxClass::Number);
                index = end;
            }
            ch if is_identifier_start(ch) => {
                let end = take_identifier(chars, index);
                let word = chars[index..end].iter().collect::<String>();
                if is_generic_keyword(&word) {
                    paint(&mut classes, index, end, SyntaxClass::Reserved);
                }
                index = end;
            }
            ch if "{}[]():;,.=+-*/<>!?&|".contains(ch) => {
                classes[index] = SyntaxClass::Symbol;
                index += 1;
            }
            _ => index += 1,
        }
    }
    classes
}

fn is_generic_keyword(word: &str) -> bool {
    matches!(
        word,
        "as" | "async"
            | "await"
            | "break"
            | "case"
            | "class"
            | "const"
            | "continue"
            | "def"
            | "do"
            | "else"
            | "enum"
            | "false"
            | "fn"
            | "for"
            | "function"
            | "if"
            | "impl"
            | "import"
            | "in"
            | "interface"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "mut"
            | "new"
            | "null"
            | "private"
            | "protected"
            | "pub"
            | "public"
            | "return"
            | "self"
            | "static"
            | "struct"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "type"
            | "use"
            | "var"
            | "void"
            | "while"
            | "yield"
    )
}
