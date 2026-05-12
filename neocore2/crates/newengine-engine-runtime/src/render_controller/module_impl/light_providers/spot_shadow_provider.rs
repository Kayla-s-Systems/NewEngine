#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::EngineResult;
use newengine_lighting::ShadowMethod;

use super::super::light_extraction::{LightExtractionCtx, LightExtractionProvider};
use super::super::shadows;

pub(super) struct SpotShadowProvider;

impl LightExtractionProvider for SpotShadowProvider {
    #[inline]
    fn id(&self) -> &'static str {
        "runtime.spot_shadow"
    }

    #[inline]
    fn supports(&self, ctx: &LightExtractionCtx<'_>) -> bool {
        matches!(ctx.settings.method, ShadowMethod::SpotDepthMap)
    }

    #[inline]
    fn extract(&self, ctx: &mut LightExtractionCtx<'_>) -> EngineResult<Option<shadows::LightShadowPlan>> {
        shadows::warn_unsupported_spot_shadow_once(&mut *ctx.controller);
        shadows::retire_shadow_rt(&mut *ctx.controller);
        Ok(Some(shadows::LightShadowPlan::unsupported(
            shadows::ShadowLightKind::Spot,
            ctx.lit.white_texture,
            ctx.settings.resolution,
        )))
    }
}
