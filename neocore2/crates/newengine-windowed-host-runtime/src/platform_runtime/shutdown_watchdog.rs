#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use newengine_core::{TaskLane, TaskPriority, TaskRequest, TaskTicket, ThreadPoolHandle};
use newengine_task_api::{
    EngineTaskEnvelopeV1, EngineTaskEvent, EngineTaskPhase, TaskExecutorKind,
};

const DEFAULT_SHUTDOWN_WATCHDOG_MS: u64 = 4_000;
const MIN_SHUTDOWN_WATCHDOG_MS: u64 = 250;
const MAX_SHUTDOWN_WATCHDOG_MS: u64 = 60_000;

/// Engine.jobs-visible shutdown guard for native platform close.
///
/// Earlier versions spawned an unmanaged watchdog thread from the platform
/// runtime. That made shutdown behavior invisible to diagnostics/profiler and
/// directly violated the engine.threading ownership model. This guard now publishes
/// its lifecycle through engine.threading and the task event stream. It deliberately
/// does not block a jobs worker while `Engine::shutdown()` is draining the job
/// pool; a long-running watchdog job would deadlock shutdown because the core
/// joins engine.threading before plugin service teardown.
pub(crate) struct ShutdownWatchdog {
    completed: Arc<AtomicBool>,
    task_id: Option<String>,
    ticket: Option<TaskTicket>,
}

impl ShutdownWatchdog {
    pub(crate) fn arm(jobs: ThreadPoolHandle, origin: &'static str, exit_code: i32) -> Self {
        let completed = Arc::new(AtomicBool::new(false));
        let Some(timeout_ms) = configured_timeout_ms() else {
            completed.store(true, Ordering::Release);
            return Self {
                completed,
                task_id: None,
                ticket: None,
            };
        };

        let task_id = format!(
            "runtime.shutdown_watchdog.{}",
            jobs.snapshot().submitted_jobs.saturating_add(1)
        );
        publish_watchdog_event(
            task_id.as_str(),
            EngineTaskPhase::Scheduled,
            "Shutdown guard armed",
            format!("origin='{origin}' timeout_ms={timeout_ms} exit_code={exit_code}"),
            Some(0.0),
        );

        let completed_for_job = Arc::clone(&completed);
        let task_id_for_job = task_id.clone();
        let request = TaskRequest::new("shutdown-watchdog")
            .with_task_id(task_id.clone())
            .with_source("newengine-runtime-host.shutdown-watchdog")
            .with_owner("newengine-runtime-host")
            .with_category("runtime-watchdog")
            .with_lane(TaskLane::Background)
            .with_priority(TaskPriority::Critical)
            .pausable(false)
            .cancellable(true);

        let ticket = jobs.submit_controlled(request, move |control| {
            publish_watchdog_event(
                task_id_for_job.as_str(),
                EngineTaskPhase::Running,
                "Shutdown guard registered",
                "Shutdown guard is visible through engine.threading; no unmanaged watchdog thread was created.",
                Some(0.05),
            );
            control.publish_progress(
                0.05,
                "Shutdown guard registered",
                "Shutdown guard is visible through engine.threading; no unmanaged watchdog thread was created.",
            );
            if completed_for_job.load(Ordering::Acquire) || !control.checkpoint() {
                return;
            }
            control.publish_progress(
                1.0,
                "Shutdown guard yielded",
                "Guard job yielded before engine.threading shutdown drain to avoid self-deadlock.",
            );
        });

        newengine_ulog_api::ulog::info!(
            "platform runtime: shutdown guard armed through engine.threading task_id={} origin={} timeout_ms={} exit_code={}",
            task_id,
            origin,
            timeout_ms,
            exit_code
        );

        Self {
            completed,
            task_id: Some(task_id),
            ticket: Some(ticket),
        }
    }

    #[inline]
    pub(crate) fn complete(&self) {
        if self.completed.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Some(ticket) = self.ticket.as_ref() {
            let _ = ticket.cancel();
        }
        if let Some(task_id) = self.task_id.as_deref() {
            newengine_ulog_api::ulog::info!(
                "platform runtime: shutdown guard completed through engine.threading task_id={}",
                task_id
            );
            publish_watchdog_event(
                task_id,
                EngineTaskPhase::Completed,
                "Shutdown guard completed",
                "Graceful shutdown completed before native platform return finished.",
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
    .with_controls(false, true);
    if let Some(progress) = progress_01 {
        event = event.with_progress(progress);
    }
    let job_event = EngineTaskEnvelopeV1::new(
        event.clone(),
        TaskExecutorKind::RuntimeWatchdog,
        "shutdown-watchdog",
    );
    if let Ok(bytes) = serde_json::to_vec(&event) {
        let _ = newengine_plugin_host::host_context::publish_event(
            newengine_task_api::ENGINE_TASK_EVENT_TOPIC_V1,
            &bytes,
        );
    }
    if let Ok(bytes) = serde_json::to_vec(&job_event) {
        let _ = newengine_plugin_host::host_context::publish_event(
            newengine_task_api::ENGINE_TASK_ENVELOPE_TOPIC_V1,
            &bytes,
        );
    }
}

fn configured_timeout_ms() -> Option<u64> {
    if env_flag_enabled("NEWENGINE_DISABLE_SHUTDOWN_WATCHDOG") {
        return None;
    }
    let raw = newengine_plugin_host::current_host_context()
        .environment_var("NEWENGINE_SHUTDOWN_WATCHDOG_MS");
    let value = raw
        .as_deref()
        .and_then(|it| it.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_SHUTDOWN_WATCHDOG_MS)
        .clamp(MIN_SHUTDOWN_WATCHDOG_MS, MAX_SHUTDOWN_WATCHDOG_MS);
    Some(value)
}

fn env_flag_enabled(name: &str) -> bool {
    match newengine_plugin_host::current_host_context().environment_var(name) {
        Some(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        None => false,
    }
}
