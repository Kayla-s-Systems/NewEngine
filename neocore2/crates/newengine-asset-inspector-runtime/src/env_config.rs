#![forbid(unsafe_op_in_unsafe_fn)]

#[inline]
pub(crate) fn normalized_logical_ref(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().replace('\\', "/"))
        .filter(|value| !value.is_empty())
}
