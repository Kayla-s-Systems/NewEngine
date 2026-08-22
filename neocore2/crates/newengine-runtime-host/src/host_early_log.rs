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
    let message = args.to_string();
    let payload = serde_json::json!({
        "schema": "northstar.ulog.event.v1",
        "timestamp_utc": format!("{}.{:03}Z", unix_ms / 1000, unix_ms % 1000),
        "level": "DEBUG",
        "event_id": "engine.runtime_host.preinit",
        "message": message,
        "source": { "kind": "engine", "name": "newengine-runtime-host" },
        "context": { "run_id": null, "session_id": null },
        "location": {
            "module": "newengine_runtime_host::host_early_log",
            "file": null,
            "line": null
        },
        "fields": { "pid": pid, "sequence": seq }
    });
    let Ok(line) = serde_json::to_string(&payload) else { return; };
    let mut wrote = false;
    for path in candidate_paths() {
        if let Some(parent) = path.parent() {
            if create_dir_all(parent).is_err() { continue; }
        }
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(f, "{line}");
            let _ = f.flush();
            wrote = true;
        }
    }
    if !wrote { eprintln!("{line}"); }
}

fn candidate_paths() -> Vec<PathBuf> {
    if let Some(path) = std::env::var_os("NEWENGINE_PLATFORM_EARLY_LOG").filter(|value| !value.is_empty()) {
        return vec![PathBuf::from(path)];
    }
    let mut paths = Vec::new();
    if let Some(cache) = std::env::var_os("NEWENGINE_CACHE_FILES")
        .or_else(|| std::env::var_os("CACHE_FILES"))
        .filter(|v| !v.as_os_str().is_empty())
    {
        paths.push(PathBuf::from(cache).join("logs").join("current.ulog.ndjson"));
    }
    paths.push(crate::path_resolver::find_neocore2_root().join("cache").join("logs").join("current.ulog.ndjson"));
    paths.sort();
    paths.dedup();
    paths
}

#[macro_export]
macro_rules! host_early_log {
    ($($arg:tt)*) => {{
        $crate::host_early_log::write(format_args!($($arg)*));
    }};
}
