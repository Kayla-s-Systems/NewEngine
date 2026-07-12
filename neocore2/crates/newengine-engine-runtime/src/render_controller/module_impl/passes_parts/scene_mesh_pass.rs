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

const ROUTE_DIAGNOSTIC_EARLY_FRAMES: u64 = 32;
const ROUTE_DIAGNOSTIC_INTERVAL_FRAMES: u64 = 240;

#[inline]
pub(super) fn route_diagnostics_due(frame_index: u64) -> bool {
    frame_index <= ROUTE_DIAGNOSTIC_EARLY_FRAMES
        || frame_index.is_multiple_of(ROUTE_DIAGNOSTIC_INTERVAL_FRAMES)
}
