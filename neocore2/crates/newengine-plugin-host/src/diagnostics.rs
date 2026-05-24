#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use newengine_loading_api::{EngineTaskEvent, EngineTaskPhase, ENGINE_TASK_EVENT_TOPIC_V1};

pub(crate) const TOPIC_JOB_BEGIN: &str = "newengine.diagnostics.job.begin.v1";
pub(crate) const TOPIC_JOB_END: &str = "newengine.diagnostics.job.end.v1";

static HOST_JOB_SEQ: AtomicU64 = AtomicU64::new(1);

#[inline]
pub(crate) fn next_job_id(prefix: &str) -> String {
    format!("{}.{}", prefix, HOST_JOB_SEQ.fetch_add(1, Ordering::Relaxed))
}

#[inline]
pub(crate) fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

#[inline]
pub(crate) fn emit_json(topic: &str, value: serde_json::Value) {
    let Ok(bytes) = serde_json::to_vec(&value) else {
        return;
    };
    let _ = crate::host_context::publish_event(topic, &bytes);
}

#[inline]
pub(crate) fn begin(value: serde_json::Value) {
    emit_json(TOPIC_JOB_BEGIN, value.clone());
    emit_task_from_diagnostic(&value, EngineTaskPhase::Running, None);
}

#[inline]
pub(crate) fn end(value: serde_json::Value) {
    emit_json(TOPIC_JOB_END, value.clone());
    let phase = match value.get("status").and_then(|v| v.as_str()).unwrap_or_default() {
        "completed" => EngineTaskPhase::Completed,
        "cancelled" | "canceled" => EngineTaskPhase::Cancelled,
        _ => EngineTaskPhase::Failed,
    };
    emit_task_from_diagnostic(&value, phase, Some(1.0));
}

fn emit_task_from_diagnostic(value: &serde_json::Value, phase: EngineTaskPhase, progress_01: Option<f32>) {
    let task_id = str_field(value, "id", "host.task.unknown");
    let name = str_field(value, "name", task_id);
    let category = str_field(value, "category", "host-task");
    let source = str_field(value, "source", "newengine-plugin-host");
    let detail = str_field(value, "detail", phase.state_label());
    let owner = value
        .get("owner_plugin_id")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("plugin_id").and_then(|v| v.as_str()))
        .or_else(|| value.get("service_id").and_then(|v| v.as_str()))
        .or_else(|| value.get("metadata").and_then(|m| m.get("owner_plugin_id")).and_then(|v| v.as_str()))
        .or_else(|| value.get("metadata").and_then(|m| m.get("plugin_id")).and_then(|v| v.as_str()))
        .or_else(|| value.get("metadata").and_then(|m| m.get("service_id")).and_then(|v| v.as_str()))
        .unwrap_or("newengine-plugin-host");

    let mut event = EngineTaskEvent::new(
        task_id,
        source,
        owner,
        category,
        name,
        category,
        phase,
        phase.state_label(),
        detail,
    )
    .with_controls(false, false);

    if let Some(progress) = progress_01 {
        event = event.with_progress(progress);
    }

    if let Ok(bytes) = serde_json::to_vec(&event) {
        let _ = crate::host_context::publish_event(ENGINE_TASK_EVENT_TOPIC_V1, &bytes);
    }
}

#[inline]
fn str_field<'a>(value: &'a serde_json::Value, key: &str, fallback: &'a str) -> &'a str {
    value.get(key).and_then(|v| v.as_str()).unwrap_or(fallback)
}
