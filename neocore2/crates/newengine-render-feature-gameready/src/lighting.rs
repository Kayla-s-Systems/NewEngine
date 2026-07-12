use std::sync::Arc;

use newengine_core::EngineResult;
use newengine_lighting::ShadowMethod;
use newengine_render_feature_api::{
    LightExtractionCommand, LightExtractionCtx, LightExtractionProvider,
    LightExtractionProviderMetadata, ShadowLightKind,
};

use crate::{
    GAME_READY_AMBIENT_OCCLUSION_PROVIDER_ID, GAME_READY_DIRECTIONAL_SHADOW_PROVIDER_ID,
    GAME_READY_POINT_CUBE_SHADOW_PROVIDER_ID, GAME_READY_SPOT_SHADOW_PROVIDER_ID,
};

pub(crate) fn providers() -> Vec<Arc<dyn LightExtractionProvider>> {
    vec![
        Arc::new(GameReadyDirectionalShadowProvider),
        Arc::new(GameReadyPointCubeShadowProvider),
        Arc::new(GameReadySpotShadowProvider),
        Arc::new(GameReadyAmbientOcclusionProvider),
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

struct GameReadyDirectionalShadowProvider;

impl LightExtractionProvider for GameReadyDirectionalShadowProvider {
    #[inline]
    fn id(&self) -> &'static str {
        GAME_READY_DIRECTIONAL_SHADOW_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> LightExtractionProviderMetadata {
        feature_metadata(self.id(), "GameReady directional shadow planning")
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

struct GameReadyPointCubeShadowProvider;

impl LightExtractionProvider for GameReadyPointCubeShadowProvider {
    #[inline]
    fn id(&self) -> &'static str {
        GAME_READY_POINT_CUBE_SHADOW_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> LightExtractionProviderMetadata {
        feature_metadata(self.id(), "GameReady point shadow planning")
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

struct GameReadySpotShadowProvider;

impl LightExtractionProvider for GameReadySpotShadowProvider {
    #[inline]
    fn id(&self) -> &'static str {
        GAME_READY_SPOT_SHADOW_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> LightExtractionProviderMetadata {
        feature_metadata(self.id(), "GameReady spot shadow planning")
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

struct GameReadyAmbientOcclusionProvider;

impl LightExtractionProvider for GameReadyAmbientOcclusionProvider {
    #[inline]
    fn id(&self) -> &'static str {
        GAME_READY_AMBIENT_OCCLUSION_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> LightExtractionProviderMetadata {
        feature_metadata(self.id(), "GameReady ambient occlusion planning")
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
