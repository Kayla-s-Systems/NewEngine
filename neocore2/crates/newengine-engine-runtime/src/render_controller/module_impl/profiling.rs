#![forbid(unsafe_op_in_unsafe_fn)]

use std::time::Instant;

const PROFILER_SAMPLE_TOPIC: &str = "newengine.diagnostics.profiler.sample.v1";

/// Small frame-local timing collector used by render orchestration.
///
/// It is intentionally allocation-light and domain-agnostic: callers decide the
/// semantic labels, this type only records elapsed milliseconds and formats a
/// compact diagnostic suffix.
#[derive(Debug)]
pub(super) struct FrameCpuProfile {
    started: Instant,
    last_mark: Instant,
    parts: Vec<(&'static str, f32)>,
}

impl FrameCpuProfile {
    #[inline]
    pub(super) fn new() -> Self {
        let now = Instant::now();
        Self {
            started: now,
            last_mark: now,
            parts: Vec::with_capacity(8),
        }
    }

    #[inline]
    pub(super) fn mark(&mut self, label: &'static str) {
        let now = Instant::now();
        self.parts.push((
            label,
            now.duration_since(self.last_mark).as_secs_f32() * 1000.0,
        ));
        self.last_mark = now;
    }

    #[inline]
    pub(super) fn total_ms(&self) -> f32 {
        self.started.elapsed().as_secs_f32() * 1000.0
    }

    pub(super) fn breakdown(&self) -> String {
        self.parts
            .iter()
            .map(|(label, ms)| format!("{}={:.2}ms", label, ms))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug)]
pub(super) struct TimedBreakdown {
    started: Instant,
    parts: Vec<(String, f32)>,
}

impl TimedBreakdown {
    #[inline]
    pub(super) fn new() -> Self {
        Self {
            started: Instant::now(),
            parts: Vec::new(),
        }
    }

    pub(super) fn time<T, E>(
        &mut self,
        label: impl Into<String>,
        work: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let started = Instant::now();
        let result = work();
        self.parts
            .push((label.into(), started.elapsed().as_secs_f32() * 1000.0));
        result
    }

    #[inline]
    pub(super) fn total_ms(&self) -> f32 {
        self.started.elapsed().as_secs_f32() * 1000.0
    }

    pub(super) fn breakdown(&self) -> String {
        self.parts
            .iter()
            .map(|(label, ms)| format!("{}={:.2}ms", label, ms))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[inline]
fn env_f32(name: &str, default: f32, min: f32, max: f32) -> f32 {
    crate::env_config::var_f32(name, default, min, max)
}

#[inline]
fn env_u64(name: &str, default: u64, min: u64, max: u64) -> u64 {
    crate::env_config::var_u64(name, default, min, max)
}

#[inline]
pub(super) fn trace_ms_threshold() -> f32 {
    env_f32("NEWENGINE_RENDER_TRACE_MS", 16.67, 1.0, 1000.0)
}

#[inline]
pub(super) fn warn_ms_threshold() -> f32 {
    env_f32("NEWENGINE_RENDER_WARN_MS", 16.67, 1.0, 2000.0)
}

#[inline]
pub(super) fn slow_profile_log_interval_frames() -> u64 {
    env_u64(
        "NEWENGINE_RENDER_SLOW_PROFILE_INTERVAL_FRAMES",
        120,
        1,
        6000,
    )
}

#[inline]
pub(super) fn profiler_sample_interval_frames() -> u64 {
    // Profiling must not become the thing that makes gameplay heavy. The old
    // default sampled every fourth frame and every slow frame; on the current
    // render path that meant JSON/event traffic every frame. Keep regular
    // telemetry visible, but sample it at a diagnostics cadence unless the user
    // explicitly asks for denser profiling.
    env_u64(
        "NEWENGINE_RENDER_PROFILER_SAMPLE_INTERVAL_FRAMES",
        120,
        1,
        6000,
    )
}

pub(super) fn emit_timed_profile(
    label: &'static str,
    frame_index: u64,
    trace_frame: bool,
    total_ms: f32,
    breakdown: impl AsRef<str>,
    suffix: impl AsRef<str>,
) {
    let slow = total_ms >= warn_ms_threshold();
    let traceable = trace_frame || total_ms >= trace_ms_threshold();
    let sample_interval = profiler_sample_interval_frames();
    let should_sample = trace_frame
        || frame_index.is_multiple_of(sample_interval)
        || (slow && frame_index.is_multiple_of(slow_profile_log_interval_frames()));
    if should_sample {
        emit_profiler_sample(
            label,
            frame_index,
            total_ms,
            breakdown.as_ref(),
            suffix.as_ref(),
            slow,
        );
    }

    if !traceable && !slow {
        return;
    }

    if slow && !trace_frame && !frame_index.is_multiple_of(slow_profile_log_interval_frames()) {
        return;
    }

    let include_breakdown = trace_frame || slow || newengine_ulog_api::ulog::debug_enabled();
    let line = if include_breakdown {
        let suffix = suffix.as_ref();
        if suffix.is_empty() {
            format!(
                "{}: frame={} total_ms={:.2} {}",
                label,
                frame_index,
                total_ms,
                breakdown.as_ref(),
            )
        } else {
            format!(
                "{}: frame={} total_ms={:.2} {} {}",
                label,
                frame_index,
                total_ms,
                breakdown.as_ref(),
                suffix,
            )
        }
    } else {
        format!("{}: frame={} total_ms={:.2}", label, frame_index, total_ms)
    };

    if slow {
        newengine_ulog_api::ulog::warn!("{}", line);
    } else {
        newengine_ulog_api::ulog::debug!("{}", line);
    }
}

fn emit_profiler_sample(
    label: &'static str,
    frame_index: u64,
    total_ms: f32,
    breakdown: &str,
    suffix: &str,
    slow: bool,
) {
    if !crate::env_config::var_bool("NEWENGINE_RENDER_PROFILER_SAMPLES", true) {
        return;
    }
    let frame_budget_ms = warn_ms_threshold();
    let payload = serde_json::json!({
        "schema": "newengine.diagnostics.profiler.sample.v1",
        "category": "render",
        "source": "render_controller",
        "name": label,
        "detail": suffix,
        "lane": "render-prep",
        "priority": "interactive",
        "dependency_group": format!("frame.{frame_index}.render"),
        "frame_index": frame_index,
        "elapsed_ms": total_ms,
        "budget_ms": frame_budget_ms,
        "frame_budget_ms": frame_budget_ms,
        "exceeded_frame_budget": total_ms > frame_budget_ms,
        "slow": slow,
        "breakdown": breakdown,
    });
    if let Ok(bytes) = serde_json::to_vec(&payload) {
        let _ = newengine_plugin_host::emit_plugin_event(PROFILER_SAMPLE_TOPIC, &bytes);
    }
}
