#![forbid(unsafe_op_in_unsafe_fn)]

const BOX_MIN_WIDTH: usize = 47;
const BOX_MAX_WIDTH: usize = 118;

#[inline]
pub(crate) fn ellipsize(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max_chars {
        return value.to_owned();
    }

    if max_chars == 1 {
        return ".".to_owned();
    }

    let mut out = String::with_capacity(max_chars);
    for ch in chars.into_iter().take(max_chars.saturating_sub(1)) {
        out.push(ch);
    }
    out.push_str("...");
    out
}

pub(crate) fn emit_boxed_kv(title: &str, rows: &[(&str, String)]) {
    if !newengine_ulog_api::ulog::debug_enabled() {
        return;
    }

    let key_width = rows.iter().map(|(key, _)| key.chars().count()).max().unwrap_or(0);
    let value_width = rows
        .iter()
        .flat_map(|(_, value)| wrap_value(value, 80))
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);

    let desired_inner_width = key_width
        .saturating_add(if key_width > 0 { 3 } else { 0 })
        .saturating_add(value_width)
        .max(title.chars().count())
        .max(BOX_MIN_WIDTH);

    let inner_width = desired_inner_width.min(BOX_MAX_WIDTH);
    let value_area_width = inner_width
        .saturating_sub(key_width)
        .saturating_sub(if key_width > 0 { 3 } else { 0 })
        .max(16);

    newengine_ulog_api::ulog::debug!("+{}", "-".repeat(inner_width + 2));
    newengine_ulog_api::ulog::debug!("| {}", pad_right(title, inner_width));

    if rows.is_empty() {
        newengine_ulog_api::ulog::debug!("+{}", "-".repeat(inner_width + 2));
        return;
    }

    newengine_ulog_api::ulog::debug!("+{}", "-".repeat(inner_width + 2));

    for (key, value) in rows {
        let wrapped = wrap_value(value, value_area_width);
        for (index, line) in wrapped.iter().enumerate() {
            if key_width == 0 {
                newengine_ulog_api::ulog::debug!("| {}", pad_right(line, inner_width));
                continue;
            }

            if index == 0 {
                let rendered = format!("{:<key_width$} : {}", key, line, key_width = key_width);
                newengine_ulog_api::ulog::debug!("| {}", pad_right(&rendered, inner_width));
            } else {
                let rendered = format!("{:<key_width$}   {}", "", line, key_width = key_width);
                newengine_ulog_api::ulog::debug!("| {}", pad_right(&rendered, inner_width));
            }
        }
    }

    newengine_ulog_api::ulog::debug!("+{}", "-".repeat(inner_width + 2));
}

pub(crate) fn emit_prefixed_table(
    prefix: &str,
    title: &str,
    headers: &[&str],
    rows: &[Vec<String>],
) {
    if !newengine_ulog_api::ulog::debug_enabled() {
        return;
    }

    if headers.is_empty() {
        return;
    }

    let mut widths: Vec<usize> = headers.iter().map(|header| header.chars().count()).collect();
    for row in rows {
        for (index, value) in row.iter().enumerate() {
            if index < widths.len() {
                widths[index] = widths[index].max(value.chars().count());
            }
        }
    }

    let border = widths
        .iter()
        .map(|width| format!("-{}-", "-".repeat(*width)))
        .collect::<Vec<_>>()
        .join("+");

    emit_prefixed_line(prefix, title);
    emit_prefixed_line(prefix, &format!("+{}+", border));

    let header_line = headers
        .iter()
        .enumerate()
        .map(|(index, header)| format!(" {} ", pad_right(header, widths[index])))
        .collect::<Vec<_>>()
        .join("|");
    emit_prefixed_line(prefix, &format!("|{}|", header_line));
    emit_prefixed_line(prefix, &format!("+{}+", border));

    for row in rows {
        let rendered = widths
            .iter()
            .enumerate()
            .map(|(index, width)| {
                let value = row.get(index).map(String::as_str).unwrap_or("");
                format!(" {} ", pad_right(value, *width))
            })
            .collect::<Vec<_>>()
            .join("|");
        emit_prefixed_line(prefix, &format!("|{}|", rendered));
    }

    emit_prefixed_line(prefix, &format!("+{}+", border));
}

#[inline]
fn emit_prefixed_line(prefix: &str, line: &str) {
    if prefix.is_empty() {
        newengine_ulog_api::ulog::debug!("{}", line);
    } else {
        newengine_ulog_api::ulog::debug!("{} {}", prefix, line);
    }
}

#[inline]
fn pad_right(value: &str, width: usize) -> String {
    let len = value.chars().count();
    if len >= width {
        value.to_owned()
    } else {
        let mut out = String::with_capacity(width);
        out.push_str(value);
        out.push_str(&" ".repeat(width - len));
        out
    }
}

fn wrap_value(value: &str, width: usize) -> Vec<String> {
    let width = width.max(8);
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return vec![String::new()];
    }

    let mut out: Vec<String> = Vec::new();
    for raw_line in trimmed.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            out.push(String::new());
            continue;
        }

        let chars: Vec<char> = line.chars().collect();
        if chars.len() <= width {
            out.push(line.to_owned());
            continue;
        }

        let mut start = 0usize;
        while start < chars.len() {
            let end = (start + width).min(chars.len());
            let slice: String = chars[start..end].iter().collect();
            out.push(slice);
            start = end;
        }
    }

    if out.is_empty() {
        out.push(String::new());
    }
    out
}
