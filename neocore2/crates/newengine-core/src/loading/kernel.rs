use super::boot_frame::{BootFrameDto, BootViewport};
use super::profile::{LoadingPhase, LoadingProfile, ResolvedLoadingAssignment};

pub struct EngineLoadingKernel {
    profile: LoadingProfile,
    active_assignment: Option<ResolvedLoadingAssignment>,
}

impl Default for EngineLoadingKernel {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl EngineLoadingKernel {
    #[inline]
    pub fn new() -> Self {
        Self::with_profile(LoadingProfile::engine_default())
    }

    #[inline]
    pub fn with_profile(profile: LoadingProfile) -> Self {
        Self {
            profile,
            active_assignment: None,
        }
    }

    #[inline]
    pub fn with_startup_config(startup: &crate::startup::StartupConfig) -> Self {
        Self::with_profile(LoadingProfile::from_startup_config(startup))
    }

    pub fn resolve_assignment(&mut self, phase: LoadingPhase) -> ResolvedLoadingAssignment {
        let assignment = ResolvedLoadingAssignment::from_profile(phase, &self.profile);
        self.active_assignment = Some(assignment.clone());
        assignment
    }

    pub fn boot_frame(&self, viewport: BootViewport) -> BootFrameDto {
        let assignment = self.active_assignment.clone().unwrap_or_else(|| {
            ResolvedLoadingAssignment::from_profile(LoadingPhase::PreStart, &self.profile)
        });

        BootFrameDto::from_status(
            assignment,
            viewport,
            self.profile.display_name.clone(),
            "PreStart loading assignment resolved.",
            "Boot-safe presenter frame is generated without engine.ui; visual refs are consumer-declared data.",
            0.05,
            0,
        )
    }
}
