use std::sync::Arc;

use newengine_core::render::RenderDrawListKind;
use newengine_core::EngineResult;
use newengine_render_feature_api::{
    shadow_and_opaque_list, DrawListBuildCtx, RenderDrawListProvider,
    RenderDrawListProviderMetadata, SceneExtractionCtx,
};

use crate::{GAME_READY_PRIMITIVE_MESH_PROVIDER_ID, GAME_READY_TERRAIN_PROVIDER_ID};

pub(crate) fn providers() -> Vec<Arc<dyn RenderDrawListProvider>> {
    vec![
        Arc::new(GameReadyTerrainProvider),
        Arc::new(GameReadyPrimitiveMeshProvider),
    ]
}

#[inline]
fn feature_metadata(id: &'static str, description: &'static str) -> RenderDrawListProviderMetadata {
    RenderDrawListProviderMetadata::feature(id, description)
}

struct GameReadyTerrainProvider;

impl RenderDrawListProvider for GameReadyTerrainProvider {
    #[inline]
    fn id(&self) -> &'static str {
        GAME_READY_TERRAIN_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> RenderDrawListProviderMetadata {
        feature_metadata(self.id(), "GameReady terrain draw extraction")
    }

    #[inline]
    fn provided_draw_lists(&self, ctx: &SceneExtractionCtx<'_>) -> &'static [RenderDrawListKind] {
        shadow_and_opaque_list(ctx.render_shadow_map)
    }

    fn extract(
        &self,
        ctx: &SceneExtractionCtx<'_>,
        out: &mut dyn DrawListBuildCtx,
    ) -> EngineResult<()> {
        if ctx.render_shadow_map {
            out.record_procedural_terrain_shadow(ctx)?;
        }
        if ctx.deferred {
            out.record_procedural_terrain_gbuffer(ctx)
        } else {
            out.record_procedural_terrain_forward(ctx)
        }
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
        feature_metadata(self.id(), "GameReady primitive mesh draw extraction")
    }

    #[inline]
    fn provided_draw_lists(&self, ctx: &SceneExtractionCtx<'_>) -> &'static [RenderDrawListKind] {
        shadow_and_opaque_list(ctx.render_shadow_map)
    }

    fn extract(
        &self,
        ctx: &SceneExtractionCtx<'_>,
        out: &mut dyn DrawListBuildCtx,
    ) -> EngineResult<()> {
        if ctx.render_shadow_map {
            out.record_primitive_mesh_shadow(ctx)?;
        }
        if ctx.deferred {
            out.record_primitive_mesh_gbuffer(ctx)?;
        }
        // Forward remains active in deferred mode for sky, transparent and
        // view-model roles; the runtime pass filter excludes deferred opaques.
        out.record_primitive_mesh_forward(ctx)
    }
}
