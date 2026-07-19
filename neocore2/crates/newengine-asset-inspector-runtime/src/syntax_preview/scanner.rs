use super::*;

pub(super) fn is_xml_name_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-' | ':' | '.')
}

pub(super) fn is_identifier_start(ch: char) -> bool {
    ch.is_alphabetic() || matches!(ch, '_' | '$')
}

pub(super) fn take_identifier(chars: &[char], start: usize) -> usize {
    let mut end = start;
    while end < chars.len()
        && (chars[end].is_alphanumeric() || matches!(chars[end], '_' | '$' | '-'))
    {
        end += 1;
    }
    end
}

pub(super) fn take_number(chars: &[char], start: usize) -> usize {
    let mut end = start;
    while end < chars.len()
        && (chars[end].is_ascii_hexdigit()
            || matches!(chars[end], '.' | '_' | '+' | '-' | 'e' | 'E' | 'x' | 'X'))
    {
        end += 1;
    }
    end
}

pub(super) fn take_quoted(chars: &[char], start: usize, quote: char) -> usize {
    chars[start + 1..]
        .iter()
        .position(|ch| *ch == quote)
        .map(|offset| start + 2 + offset)
        .unwrap_or(chars.len())
}

pub(super) fn take_quoted_escaped(chars: &[char], start: usize, quote: char) -> usize {
    let mut index = start + 1;
    let mut escaped = false;
    while index < chars.len() {
        let ch = chars[index];
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == quote {
            return index + 1;
        }
        index += 1;
    }
    chars.len()
}

pub(super) fn take_json_string(chars: &[char], start: usize) -> usize {
    take_quoted_escaped(chars, start, '"')
}

pub(super) fn paint(classes: &mut [SyntaxClass], start: usize, end: usize, class: SyntaxClass) {
    let end = end.min(classes.len());
    if start < end {
        classes[start..end].fill(class);
    }
}

pub(super) fn find_sequence(chars: &[char], start: usize, sequence: &[char]) -> Option<usize> {
    if sequence.is_empty() || start >= chars.len() {
        return None;
    }
    (start..=chars.len().saturating_sub(sequence.len()))
        .find(|index| starts_with(chars, *index, sequence))
}

pub(super) fn starts_with(chars: &[char], start: usize, sequence: &[char]) -> bool {
    chars.get(start..start.saturating_add(sequence.len())) == Some(sequence)
}

pub(super) fn starts_with_str(chars: &[char], start: usize, value: &str) -> bool {
    let sequence = value.chars().collect::<Vec<_>>();
    starts_with(chars, start, &sequence)
}
