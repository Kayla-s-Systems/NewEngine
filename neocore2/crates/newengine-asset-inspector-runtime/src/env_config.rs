#![forbid(unsafe_op_in_unsafe_fn)]

#[inline]
pub(crate) fn normalized_logical_ref(name: &str) -> Option<String> {
    newengine_plugin_host::current_host_context()
        .environment_var(name)
        .map(|value| value.trim().replace('\\', "/"))
        .filter(|value| !value.is_empty())
}
