#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SceneMeshPass {
    Forward,
    GBuffer,
}

impl SceneMeshPass {
    #[inline]
    pub(super) const fn is_gbuffer(self) -> bool {
        matches!(self, Self::GBuffer)
    }

    #[inline]
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Forward => "viewport_forward",
            Self::GBuffer => "gbuffer",
        }
    }
}

const ROUTE_DIAGNOSTIC_EARLY_FRAMES: u64 = 3;

#[inline]
fn route_diagnostics_due_for_policy(
    frame_index: u64,
    steady_enabled: bool,
    steady_interval_frames: u64,
) -> bool {
    frame_index <= ROUTE_DIAGNOSTIC_EARLY_FRAMES
        || (steady_enabled
            && steady_interval_frames > 0
            && frame_index.is_multiple_of(steady_interval_frames))
}

#[inline]
pub(super) fn route_diagnostics_due(frame_index: u64) -> bool {
    let policy = crate::runtime_policy::render_runtime_policy();
    route_diagnostics_due_for_policy(
        frame_index,
        policy.render_route_diagnostics,
        policy.render_route_diagnostic_interval_frames,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_route_diagnostics_are_opt_in() {
        assert!(route_diagnostics_due_for_policy(0, false, 240));
        assert!(route_diagnostics_due_for_policy(3, false, 240));
        assert!(!route_diagnostics_due_for_policy(240, false, 240));
        assert!(!route_diagnostics_due_for_policy(480, false, 240));
    }

    #[test]
    fn enabled_route_diagnostics_follow_the_requested_interval() {
        assert!(route_diagnostics_due_for_policy(240, true, 240));
        assert!(!route_diagnostics_due_for_policy(241, true, 240));
        assert!(!route_diagnostics_due_for_policy(240, true, 0));
    }
}
