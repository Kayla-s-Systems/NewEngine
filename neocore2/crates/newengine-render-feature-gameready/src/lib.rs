#![forbid(unsafe_op_in_unsafe_fn)]

//! Backward-compatible GameReady facade over `newengine-render-feature-standard`.
//!
//! Draw/light extraction is implemented only by the standard pack. This facade wraps
//! those providers solely to preserve historical provider IDs for existing profiles.

use std::sync::Arc;

use newengine_material_domain_api::{MaterialGpuPipelineKey, MaterialGpuPipelineProvider};
use newengine_render_feature_api::{
    DrawListBuildCtx, LightExtractionCommand, LightExtractionCtx, LightExtractionProvider,
    LightExtractionProviderMetadata, RenderDrawListProvider, RenderDrawListProviderMetadata,
    SceneExtractionCtx,
};

pub const GAME_READY_TERRAIN_PROVIDER_ID: &str = "gameready.terrain";
pub const GAME_READY_PRIMITIVE_MESH_PROVIDER_ID: &str = "gameready.primitive_mesh";
pub const GAME_READY_DIRECTIONAL_SHADOW_PROVIDER_ID: &str = "gameready.directional_shadow";
pub const GAME_READY_POINT_CUBE_SHADOW_PROVIDER_ID: &str = "gameready.point_cube_shadow";
pub const GAME_READY_SPOT_SHADOW_PROVIDER_ID: &str = "gameready.spot_shadow";
pub const GAME_READY_AMBIENT_OCCLUSION_PROVIDER_ID: &str = "gameready.ambient_occlusion";

const LEGACY_DRAW_IDS: &[&str] = &[
    GAME_READY_TERRAIN_PROVIDER_ID,
    GAME_READY_PRIMITIVE_MESH_PROVIDER_ID,
];
const LEGACY_LIGHT_IDS: &[&str] = &[
    GAME_READY_DIRECTIONAL_SHADOW_PROVIDER_ID,
    GAME_READY_POINT_CUBE_SHADOW_PROVIDER_ID,
    GAME_READY_SPOT_SHADOW_PROVIDER_ID,
    GAME_READY_AMBIENT_OCCLUSION_PROVIDER_ID,
];

#[derive(Default)]
pub struct GameReadyRenderFeaturePack {
    standard: newengine_render_feature_standard::StandardRenderFeaturePack,
}

impl GameReadyRenderFeaturePack {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn material_pipeline_provider(&self) -> Box<dyn MaterialGpuPipelineProvider> {
        Box::new(newengine_material_domain_gameready::GameReadyLitMaterialDomainProvider::new())
    }

    #[inline]
    pub fn primary_lit_material_domain(&self) -> MaterialGpuPipelineKey {
        newengine_material_domain_gameready::GAME_READY_LIT_PIPELINE_KEY
    }

    pub fn draw_list_providers(&self) -> Vec<Arc<dyn RenderDrawListProvider>> {
        self.standard
            .draw_list_providers()
            .into_iter()
            .enumerate()
            .map(|(index, inner)| {
                let id = LEGACY_DRAW_IDS
                    .get(index)
                    .copied()
                    .unwrap_or_else(|| inner.id());
                Arc::new(CompatDrawProvider { id, inner }) as Arc<dyn RenderDrawListProvider>
            })
            .collect()
    }

    pub fn light_extraction_providers(&self) -> Vec<Arc<dyn LightExtractionProvider>> {
        self.standard
            .light_extraction_providers()
            .into_iter()
            .enumerate()
            .map(|(index, inner)| {
                let id = LEGACY_LIGHT_IDS
                    .get(index)
                    .copied()
                    .unwrap_or_else(|| inner.id());
                Arc::new(CompatLightProvider { id, inner }) as Arc<dyn LightExtractionProvider>
            })
            .collect()
    }
}

struct CompatDrawProvider {
    id: &'static str,
    inner: Arc<dyn RenderDrawListProvider>,
}

impl RenderDrawListProvider for CompatDrawProvider {
    #[inline]
    fn id(&self) -> &'static str {
        self.id
    }

    #[inline]
    fn metadata(&self) -> RenderDrawListProviderMetadata {
        RenderDrawListProviderMetadata::feature(self.id, self.inner.metadata().label)
    }

    #[inline]
    fn provided_draw_lists(
        &self,
        ctx: &SceneExtractionCtx<'_>,
    ) -> &'static [newengine_core::render::RenderDrawListKind] {
        self.inner.provided_draw_lists(ctx)
    }

    #[inline]
    fn extract(
        &self,
        ctx: &SceneExtractionCtx<'_>,
        out: &mut dyn DrawListBuildCtx,
    ) -> newengine_core::EngineResult<()> {
        self.inner.extract(ctx, out)
    }
}

struct CompatLightProvider {
    id: &'static str,
    inner: Arc<dyn LightExtractionProvider>,
}

impl LightExtractionProvider for CompatLightProvider {
    #[inline]
    fn id(&self) -> &'static str {
        self.id
    }

    #[inline]
    fn metadata(&self) -> LightExtractionProviderMetadata {
        LightExtractionProviderMetadata::feature(self.id, self.inner.metadata().label)
    }

    #[inline]
    fn supports(&self, ctx: &LightExtractionCtx<'_>) -> bool {
        self.inner.supports(ctx)
    }

    #[inline]
    fn extract(
        &self,
        ctx: &LightExtractionCtx<'_>,
    ) -> newengine_core::EngineResult<Option<LightExtractionCommand>> {
        self.inner.extract(ctx)
    }
}

#[cfg(test)]
mod tests;
