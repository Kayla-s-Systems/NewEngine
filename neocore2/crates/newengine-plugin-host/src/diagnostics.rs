#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

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
    emit_json(TOPIC_JOB_BEGIN, value);
}

#[inline]
pub(crate) fn end(value: serde_json::Value) {
    emit_json(TOPIC_JOB_END, value);
}
