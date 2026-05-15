use std::fmt;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SEQ: AtomicU64 = AtomicU64::new(1);

pub(crate) fn write(args: fmt::Arguments<'_>) {
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_millis())
        .unwrap_or(0);
    let pid = std::process::id();
    let line = format!("[{unix_ms}] [pid={pid}] [seq={seq}] {args}\n");

    for path in candidate_paths() {
        if let Some(parent) = path.parent() {
            if create_dir_all(parent).is_err() {
                continue;
            }
        }
        match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(mut f) => {
                let _ = f.write_all(line.as_bytes());
                let _ = f.flush();
                return;
            }
            Err(_) => continue,
        }
    }

    eprint!("{line}");
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if std::env::var_os("NEWENGINE_CACHE_FILES_READY").is_some() {
        if let Some(cache) = std::env::var_os("NEWENGINE_CACHE_FILES")
            .or_else(|| std::env::var_os("CACHE_FILES"))
            .filter(|v| !v.as_os_str().is_empty())
        {
            paths.push(PathBuf::from(cache).join("logs").join("platform-host-early.log"));
        }
    }

    paths.push(find_neocore2_root().join("cache").join("logs").join("platform-host-early.log"));
    paths
}

fn find_neocore2_root() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.file_name().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("neocore2")) {
            return cwd;
        }
        let nested = cwd.join("NewEngine").join("neocore2");
        if nested.exists() {
            return nested;
        }
        for ancestor in cwd.ancestors() {
            if ancestor.file_name().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("neocore2")) {
                return ancestor.to_path_buf();
            }
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors() {
            if ancestor.file_name().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("neocore2")) {
                return ancestor.to_path_buf();
            }
        }
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
#[macro_export]
macro_rules! platform_early_log {
    ($($arg:tt)*) => {{
        $crate::platform_runtime::early_log::write(format_args!($($arg)*));
    }};
}
