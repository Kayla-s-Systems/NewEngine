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

#[inline]
pub(crate) const fn is_valid_transition(
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
