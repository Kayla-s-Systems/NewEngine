#![forbid(unsafe_op_in_unsafe_fn)]

use std::time::Instant;

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
    env_f32("NEWENGINE_RENDER_TRACE_MS", 16.6, 1.0, 1000.0)
}

#[inline]
pub(super) fn warn_ms_threshold() -> f32 {
    env_f32("NEWENGINE_RENDER_WARN_MS", 33.3, 1.0, 2000.0)
}

#[inline]
pub(super) fn slow_profile_log_interval_frames() -> u64 {
    env_u64("NEWENGINE_RENDER_SLOW_PROFILE_INTERVAL_FRAMES", 60, 1, 6000)
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
    if !traceable && !slow {
        return;
    }

    if slow && !trace_frame && frame_index % slow_profile_log_interval_frames() != 0 {
        return;
    }

    let include_breakdown = trace_frame || log::log_enabled!(log::Level::Debug);
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
        log::warn!("{}", line);
    } else {
        log::debug!("{}", line);
    }
}
