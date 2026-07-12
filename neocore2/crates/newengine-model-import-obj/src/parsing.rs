use std::str::SplitWhitespace;

#[inline]
pub(crate) fn content_line(raw_line: &str) -> &str {
    raw_line.split('#').next().unwrap_or_default().trim()
}

#[inline]
pub(crate) fn next_f32_or(words: &mut SplitWhitespace<'_>, fallback: f32) -> f32 {
    words
        .next()
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(fallback)
}

#[inline]
pub(crate) fn indexed_f32_or(values: &[&str], index: usize, fallback: f32) -> f32 {
    values
        .get(index)
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(fallback)
}
