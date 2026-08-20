use std::sync::Arc;

use newengine_core::render::RenderDrawListKind;
use newengine_core::EngineResult;
use newengine_render_feature_api::{
    shadow_lists_and_opaque, DrawListBuildCtx, RenderDrawListProvider,
    RenderDrawListProviderMetadata, SceneExtractionCtx,
};

use crate::{STANDARD_PRIMITIVE_MESH_PROVIDER_ID, STANDARD_TERRAIN_PROVIDER_ID};

pub(crate) fn providers() -> Vec<Arc<dyn RenderDrawListProvider>> {
    vec![
        Arc::new(StandardTerrainProvider),
        Arc::new(StandardPrimitiveMeshProvider),
    ]
}

#[inline]
fn feature_metadata(id: &'static str, description: &'static str) -> RenderDrawListProviderMetadata {
    RenderDrawListProviderMetadata::feature(id, description)
}

struct StandardTerrainProvider;

impl RenderDrawListProvider for StandardTerrainProvider {
    #[inline]
    fn id(&self) -> &'static str {
        STANDARD_TERRAIN_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> RenderDrawListProviderMetadata {
        feature_metadata(self.id(), "Standard terrain draw extraction")
    }

    #[inline]
    fn provided_draw_lists(&self, ctx: &SceneExtractionCtx<'_>) -> &'static [RenderDrawListKind] {
        shadow_lists_and_opaque(ctx.render_shadow_map, ctx.render_local_shadow_map)
    }

    fn extract(
        &self,
        ctx: &SceneExtractionCtx<'_>,
        out: &mut dyn DrawListBuildCtx,
    ) -> EngineResult<()> {
        if ctx.render_shadow_map {
            out.record_procedural_terrain_shadow(ctx)?;
        }
        if ctx.render_local_shadow_map {
            out.record_procedural_terrain_local_shadow(ctx)?;
        }
        if ctx.deferred {
            out.record_procedural_terrain_gbuffer(ctx)
        } else {
            out.record_procedural_terrain_forward(ctx)
        }
    }
}

struct StandardPrimitiveMeshProvider;

impl RenderDrawListProvider for StandardPrimitiveMeshProvider {
    #[inline]
    fn id(&self) -> &'static str {
        STANDARD_PRIMITIVE_MESH_PROVIDER_ID
    }

    #[inline]
    fn metadata(&self) -> RenderDrawListProviderMetadata {
        feature_metadata(self.id(), "Standard primitive mesh draw extraction")
    }

    #[inline]
    fn provided_draw_lists(&self, ctx: &SceneExtractionCtx<'_>) -> &'static [RenderDrawListKind] {
        shadow_lists_and_opaque(ctx.render_shadow_map, ctx.render_local_shadow_map)
    }

    fn extract(
        &self,
        ctx: &SceneExtractionCtx<'_>,
        out: &mut dyn DrawListBuildCtx,
    ) -> EngineResult<()> {
        if ctx.render_shadow_map {
            out.record_primitive_mesh_shadow(ctx)?;
        }
        if ctx.render_local_shadow_map {
            out.record_primitive_mesh_local_shadow(ctx)?;
        }
        if ctx.deferred {
            out.record_primitive_mesh_gbuffer(ctx)?;
        }
        // Forward remains active in deferred mode for sky, transparent and
        // view-model roles; the runtime pass filter excludes deferred opaques.
        out.record_primitive_mesh_forward(ctx)
    }
}
