#![forbid(unsafe_op_in_unsafe_fn)]

/// Lightweight runtime trace policy.
///
/// The first frames after launch are always traced because they validate the
/// loading gate, first shadow map, first frame graph and first playable frame.
/// Steady-state tracing is deliberately sparse: per-pass diagnostics allocate
/// strings, walk draw-list stats and can distort the very frame timing that we
/// are trying to measure.
pub(super) const STARTUP_TRACE_FRAMES: u64 = 8;
#[inline]
fn should_trace_frame_with_interval(frame_index: u64, steady_interval_frames: u64) -> bool {
    frame_index < STARTUP_TRACE_FRAMES
        || (steady_interval_frames > 0 && frame_index.is_multiple_of(steady_interval_frames))
}

#[inline]
pub(super) fn should_trace_frame(frame_index: u64) -> bool {
    should_trace_frame_with_interval(
        frame_index,
        crate::runtime_policy::diagnostics_policy().render_steady_trace_interval_frames,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_state_tracing_can_be_fully_disabled() {
        assert!(should_trace_frame_with_interval(0, 0));
        assert!(should_trace_frame_with_interval(
            STARTUP_TRACE_FRAMES - 1,
            0
        ));
        assert!(!should_trace_frame_with_interval(STARTUP_TRACE_FRAMES, 0));
        assert!(!should_trace_frame_with_interval(600, 0));
    }

    #[test]
    fn explicit_interval_restores_sampled_steady_tracing() {
        assert!(should_trace_frame_with_interval(600, 600));
        assert!(!should_trace_frame_with_interval(601, 600));
    }
}
