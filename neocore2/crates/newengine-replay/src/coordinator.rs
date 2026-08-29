use serde::{Deserialize, Serialize};

use crate::state::is_valid_transition;
use crate::{
    ReplayCoordinatorState, ReplayCoordinatorTransition, ReplayPlaybackClock,
    ReplayPlaybackClockSnapshot,
};

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
        self.state = if valid {
            next
        } else {
            ReplayCoordinatorState::Faulted
        };

        ReplayCoordinatorTransition {
            previous,
            next: self.state,
            changed: true,
            valid,
        }
    }
}
