#[inline]
pub fn backend_matches(spec: &str, active_id: &str) -> bool {
    let spec = normalize_backend_token(spec);
    let active = normalize_backend_token(active_id);

    spec.is_empty()
        || active.is_empty()
        || spec == active
        || active.contains(&spec)
        || spec.contains(&active)
}

#[inline]
fn normalize_backend_token(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}