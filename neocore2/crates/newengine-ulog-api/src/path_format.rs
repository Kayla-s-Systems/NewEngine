//! Shared stable path formatting for diagnostics.

use std::path::{Path, PathBuf};

/// Best-effort canonicalization for nicer, stable logs.
///
/// - If the path exists, tries `std::fs::canonicalize` to remove `..` and `.` segments.
/// - If it does not exist (or canonicalization fails), returns the original path.
#[inline]
pub fn canonicalize_if_exists(p: &Path) -> PathBuf {
    if p.exists() {
        std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
    } else {
        p.to_path_buf()
    }
}

/// Formats a path for logs as a stable, Windows-friendly string.
///
/// - Strips the Windows verbatim prefix (`\\?\`) if present.
/// - Also strips its slash-normalized form (`//?/`) if present.
/// - Uses forward slashes (`/`) for readability/stability in logs.
#[inline]
pub fn display_clean(p: &Path) -> String {
    let s = p.to_string_lossy();

    // Note: some code paths may already have normalized separators before logging.
    // Handle both representations to avoid leaking the prefix into logs.
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    let s = s.strip_prefix("//?/").unwrap_or(s);

    s.replace('\\', "/")
}
