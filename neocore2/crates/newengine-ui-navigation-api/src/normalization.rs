pub(crate) fn normalize_optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

pub(crate) fn normalize_non_empty_strings(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter_map(|value| normalize_optional_string(&value))
        .collect()
}
