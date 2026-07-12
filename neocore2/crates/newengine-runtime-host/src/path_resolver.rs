use std::path::PathBuf;

/// Resolves the active `neocore2` root from the process CWD or executable path.
///
/// Startup logging, launcher bootstrapping and future tool hosts must share this
/// exact policy so paths do not diverge depending on which subsystem starts first.
pub(crate) fn find_neocore2_root() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        if is_neocore2_dir(&cwd) {
            return cwd;
        }
        let nested = cwd.join("NewEngine").join("neocore2");
        if nested.exists() {
            return nested;
        }
        if let Some(root) = cwd.ancestors().find(|path| is_neocore2_dir(path)) {
            return root.to_path_buf();
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(root) = exe.ancestors().find(|path| is_neocore2_dir(path)) {
            return root.to_path_buf();
        }
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[inline]
fn is_neocore2_dir(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("neocore2"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_name_match_is_case_insensitive() {
        assert!(is_neocore2_dir(std::path::Path::new("C:/repo/NeoCore2")));
        assert!(!is_neocore2_dir(std::path::Path::new("C:/repo/NewEngine")));
    }
}
