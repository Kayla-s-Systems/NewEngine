#![forbid(unsafe_op_in_unsafe_fn)]

//! GameReady profile-owned render feature pack.
//!
//! This crate is not a renderer backend and does not depend on a concrete
//! runtime controller. It implements provider traits from
//! `newengine-render-feature-api`; the active profile composes these providers
//! into whatever runtime owns the render feature registries.

use std::sync::Arc;

use newengine_core::render::RenderDrawListKind;
use newengine_core::EngineResult;
use newengine_lighting::ShadowMethod;
use newengine_material_domain_api::{MaterialGpuPipelineKey, MaterialGpuPipelineProvider};
use newengine_material_domain_gameready::{
    GameReadyLitMaterialDomainProvider, GAME_READY_LIT_PIPELINE_KEY,
};
use newengine_render_feature_api::{
    shadow_and_opaque_list, DrawListBuildCtx, LightExtractionCommand, LightExtractionCtx,
    LightExtractionProvider,
    LightExtractionProviderMetadata, RenderDrawListProvider, RenderDrawListProviderMetadata,
    SceneExtractionCtx, ShadowLightKind,
};

pub const GAME_READY_TERRAIN_PROVIDER_ID: &str = "gameready.terrain";
pub const GAME_READY_PRIMITIVE_MESH_PROVIDER_ID: &str = "gameready.primitive_mesh";
pub const GAME_READY_DIRECTIONAL_SHADOW_PROVIDER_ID: &str = "gameready.directional_shadow";
pub const GAME_READY_POINT_CUBE_SHADOW_PROVIDER_ID: &str = "gameready.point_cube_shadow";
pub const GAME_READY_SPOT_SHADOW_PROVIDER_ID: &str = "gameready.spot_shadow";
pub const GAME_READY_AMBIENT_OCCLUSION_PROVIDER_ID: &str = "gameready.ambient_occlusion";

#[derive(Default)]
pub struct GameReadyRenderFeaturePack;

impl GameReadyRenderFeaturePack {
    #[inline]
    pub fn new() -> Self {
        Self
    }

    #[inline]
    pub fn material_pipeline_provider(&self) -> Box<dyn MaterialGpuPipelineProvider> {
        Box::new(GameReadyLitMaterialDomainProvider::new())
    }

    #[inline]
    pub fn primary_lit_material_domain(&self) -> MaterialGpuPipelineKey {
        GAME_READY_LIT_PIPELINE_KEY
    }

    #[inline]
    pub fn draw_list_providers(&self) -> Vec<Arc<dyn RenderDrawListProvider>> {
        vec![
            Arc::new(GameReadyTerrainProvider),
            Arc::new(GameReadyPrimitiveMeshProvider),
        ]
    }

    #[inline]
    pub fn light_extraction_providers(&self) -> Vec<Arc<dyn LightExtractionProvider>> {
        vec![
            Arc::new(GameReadyDirectionalShadowProvider),
            Arc::new(GameReadyPointCubeShadowProvider),
            Arc::new(GameReadySpotShadowProvider),
            Arc::new(GameReadyAmbientOcclusionProvider),
        ]
    }
}

struct GameReadyTerrainProvider;

impl RenderDrawListProvider for GameReadyTerrainProvider {
    #[inline]
    fn id(&self) -> &'static str {
        GAME_READY_TERRAIN_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> RenderDrawListProviderMetadata {
        RenderDrawListProviderMetadata::feature(self.id(), "GameReady terrain draw extraction")
    }

    #[inline]
    fn provided_draw_lists(&self, ctx: &SceneExtractionCtx<'_>) -> &'static [RenderDrawListKind] {
        shadow_and_opaque_list(ctx.render_shadow_map)
    }

    fn extract(&self, ctx: &SceneExtractionCtx<'_>, out: &mut dyn DrawListBuildCtx) -> EngineResult<()> {
        if ctx.render_shadow_map {
            out.record_procedural_terrain_shadow(ctx)?;
        }
        out.record_procedural_terrain_forward(ctx)
    }
}

struct GameReadyPrimitiveMeshProvider;

impl RenderDrawListProvider for GameReadyPrimitiveMeshProvider {
    #[inline]
    fn id(&self) -> &'static str {
        GAME_READY_PRIMITIVE_MESH_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> RenderDrawListProviderMetadata {
        RenderDrawListProviderMetadata::feature(self.id(), "GameReady primitive mesh draw extraction")
    }

    #[inline]
    fn provided_draw_lists(&self, ctx: &SceneExtractionCtx<'_>) -> &'static [RenderDrawListKind] {
        shadow_and_opaque_list(ctx.render_shadow_map)
    }

    fn extract(&self, ctx: &SceneExtractionCtx<'_>, out: &mut dyn DrawListBuildCtx) -> EngineResult<()> {
        if ctx.render_shadow_map {
            out.record_primitive_mesh_shadow(ctx)?;
        }
        out.record_primitive_mesh_forward(ctx)
    }
}

struct GameReadyDirectionalShadowProvider;

impl LightExtractionProvider for GameReadyDirectionalShadowProvider {
    #[inline]
    fn id(&self) -> &'static str {
        GAME_READY_DIRECTIONAL_SHADOW_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> LightExtractionProviderMetadata {
        LightExtractionProviderMetadata::feature(self.id(), "GameReady directional shadow planning")
    }

    #[inline]
    fn supports(&self, ctx: &LightExtractionCtx<'_>) -> bool {
        matches!(
            ctx.settings.method,
            ShadowMethod::DirectionalDepthMap | ShadowMethod::CascadedShadowMaps
        ) && ctx.lights.has_directional_light()
    }

    #[inline]
    fn extract(&self, _ctx: &LightExtractionCtx<'_>) -> EngineResult<Option<LightExtractionCommand>> {
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
        LightExtractionProviderMetadata::feature(self.id(), "GameReady point shadow planning")
    }

    #[inline]
    fn supports(&self, ctx: &LightExtractionCtx<'_>) -> bool {
        matches!(ctx.settings.method, ShadowMethod::PointCubeMap)
            && ctx.lights.primary_point_light().is_some()
    }

    #[inline]
    fn extract(&self, _ctx: &LightExtractionCtx<'_>) -> EngineResult<Option<LightExtractionCommand>> {
        Ok(Some(LightExtractionCommand::Unsupported(ShadowLightKind::Point)))
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
        LightExtractionProviderMetadata::feature(self.id(), "GameReady spot shadow planning")
    }

    #[inline]
    fn supports(&self, ctx: &LightExtractionCtx<'_>) -> bool {
        matches!(ctx.settings.method, ShadowMethod::SpotDepthMap)
    }

    #[inline]
    fn extract(&self, _ctx: &LightExtractionCtx<'_>) -> EngineResult<Option<LightExtractionCommand>> {
        Ok(Some(LightExtractionCommand::Unsupported(ShadowLightKind::Spot)))
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
        LightExtractionProviderMetadata::feature(self.id(), "GameReady ambient occlusion planning")
    }

    #[inline]
    fn supports(&self, _ctx: &LightExtractionCtx<'_>) -> bool {
        false
    }

    #[inline]
    fn extract(&self, _ctx: &LightExtractionCtx<'_>) -> EngineResult<Option<LightExtractionCommand>> {
        Ok(None)
    }
}
