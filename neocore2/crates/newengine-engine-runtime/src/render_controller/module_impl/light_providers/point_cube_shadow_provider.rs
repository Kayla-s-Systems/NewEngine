#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::EngineResult;
use newengine_lighting::ShadowMethod;

use super::super::light_extraction::{LightExtractionCtx, LightExtractionProvider};
use super::super::{lights, shadows};

pub(super) struct PointCubeShadowProvider;

impl LightExtractionProvider for PointCubeShadowProvider {
    #[inline]
    fn id(&self) -> &'static str {
        "runtime.point_cube_shadow"
    }

    #[inline]
    fn supports(&self, ctx: &LightExtractionCtx<'_>) -> bool {
        matches!(ctx.settings.method, ShadowMethod::Auto | ShadowMethod::PointCubeMap)
            && lights::primary_point_light(ctx.world).is_some()
    }

    #[inline]
    fn extract(&self, ctx: &mut LightExtractionCtx<'_>) -> EngineResult<Option<shadows::LightShadowPlan>> {
        shadows::warn_unsupported_point_shadow_once(&mut *ctx.controller);
        shadows::retire_shadow_rt(&mut *ctx.controller);
        Ok(Some(shadows::LightShadowPlan::unsupported(
            shadows::ShadowLightKind::Point,
            ctx.lit.white_texture,
            ctx.settings.resolution,
        )))
    }
}
