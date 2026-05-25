#![forbid(unsafe_op_in_unsafe_fn)]

use std::{
    io::Write,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use newengine_jobs_api::{EngineJobEventV1, EngineTaskEvent, EngineTaskPhase, JobExecutorKind};

const DEFAULT_SHUTDOWN_WATCHDOG_MS: u64 = 4_000;
const MIN_SHUTDOWN_WATCHDOG_MS: u64 = 250;
const MAX_SHUTDOWN_WATCHDOG_MS: u64 = 60_000;

static WATCHDOG_JOB_SEQ: AtomicU64 = AtomicU64::new(1);

/// Last-resort process-exit guard for native platform close.
///
/// Normal close remains fully graceful: the host still shuts down modules,
/// services and plugin providers on the main thread. The watchdog only fires if
/// a provider/module teardown path blocks forever after the native window has
/// already disappeared. This keeps user-facing close deterministic while still
/// preserving a strict/shutdown-debug path through environment overrides.
pub(crate) struct ShutdownWatchdog {
    completed: Arc<AtomicBool>,
    task_id: Option<String>,
}

impl ShutdownWatchdog {
    pub(crate) fn arm(origin: &'static str, exit_code: i32) -> Self {
        let completed = Arc::new(AtomicBool::new(false));
        let Some(timeout_ms) = configured_timeout_ms() else {
            completed.store(true, Ordering::Release);
            return Self { completed, task_id: None };
        };

        let task_id = format!(
            "runtime.shutdown_watchdog.{}",
            WATCHDOG_JOB_SEQ.fetch_add(1, Ordering::Relaxed)
        );
        publish_watchdog_event(
            task_id.as_str(),
            EngineTaskPhase::Scheduled,
            "Shutdown watchdog armed",
            format!("origin='{origin}' timeout_ms={timeout_ms} exit_code={exit_code}"),
            Some(0.0),
        );

        let completed_for_thread = Arc::clone(&completed);
        let task_id_for_thread = task_id.clone();
        // no-hidden-thread-scan: allowed engine.jobs-visible shutdown watchdog; it publishes a JobId before sleeping and terminal state before forced exit.
        let _ = std::thread::Builder::new()
            .name("newengine.shutdown.watchdog".to_owned())
            .spawn(move || {
                publish_watchdog_event(
                    task_id_for_thread.as_str(),
                    EngineTaskPhase::Running,
                    "Shutdown watchdog running",
                    "Waiting for graceful shutdown completion.",
                    None,
                );
                std::thread::sleep(Duration::from_millis(timeout_ms));
                if completed_for_thread.load(Ordering::Acquire) {
                    return;
                }

                publish_watchdog_event(
                    task_id_for_thread.as_str(),
                    EngineTaskPhase::Failed,
                    "Shutdown watchdog forced exit",
                    format!("Forced process exit after {timeout_ms}ms origin='{origin}' exit_code={exit_code}"),
                    Some(1.0),
                );
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

        Self { completed, task_id: Some(task_id) }
    }

    #[inline]
    pub(crate) fn complete(&self) {
        if self.completed.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Some(task_id) = self.task_id.as_deref() {
            log::warn!(
                "platform runtime: stopping shutdown watchdog thread outside engine.jobs task_id={}",
                task_id
            );
            eprintln!(
                "[WARN] platform runtime: stopping shutdown watchdog thread outside engine.jobs task_id={}",
                task_id
            );
            publish_watchdog_event(
                task_id,
                EngineTaskPhase::Completed,
                "Shutdown watchdog disarmed",
                "Graceful shutdown completed before watchdog timeout.",
                Some(1.0),
            );
        }
    }
}

impl Drop for ShutdownWatchdog {
    fn drop(&mut self) {
        self.complete();
    }
}

fn publish_watchdog_event(
    task_id: &str,
    phase: EngineTaskPhase,
    status: impl Into<String>,
    detail: impl Into<String>,
    progress_01: Option<f32>,
) {
    let mut event = EngineTaskEvent::new(
        task_id,
        "newengine-runtime-host.shutdown-watchdog",
        "newengine-runtime-host",
        "runtime-watchdog",
        "shutdown-watchdog",
        "background",
        phase,
        status,
        detail,
    )
    .with_controls(false, false);
    if let Some(progress) = progress_01 {
        event = event.with_progress(progress);
    }
    let job_event = EngineJobEventV1::new(event.clone(), JobExecutorKind::RuntimeWatchdog, "shutdown-watchdog");
    if let Ok(bytes) = serde_json::to_vec(&event) {
        let _ = newengine_plugin_host::host_context::publish_event(newengine_jobs_api::ENGINE_TASK_EVENT_TOPIC_V1, &bytes);
    }
    if let Ok(bytes) = serde_json::to_vec(&job_event) {
        let _ = newengine_plugin_host::host_context::publish_event(newengine_jobs_api::ENGINE_JOB_EVENT_TOPIC_V1, &bytes);
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
