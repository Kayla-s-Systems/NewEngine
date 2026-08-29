#![forbid(unsafe_op_in_unsafe_fn)]

/// Lightweight runtime trace policy.
///
/// The first frames after launch are always traced because they validate the
/// loading gate, first shadow map, first frame graph and first playable frame.
/// Steady-state tracing is deliberately sparse: per-pass diagnostics allocate
/// strings, walk draw-list stats and can distort the very frame timing that we
/// are trying to measure.
pub(super) const STARTUP_TRACE_FRAMES: u64 = 8;
pub(super) const STEADY_TRACE_INTERVAL_FRAMES: u64 = 600;

#[inline]
pub(super) fn should_trace_frame(frame_index: u64) -> bool {
    frame_index < STARTUP_TRACE_FRAMES
        || (STEADY_TRACE_INTERVAL_FRAMES > 0
            && frame_index.is_multiple_of(STEADY_TRACE_INTERVAL_FRAMES))
}
