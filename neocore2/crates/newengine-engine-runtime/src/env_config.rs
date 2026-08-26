#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-instance environment compatibility access.
//!
//! The launcher may translate CLI/process environment into the HostContext snapshot, but
//! reusable runtime code never consults process-global environment after construction.

#[inline]
pub(crate) fn var(name: &str) -> Option<String> {
    newengine_plugin_host::current_host_context().environment_var(name)
}

#[inline]
pub(crate) fn var_os(name: &str) -> Option<std::ffi::OsString> {
    newengine_plugin_host::current_host_context().environment_var_os(name)
}

#[inline]
pub(crate) fn var_bool(name: &str, default: bool) -> bool {
    var(name)
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            !matches!(v.as_str(), "" | "0" | "false" | "off" | "no")
        })
        .unwrap_or(default)
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
