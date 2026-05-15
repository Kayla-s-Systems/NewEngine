#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;

/// Formats a runtime-visible path for diagnostics.
///
/// This is intentionally shared by launchers/runtime host code so startup logs
/// do not drift across apps and do not leak Windows verbatim prefixes.
#[inline]
pub fn display_abs_path(path: &Path) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    display_path(&canonical)
}

#[inline]
pub fn display_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    let s = s.strip_prefix("//?/").unwrap_or(s);
    s.replace('\\', "/")
}
