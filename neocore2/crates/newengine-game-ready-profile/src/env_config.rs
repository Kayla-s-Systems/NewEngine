#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;

#[inline]
pub(crate) fn var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

#[inline]
pub(crate) fn path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

#[inline]
pub(crate) fn var_u32(name: &str, default: u32, min: u32, max: u32) -> u32 {
    var(name)
        .and_then(|value| value.trim().parse::<u32>().ok())
        .map(|value| value.clamp(min, max))
        .unwrap_or(default)
}
