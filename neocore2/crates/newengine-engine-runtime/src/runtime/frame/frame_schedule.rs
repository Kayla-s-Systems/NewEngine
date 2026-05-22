#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeFramePhase {
    InputCollect,
    BuildDefinitionDtos,
    ResolveAssetGraphs,
    ProviderCalls,
    EcsApply,
    RenderExtract,
    UiUpdate,
    Diagnostics,
}

#[derive(Clone, Debug)]
pub struct RuntimeFrameSchedule {
    pub phases: &'static [RuntimeFramePhase],
}

impl Default for RuntimeFrameSchedule {
    fn default() -> Self {
        Self {
            phases: &[
                RuntimeFramePhase::InputCollect,
                RuntimeFramePhase::BuildDefinitionDtos,
                RuntimeFramePhase::ResolveAssetGraphs,
                RuntimeFramePhase::ProviderCalls,
                RuntimeFramePhase::EcsApply,
                RuntimeFramePhase::RenderExtract,
                RuntimeFramePhase::UiUpdate,
                RuntimeFramePhase::Diagnostics,
            ],
        }
    }
}
