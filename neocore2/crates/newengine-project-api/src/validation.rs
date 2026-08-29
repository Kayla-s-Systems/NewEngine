#[inline]
pub(crate) fn optional_blank(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.trim().is_empty())
}

pub(crate) fn ensure_optional_non_blank(
    value: Option<&str>,
    error: impl FnOnce() -> String,
) -> Result<(), String> {
    if optional_blank(value) {
        Err(error())
    } else {
        Ok(())
    }
}

pub(crate) fn collect_optional_non_blank(
    errors: &mut Vec<String>,
    value: Option<&str>,
    error: impl FnOnce() -> String,
) {
    if let Err(error) = ensure_optional_non_blank(value, error) {
        errors.push(error);
    }
}
