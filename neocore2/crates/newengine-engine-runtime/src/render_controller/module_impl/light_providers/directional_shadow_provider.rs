#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::EngineResult;
use newengine_lighting::ShadowMethod;

use super::super::light_extraction::{LightExtractionCtx, LightExtractionProvider};
use super::super::{lights, shadows};

pub(super) struct DirectionalShadowProvider;

impl LightExtractionProvider for DirectionalShadowProvider {
    #[inline]
    fn id(&self) -> &'static str {
        "runtime.directional_shadow"
    }

    #[inline]
    fn supports(&self, ctx: &LightExtractionCtx<'_>) -> bool {
        matches!(ctx.settings.method, ShadowMethod::Auto | ShadowMethod::DirectionalDepthMap)
            && lights::primary_directional_light(ctx.world).is_some()
    }

    #[inline]
    fn extract(&self, ctx: &mut LightExtractionCtx<'_>) -> EngineResult<Option<shadows::LightShadowPlan>> {
        shadows::try_build_directional_shadow_plan(
            &mut *ctx.controller,
            &mut *ctx.render,
            ctx.world,
            ctx.bounds,
            ctx.lit,
            ctx.settings,
        )
    }
}
