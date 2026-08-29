use std::sync::Arc;

use newengine_core::EngineResult;
use newengine_lighting::ShadowMethod;
use newengine_render_feature_api::{
    LightExtractionCommand, LightExtractionCtx, LightExtractionProvider,
    LightExtractionProviderMetadata, ShadowLightKind,
};

use crate::{
    STANDARD_AMBIENT_OCCLUSION_PROVIDER_ID, STANDARD_DIRECTIONAL_SHADOW_PROVIDER_ID,
    STANDARD_POINT_CUBE_SHADOW_PROVIDER_ID, STANDARD_SPOT_SHADOW_PROVIDER_ID,
};

pub(crate) fn providers() -> Vec<Arc<dyn LightExtractionProvider>> {
    vec![
        Arc::new(StandardDirectionalShadowProvider),
        Arc::new(StandardPointCubeShadowProvider),
        Arc::new(StandardSpotShadowProvider),
        Arc::new(StandardAmbientOcclusionProvider),
    ]
}

#[inline]
fn feature_metadata(
    id: &'static str,
    description: &'static str,
) -> LightExtractionProviderMetadata {
    LightExtractionProviderMetadata::feature(id, description)
}

#[inline]
fn unsupported(kind: ShadowLightKind) -> EngineResult<Option<LightExtractionCommand>> {
    Ok(Some(LightExtractionCommand::Unsupported(kind)))
}

struct StandardDirectionalShadowProvider;

impl LightExtractionProvider for StandardDirectionalShadowProvider {
    #[inline]
    fn id(&self) -> &'static str {
        STANDARD_DIRECTIONAL_SHADOW_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> LightExtractionProviderMetadata {
        feature_metadata(self.id(), "Standard directional shadow planning")
    }

    #[inline]
    fn supports(&self, ctx: &LightExtractionCtx<'_>) -> bool {
        matches!(
            ctx.settings.method,
            ShadowMethod::DirectionalDepthMap | ShadowMethod::CascadedShadowMaps
        ) && ctx.lights.has_directional_light()
    }

    #[inline]
    fn extract(
        &self,
        _ctx: &LightExtractionCtx<'_>,
    ) -> EngineResult<Option<LightExtractionCommand>> {
        Ok(Some(LightExtractionCommand::DirectionalShadow))
    }
}

struct StandardPointCubeShadowProvider;

impl LightExtractionProvider for StandardPointCubeShadowProvider {
    #[inline]
    fn id(&self) -> &'static str {
        STANDARD_POINT_CUBE_SHADOW_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> LightExtractionProviderMetadata {
        feature_metadata(self.id(), "Standard point shadow planning")
    }

    #[inline]
    fn supports(&self, ctx: &LightExtractionCtx<'_>) -> bool {
        matches!(ctx.settings.method, ShadowMethod::PointCubeMap)
            && ctx.lights.primary_point_light().is_some()
    }

    #[inline]
    fn extract(
        &self,
        _ctx: &LightExtractionCtx<'_>,
    ) -> EngineResult<Option<LightExtractionCommand>> {
        unsupported(ShadowLightKind::Point)
    }
}

struct StandardSpotShadowProvider;

impl LightExtractionProvider for StandardSpotShadowProvider {
    #[inline]
    fn id(&self) -> &'static str {
        STANDARD_SPOT_SHADOW_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> LightExtractionProviderMetadata {
        feature_metadata(self.id(), "Standard spot shadow planning")
    }

    #[inline]
    fn supports(&self, ctx: &LightExtractionCtx<'_>) -> bool {
        matches!(ctx.settings.method, ShadowMethod::SpotDepthMap)
    }

    #[inline]
    fn extract(
        &self,
        _ctx: &LightExtractionCtx<'_>,
    ) -> EngineResult<Option<LightExtractionCommand>> {
        unsupported(ShadowLightKind::Spot)
    }
}

struct StandardAmbientOcclusionProvider;

impl LightExtractionProvider for StandardAmbientOcclusionProvider {
    #[inline]
    fn id(&self) -> &'static str {
        STANDARD_AMBIENT_OCCLUSION_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> LightExtractionProviderMetadata {
        feature_metadata(self.id(), "Standard ambient occlusion planning")
    }

    #[inline]
    fn supports(&self, _ctx: &LightExtractionCtx<'_>) -> bool {
        false
    }

    #[inline]
    fn extract(
        &self,
        _ctx: &LightExtractionCtx<'_>,
    ) -> EngineResult<Option<LightExtractionCommand>> {
        Ok(None)
    }
}
