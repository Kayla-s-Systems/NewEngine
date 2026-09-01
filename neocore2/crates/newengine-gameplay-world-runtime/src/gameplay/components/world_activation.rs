/// Provider-neutral world activation lifecycle. The engine owns the state machine;
/// application/profile code owns the policy that decides when authored content is ready.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldActivationPhase {
    Preparing,
    Ready,
    Preview,
    Active,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidencyProgress {
    pub waiting: u32,
    pub total: u32,
    pub failed: u32,
}

impl ResidencyProgress {
    #[inline]
    pub const fn ready(self) -> u32 {
        self.total.saturating_sub(self.waiting)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldActivationState {
    pub requested_frame: u64,
    /// Process-monotonic origin for the soft activation timeout.
    pub requested_at_ms: u64,
    pub ready_frame: Option<u64>,
    pub phase: WorldActivationPhase,
    pub reason: String,
    pub residency: ResidencyProgress,
}

impl WorldActivationState {
    #[inline]
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            requested_frame: u64::MAX,
            requested_at_ms: 0,
            ready_frame: None,
            phase: WorldActivationPhase::Preparing,
            reason: reason.into(),
            residency: ResidencyProgress::default(),
        }
    }

    #[inline]
    pub const fn is_ready(&self) -> bool {
        matches!(
            self.phase,
            WorldActivationPhase::Ready
                | WorldActivationPhase::Preview
                | WorldActivationPhase::Active
        )
    }

    #[inline]
    pub const fn is_active(&self) -> bool {
        matches!(self.phase, WorldActivationPhase::Active)
    }

    #[inline]
    pub const fn is_preview_ready(&self) -> bool {
        matches!(self.phase, WorldActivationPhase::Preview)
    }

    #[inline]
    pub const fn needs_prelaunch_gate(&self) -> bool {
        matches!(
            self.phase,
            WorldActivationPhase::Preparing | WorldActivationPhase::Ready
        )
    }

    #[inline]
    pub fn mark_ready(&mut self, frame: u64, reason: impl Into<String>) {
        self.mark_ready_phase(frame, WorldActivationPhase::Ready, reason);
    }

    #[inline]
    pub fn update_residency(&mut self, waiting: u32, total: u32, failed: u32) {
        self.residency = ResidencyProgress {
            waiting,
            total,
            failed,
        };
    }

    #[inline]
    pub fn mark_preview_ready(&mut self, frame: u64, reason: impl Into<String>) {
        self.mark_ready_phase(frame, WorldActivationPhase::Preview, reason);
    }

    fn mark_ready_phase(
        &mut self,
        frame: u64,
        phase: WorldActivationPhase,
        reason: impl Into<String>,
    ) {
        self.requested_frame = self.requested_frame.min(frame);
        self.ready_frame = Some(frame);
        self.phase = phase;
        self.reason = reason.into();
        self.residency.waiting = 0;
    }

    #[inline]
    pub fn mark_active(&mut self) {
        self.phase = WorldActivationPhase::Active;
    }

    #[inline]
    pub fn mark_failed(&mut self, reason: impl Into<String>) {
        self.phase = WorldActivationPhase::Failed;
        self.reason = reason.into();
    }
}
