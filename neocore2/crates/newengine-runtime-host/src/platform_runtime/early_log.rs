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

    let mut wrote = false;
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
                wrote = true;
            }
            Err(_) => continue,
        }
    }

    if !wrote {
        eprint!("{line}");
    }
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(path) = std::env::var_os("NEWENGINE_PLATFORM_EARLY_LOG") {
        paths.push(PathBuf::from(path));
    }

    if let Some(cache) = std::env::var_os("NEWENGINE_CACHE_FILES")
        .or_else(|| std::env::var_os("CACHE_FILES"))
        .filter(|v| !v.as_os_str().is_empty())
    {
        paths.push(PathBuf::from(cache).join("logs").join("platform-host-early.log"));
    }

    paths.push(find_neocore2_root().join("cache").join("logs").join("platform-host-early.log"));
    dedup_paths(paths)
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
fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in paths {
        if !out.iter().any(|p: &PathBuf| p == &path) {
            out.push(path);
        }
    }
    out
}

#[macro_export]
macro_rules! platform_early_log {
    ($($arg:tt)*) => {{
        $crate::platform_runtime::early_log::write(format_args!($($arg)*));
    }};
}
