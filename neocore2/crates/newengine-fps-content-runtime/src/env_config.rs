#![forbid(unsafe_op_in_unsafe_fn)]

#[inline]
pub(crate) fn var(name: &str) -> Option<String> {
    newengine_plugin_host::current_host_context().environment_var(name)
}

#[inline]
pub(crate) fn var_i32(name: &str, default: i32, min: i32, max: i32) -> i32 {
    var(name)
        .and_then(|v| v.trim().parse::<i32>().ok())
        .map(|v| v.clamp(min, max))
        .unwrap_or(default)
}
