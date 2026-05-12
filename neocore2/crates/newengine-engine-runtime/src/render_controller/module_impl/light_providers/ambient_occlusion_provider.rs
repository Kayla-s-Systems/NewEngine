#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::EngineResult;

use super::super::light_extraction::{LightExtractionCtx, LightExtractionProvider};
use super::super::shadows;

pub(super) struct AmbientOcclusionProvider;

impl LightExtractionProvider for AmbientOcclusionProvider {
    #[inline]
    fn id(&self) -> &'static str {
        "runtime.ambient_occlusion"
    }

    #[inline]
    fn supports(&self, _ctx: &LightExtractionCtx<'_>) -> bool {
        // Ambient light has no direction/position, so it never emits a direct shadow map.
        // This provider owns the future AO/IBL/probe occlusion extension point.
        false
    }

    #[inline]
    fn extract(&self, _ctx: &mut LightExtractionCtx<'_>) -> EngineResult<Option<shadows::LightShadowPlan>> {
        Ok(None)
    }
}
