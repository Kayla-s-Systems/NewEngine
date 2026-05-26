use super::*;

pub(super) fn resolve_project_folder(config_path: &Path, raw_path: &str) -> PathBuf {
    let trimmed = raw_path.trim();
    let path = if trimmed.is_empty() { PathBuf::from(".") } else { PathBuf::from(trimmed) };
    if path.is_absolute() {
        path
    } else if let Some(parent) = config_path.parent() {
        parent.join(path)
    } else {
        path
    }
}

pub(super) fn open_folder_in_shell(path: &Path) -> Result<(), String> {
    let target = if path.exists() {
        path.to_path_buf()
    } else {
        path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
    };

    // no-hidden-thread-scan: explicit user action opens the project folder in the OS shell; not runtime frame work.
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer");
        command.arg(&target);
        command
    };

    // no-hidden-thread-scan: explicit user action opens the project folder in the OS shell; not runtime frame work.
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(&target);
        command
    };

    // no-hidden-thread-scan: explicit user action opens the project folder in the OS shell; not runtime frame work.
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(&target);
        command
    };

    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        let _ = target;
        return Err("opening folders is not supported on this platform".to_owned());
    }

    command
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("failed to spawn platform folder opener: {err}"))
}
