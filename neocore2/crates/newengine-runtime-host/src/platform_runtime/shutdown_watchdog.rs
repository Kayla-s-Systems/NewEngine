#![forbid(unsafe_op_in_unsafe_fn)]

use std::{
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

const DEFAULT_SHUTDOWN_WATCHDOG_MS: u64 = 4_000;
const MIN_SHUTDOWN_WATCHDOG_MS: u64 = 250;
const MAX_SHUTDOWN_WATCHDOG_MS: u64 = 60_000;

/// Last-resort process-exit guard for native platform close.
///
/// Normal close remains fully graceful: the host still shuts down modules,
/// services and plugin providers on the main thread. The watchdog only fires if
/// a provider/module teardown path blocks forever after the native window has
/// already disappeared. This keeps user-facing close deterministic while still
/// preserving a strict/shutdown-debug path through environment overrides.
pub(crate) struct ShutdownWatchdog {
    completed: Arc<AtomicBool>,
}

impl ShutdownWatchdog {
    pub(crate) fn arm(origin: &'static str, exit_code: i32) -> Self {
        let completed = Arc::new(AtomicBool::new(false));
        let Some(timeout_ms) = configured_timeout_ms() else {
            completed.store(true, Ordering::Release);
            return Self { completed };
        };

        let completed_for_thread = Arc::clone(&completed);
        let _ = std::thread::Builder::new()
            .name("newengine.shutdown.watchdog".to_owned())
            .spawn(move || {
                std::thread::sleep(Duration::from_millis(timeout_ms));
                if completed_for_thread.load(Ordering::Acquire) {
                    return;
                }

                let _ = writeln!(
                    std::io::stderr(),
                    "NewEngine shutdown watchdog: forced process exit after {timeout_ms}ms origin='{origin}' exit_code={exit_code}"
                );
                let _ = std::io::stderr().flush();
                std::process::exit(exit_code);
            });

        log::info!(
            "platform runtime: shutdown watchdog armed origin={} timeout_ms={} exit_code={}",
            origin,
            timeout_ms,
            exit_code
        );

        Self { completed }
    }

    #[inline]
    pub(crate) fn complete(&self) {
        self.completed.store(true, Ordering::Release);
    }
}

impl Drop for ShutdownWatchdog {
    fn drop(&mut self) {
        self.complete();
    }
}

fn configured_timeout_ms() -> Option<u64> {
    if env_flag_enabled("NEWENGINE_DISABLE_SHUTDOWN_WATCHDOG") {
        return None;
    }

    let Ok(raw) = std::env::var("NEWENGINE_SHUTDOWN_WATCHDOG_MS") else {
        return Some(DEFAULT_SHUTDOWN_WATCHDOG_MS);
    };

    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("off")
        || trimmed.eq_ignore_ascii_case("false")
        || trimmed == "0"
    {
        return None;
    }

    match trimmed.parse::<u64>() {
        Ok(ms) => Some(ms.clamp(MIN_SHUTDOWN_WATCHDOG_MS, MAX_SHUTDOWN_WATCHDOG_MS)),
        Err(_) => Some(DEFAULT_SHUTDOWN_WATCHDOG_MS),
    }
}

fn env_flag_enabled(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}
