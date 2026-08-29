#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-instance environment compatibility access.
//!
//! Runtime values come from the active HostContext snapshot; process environment is a
//! launcher/bootstrap concern only.

#[inline]
pub(crate) fn var(name: &str) -> Option<String> {
    newengine_plugin_host::current_host_context().environment_var(name)
}

#[inline]
pub(crate) fn var_os(name: &str) -> Option<std::ffi::OsString> {
    newengine_plugin_host::current_host_context().environment_var_os(name)
}

#[inline]
pub(crate) fn var_f32(name: &str, default: f32, min: f32, max: f32) -> f32 {
    var(name)
        .and_then(|v| v.trim().parse::<f32>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}

#[inline]
pub(crate) fn var_u32(name: &str, default: u32, min: u32, max: u32) -> u32 {
    var(name)
        .and_then(|v| v.trim().parse::<u32>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}

#[inline]
pub(crate) fn var_u64(name: &str, default: u64, min: u64, max: u64) -> u64 {
    var(name)
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}

#[inline]
pub(crate) fn var_i32(name: &str, default: i32, min: i32, max: i32) -> i32 {
    var(name)
        .and_then(|v| v.trim().parse::<i32>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}

#[inline]
pub(crate) fn var_usize(name: &str, default: usize, min: usize, max: usize) -> usize {
    var(name)
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}
