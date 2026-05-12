#![forbid(unsafe_op_in_unsafe_fn)]

//! Deterministic replay coordinator primitives.
//!
//! The replay layer is intentionally modeled as a small finite-state machine.
//! Runtime/editor code observes snapshots and submits explicit intents; it must
//! not mirror replay state with ad-hoc booleans such as `is_baking`,
//! `pending_cleanup` or `paused_called`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReplayCoordinatorState {
    Inactive,
    Idle,
    ClipPreview,
    ProjectPreview,
    VideoPendingBake,
    VideoBake,
    VideoBakePauseRequested,
    VideoBakePaused,
    VideoBakePendingCleanup,
    Faulted,
}

impl ReplayCoordinatorState {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inactive => "inactive",
            Self::Idle => "idle",
            Self::ClipPreview => "clip-preview",
            Self::ProjectPreview => "project-preview",
            Self::VideoPendingBake => "video-pending-bake",
            Self::VideoBake => "video-bake",
            Self::VideoBakePauseRequested => "video-bake-pause-requested",
            Self::VideoBakePaused => "video-bake-paused",
            Self::VideoBakePendingCleanup => "video-bake-pending-cleanup",
            Self::Faulted => "faulted",
        }
    }

    #[inline]
    pub const fn is_active(self) -> bool {
        !matches!(self, Self::Inactive | Self::Faulted)
    }

    #[inline]
    pub const fn is_previewing(self) -> bool {
        matches!(self, Self::ClipPreview | Self::ProjectPreview)
    }

    #[inline]
    pub const fn is_rendering_video(self) -> bool {
        matches!(
            self,
            Self::VideoPendingBake
                | Self::VideoBake
                | Self::VideoBakePauseRequested
                | Self::VideoBakePaused
        )
    }

    #[inline]
    pub const fn is_paused(self) -> bool {
        matches!(self, Self::VideoBakePaused)
    }

    #[inline]
    pub const fn is_pending_cleanup(self) -> bool {
        matches!(self, Self::VideoBakePendingCleanup)
    }

    #[inline]
    pub const fn should_block_live_simulation(self) -> bool {
        self.is_previewing() || self.is_rendering_video() || self.is_pending_cleanup()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayCoordinatorTransition {
    pub previous: ReplayCoordinatorState,
    pub next: ReplayCoordinatorState,
    pub changed: bool,
    pub valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayCoordinatorSnapshot {
    pub state: ReplayCoordinatorState,
    pub last_error: Option<String>,
    pub playback: ReplayPlaybackClockSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayCoordinatorFsm {
    state: ReplayCoordinatorState,
    last_error: Option<String>,
    playback: ReplayPlaybackClock,
}

impl Default for ReplayCoordinatorFsm {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayCoordinatorFsm {
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: ReplayCoordinatorState::Inactive,
            last_error: None,
            playback: ReplayPlaybackClock::new(),
        }
    }

    #[inline]
    pub const fn state(&self) -> ReplayCoordinatorState {
        self.state
    }

    #[inline]
    pub fn snapshot(&self) -> ReplayCoordinatorSnapshot {
        ReplayCoordinatorSnapshot {
            state: self.state,
            last_error: self.last_error.clone(),
            playback: self.playback.snapshot(),
        }
    }

    #[inline]
    pub fn playback(&self) -> &ReplayPlaybackClock {
        &self.playback
    }

    #[inline]
    pub fn playback_mut(&mut self) -> &mut ReplayPlaybackClock {
        &mut self.playback
    }

    #[inline]
    pub fn activate(&mut self) -> ReplayCoordinatorTransition {
        self.transition(ReplayCoordinatorState::Idle)
    }

    #[inline]
    pub fn deactivate(&mut self) -> ReplayCoordinatorTransition {
        if self.state.is_rendering_video() || self.state.is_previewing() {
            let _ = self.transition(ReplayCoordinatorState::VideoBakePendingCleanup);
        }
        self.playback.reset();
        self.transition(ReplayCoordinatorState::Inactive)
    }

    #[inline]
    pub fn play_clip_preview(&mut self, clip_index: u32) -> ReplayCoordinatorTransition {
        self.playback.jump_to_clip(clip_index, 0);
        self.transition(ReplayCoordinatorState::ClipPreview)
    }

    #[inline]
    pub fn play_project_preview(&mut self) -> ReplayCoordinatorTransition {
        self.transition(ReplayCoordinatorState::ProjectPreview)
    }

    #[inline]
    pub fn start_bake(&mut self) -> ReplayCoordinatorTransition {
        self.transition(ReplayCoordinatorState::VideoPendingBake)
    }

    #[inline]
    pub fn mark_bake_ready(&mut self) -> ReplayCoordinatorTransition {
        self.transition(ReplayCoordinatorState::VideoBake)
    }

    #[inline]
    pub fn request_bake_pause(&mut self) -> ReplayCoordinatorTransition {
        self.transition(ReplayCoordinatorState::VideoBakePauseRequested)
    }

    #[inline]
    pub fn commit_bake_pause(&mut self) -> ReplayCoordinatorTransition {
        self.transition(ReplayCoordinatorState::VideoBakePaused)
    }

    #[inline]
    pub fn resume_bake(&mut self) -> ReplayCoordinatorTransition {
        self.transition(ReplayCoordinatorState::VideoBake)
    }

    #[inline]
    pub fn request_cleanup(&mut self) -> ReplayCoordinatorTransition {
        self.transition(ReplayCoordinatorState::VideoBakePendingCleanup)
    }

    #[inline]
    pub fn cleanup_finished(&mut self) -> ReplayCoordinatorTransition {
        self.playback.reset();
        self.transition(ReplayCoordinatorState::Idle)
    }

    #[inline]
    pub fn fail(&mut self, error: impl Into<String>) -> ReplayCoordinatorTransition {
        self.last_error = Some(error.into());
        self.transition(ReplayCoordinatorState::Faulted)
    }

    #[inline]
    pub fn clear_error(&mut self) {
        self.last_error = None;
    }

    pub fn transition(&mut self, next: ReplayCoordinatorState) -> ReplayCoordinatorTransition {
        let previous = self.state;
        if previous == next {
            return ReplayCoordinatorTransition {
                previous,
                next,
                changed: false,
                valid: true,
            };
        }

        let valid = is_valid_transition(previous, next);
        self.state = if valid { next } else { ReplayCoordinatorState::Faulted };

        ReplayCoordinatorTransition {
            previous,
            next: self.state,
            changed: true,
            valid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayJumpTarget {
    pub clip_index: u32,
    pub clip_time_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayPlaybackClockSnapshot {
    pub frame_index: u64,
    pub clip_index: u32,
    pub clip_time_ms: u64,
    pub pending_jump: Option<ReplayJumpTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayPlaybackClock {
    frame_index: u64,
    clip_index: u32,
    clip_time_ms: u64,
    pending_jump: Option<ReplayJumpTarget>,
}

impl Default for ReplayPlaybackClock {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayPlaybackClock {
    #[inline]
    pub const fn new() -> Self {
        Self {
            frame_index: 0,
            clip_index: 0,
            clip_time_ms: 0,
            pending_jump: None,
        }
    }

    #[inline]
    pub const fn snapshot(&self) -> ReplayPlaybackClockSnapshot {
        ReplayPlaybackClockSnapshot {
            frame_index: self.frame_index,
            clip_index: self.clip_index,
            clip_time_ms: self.clip_time_ms,
            pending_jump: self.pending_jump,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    #[inline]
    pub fn jump_to_clip(&mut self, clip_index: u32, clip_time_ms: u64) {
        self.pending_jump = Some(ReplayJumpTarget {
            clip_index,
            clip_time_ms,
        });
    }

    #[inline]
    pub fn clear_pending_jump(&mut self) {
        self.pending_jump = None;
    }

    pub fn advance_fixed(&mut self, frame_duration_ms: u64) -> ReplayPlaybackClockSnapshot {
        if let Some(jump) = self.pending_jump.take() {
            self.clip_index = jump.clip_index;
            self.clip_time_ms = jump.clip_time_ms;
        } else {
            self.clip_time_ms = self.clip_time_ms.saturating_add(frame_duration_ms);
        }
        self.frame_index = self.frame_index.wrapping_add(1);
        self.snapshot()
    }
}

#[inline]
const fn is_valid_transition(
    previous: ReplayCoordinatorState,
    next: ReplayCoordinatorState,
) -> bool {
    use ReplayCoordinatorState::*;

    matches!(
        (previous, next),
        (Inactive, Idle)
            | (Inactive, Faulted)
            | (Idle, ClipPreview)
            | (Idle, ProjectPreview)
            | (Idle, VideoPendingBake)
            | (Idle, Inactive)
            | (Idle, Faulted)
            | (ClipPreview, Idle)
            | (ClipPreview, ProjectPreview)
            | (ClipPreview, VideoPendingBake)
            | (ClipPreview, VideoBakePendingCleanup)
            | (ClipPreview, Inactive)
            | (ClipPreview, Faulted)
            | (ProjectPreview, Idle)
            | (ProjectPreview, ClipPreview)
            | (ProjectPreview, VideoPendingBake)
            | (ProjectPreview, VideoBakePendingCleanup)
            | (ProjectPreview, Inactive)
            | (ProjectPreview, Faulted)
            | (VideoPendingBake, VideoBake)
            | (VideoPendingBake, Idle)
            | (VideoPendingBake, VideoBakePendingCleanup)
            | (VideoPendingBake, Faulted)
            | (VideoBake, VideoBakePauseRequested)
            | (VideoBake, VideoBakePendingCleanup)
            | (VideoBake, Idle)
            | (VideoBake, Faulted)
            | (VideoBakePauseRequested, VideoBakePaused)
            | (VideoBakePauseRequested, VideoBake)
            | (VideoBakePauseRequested, VideoBakePendingCleanup)
            | (VideoBakePauseRequested, Faulted)
            | (VideoBakePaused, VideoBake)
            | (VideoBakePaused, VideoBakePendingCleanup)
            | (VideoBakePaused, Idle)
            | (VideoBakePaused, Faulted)
            | (VideoBakePendingCleanup, Idle)
            | (VideoBakePendingCleanup, Inactive)
            | (VideoBakePendingCleanup, Faulted)
            | (Faulted, VideoBakePendingCleanup)
            | (Faulted, Inactive)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_bake_pause_resume_flow_is_valid() {
        let mut fsm = ReplayCoordinatorFsm::new();
        assert!(fsm.activate().valid);
        assert!(fsm.start_bake().valid);
        assert!(fsm.mark_bake_ready().valid);
        assert!(fsm.request_bake_pause().valid);
        assert!(fsm.commit_bake_pause().valid);
        assert!(fsm.resume_bake().valid);
        assert_eq!(fsm.state(), ReplayCoordinatorState::VideoBake);
    }

    #[test]
    fn invalid_transition_forces_faulted_state() {
        let mut fsm = ReplayCoordinatorFsm::new();
        let transition = fsm.mark_bake_ready();
        assert!(!transition.valid);
        assert_eq!(transition.next, ReplayCoordinatorState::Faulted);
    }

    #[test]
    fn playback_clock_applies_jump_before_advance() {
        let mut clock = ReplayPlaybackClock::new();
        clock.jump_to_clip(7, 1200);
        let snapshot = clock.advance_fixed(16);
        assert_eq!(snapshot.clip_index, 7);
        assert_eq!(snapshot.clip_time_ms, 1200);
        assert_eq!(snapshot.frame_index, 1);
    }
}
