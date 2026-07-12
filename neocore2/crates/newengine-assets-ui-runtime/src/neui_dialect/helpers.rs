use super::*;

#[inline]
pub(super) fn normalize_name(value: &str) -> String {
    sanitize_tag(value).replace('-', "")
}

pub(crate) fn sanitize_tag(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned()
}

pub(super) fn node_kind_from_str(value: &str) -> Result<UiRuntimeNodeKind, String> {
    match normalize_name(value).as_str() {
        "surface" => Ok(UiRuntimeNodeKind::Surface),
        "panel" => Ok(UiRuntimeNodeKind::Panel),
        "stack" => Ok(UiRuntimeNodeKind::Stack),
        "row" => Ok(UiRuntimeNodeKind::Row),
        "column" => Ok(UiRuntimeNodeKind::Column),
        "grid" => Ok(UiRuntimeNodeKind::Grid),
        "text" => Ok(UiRuntimeNodeKind::Text),
        "button" => Ok(UiRuntimeNodeKind::Button),
        "action" => Ok(UiRuntimeNodeKind::Action),
        "input" => Ok(UiRuntimeNodeKind::Input),
        "checkbox" => Ok(UiRuntimeNodeKind::Checkbox),
        "toggle" => Ok(UiRuntimeNodeKind::Toggle),
        "slider" => Ok(UiRuntimeNodeKind::Slider),
        "scrollbar" => Ok(UiRuntimeNodeKind::ScrollBar),
        "select" => Ok(UiRuntimeNodeKind::Select),
        "separator" => Ok(UiRuntimeNodeKind::Separator),
        "list" => Ok(UiRuntimeNodeKind::List),
        "tree" => Ok(UiRuntimeNodeKind::Tree),
        "split" => Ok(UiRuntimeNodeKind::Split),
        "viewport" => Ok(UiRuntimeNodeKind::Viewport),
        "externaltexture" => Ok(UiRuntimeNodeKind::ExternalTexture),
        "spacer" => Ok(UiRuntimeNodeKind::Spacer),
        other => Err(format!("unknown UiRuntimeNodeKind '{other}'")),
    }
}

pub(super) fn split_tokens(value: &str) -> Vec<String> {
    value
        .split(|ch: char| ch == ',' || ch == ';' || ch.is_whitespace())
        .map(str::trim)
        .filter(|it| !it.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(super) fn attr_bool(open: &str, key: &str) -> Option<bool> {
    let value = attr_value(open, key)?;
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

pub(super) fn push_unique(values: &mut Vec<String>, value: String) {
    if !value.is_empty() && !values.iter().any(|it| it == &value) {
        values.push(value);
    }
}
