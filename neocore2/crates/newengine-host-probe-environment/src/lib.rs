#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_host_capabilities_api::HostEnvironmentSnapshot;

/// Discover only process/OS environment identity. No hardware probing or policy.
pub fn discover() -> HostEnvironmentSnapshot {
    HostEnvironmentSnapshot {
        executable: std::env::current_exe().ok().map(display_path),
        cwd: std::env::current_dir().ok().map(display_path),
        pid: std::process::id(),
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        family: std::env::consts::FAMILY.to_owned(),
    }
}

fn display_path(path: std::path::PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_probe_returns_process_identity() {
        let value = discover();
        assert!(value.pid > 0);
        assert!(!value.os.is_empty());
        assert!(!value.arch.is_empty());
    }
}
