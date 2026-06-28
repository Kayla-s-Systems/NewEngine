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
}
