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
pub(super) fn trace_ms_threshold() -> f32 {
    16.6
}

#[inline]
pub(super) fn warn_ms_threshold() -> f32 {
    33.3
}

pub(super) fn emit_timed_profile(
    label: &'static str,
    frame_index: u64,
    trace_frame: bool,
    total_ms: f32,
    breakdown: impl AsRef<str>,
    suffix: impl AsRef<str>,
) {
    if !trace_frame && total_ms < trace_ms_threshold() {
        return;
    }

    let suffix = suffix.as_ref();
    let line = if suffix.is_empty() {
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
    };

    if total_ms >= warn_ms_threshold() {
        log::warn!("{}", line);
    } else {
        log::debug!("{}", line);
    }
}
