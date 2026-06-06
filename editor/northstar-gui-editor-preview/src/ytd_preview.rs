#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YtdTextureEntry {
    pub name: String,
    pub details: String,
}

#[derive(Default)]
struct PartialEntry {
    name: Option<String>,
    width: Option<String>,
    height: Option<String>,
    format: Option<String>,
    color_space: Option<String>,
    mip_count: Option<String>,
    byte_len: Option<String>,
}

pub fn parse_ytd_inspect_entries(lines: &[String]) -> Vec<YtdTextureEntry> {
    let mut in_entries = false;
    let mut in_object = false;
    let mut current = PartialEntry::default();
    let mut out = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("\"entries\"") && trimmed.contains('[') {
            in_entries = true;
            continue;
        }
        if !in_entries {
            continue;
        }
        if trimmed.starts_with(']') {
            if in_object {
                push_entry(&mut out, &current);
            }
            break;
        }
        if trimmed.starts_with('{') {
            in_object = true;
            current = PartialEntry::default();
            continue;
        }
        if trimmed.starts_with('}') {
            if in_object {
                push_entry(&mut out, &current);
            }
            in_object = false;
            current = PartialEntry::default();
            continue;
        }
        if !in_object {
            continue;
        }
        if let Some(value) = json_string_field(trimmed, "name") {
            current.name = Some(value);
        } else if let Some(value) = json_number_field(trimmed, "width") {
            current.width = Some(value);
        } else if let Some(value) = json_number_field(trimmed, "height") {
            current.height = Some(value);
        } else if let Some(value) = json_string_field(trimmed, "format") {
            current.format = Some(value);
        } else if let Some(value) = json_string_field(trimmed, "color_space") {
            current.color_space = Some(value);
        } else if let Some(value) = json_number_field(trimmed, "mip_count") {
            current.mip_count = Some(value);
        } else if let Some(value) = json_number_field(trimmed, "byte_len") {
            current.byte_len = Some(value);
        }
    }

    out
}

fn push_entry(out: &mut Vec<YtdTextureEntry>, entry: &PartialEntry) {
    let Some(name) = entry.name.as_ref().filter(|value| !value.trim().is_empty()) else {
        return;
    };
    let width = entry.width.as_deref().unwrap_or("?");
    let height = entry.height.as_deref().unwrap_or("?");
    let format = entry.format.as_deref().unwrap_or("format?");
    let color_space = entry.color_space.as_deref().unwrap_or("space?");
    let mip_count = entry.mip_count.as_deref().unwrap_or("?");
    let byte_len = entry.byte_len.as_deref().unwrap_or("?");
    out.push(YtdTextureEntry {
        name: name.to_owned(),
        details: format!("{width}x{height}, {format}, {color_space}, mips={mip_count}, bytes={byte_len}"),
    });
}

fn json_string_field(line: &str, key: &str) -> Option<String> {
    let prefix = format!("\"{key}\":");
    let rest = line.trim().strip_prefix(&prefix)?.trim().trim_end_matches(',').trim();
    let value = rest.strip_prefix('"')?.strip_suffix('"')?;
    Some(unescape_minimal_json(value))
}

fn json_number_field(line: &str, key: &str) -> Option<String> {
    let prefix = format!("\"{key}\":");
    let rest = line.trim().strip_prefix(&prefix)?.trim().trim_end_matches(',').trim();
    if rest.chars().all(|ch| ch.is_ascii_digit()) {
        Some(rest.to_owned())
    } else {
        None
    }
}

fn unescape_minimal_json(value: &str) -> String {
    value.replace("\\\"", "\"").replace("\\\\", "\\")
}
