#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::EngineResult;
use newengine_core::render::RenderDrawListKind;

use super::super::draw_lists::{shadow_and_opaque_list, DrawListBuildCtx, RenderDrawListProvider, SceneExtractionCtx};
use super::super::passes;

pub(super) struct PrimitiveMeshProvider;

impl RenderDrawListProvider for PrimitiveMeshProvider {
    #[inline]
    fn id(&self) -> &'static str {
        "runtime.primitive_mesh"
    }

    #[inline]
    fn provided_draw_lists(&self, ctx: &SceneExtractionCtx<'_>) -> &'static [RenderDrawListKind] {
        shadow_and_opaque_list(ctx.render_shadow_map)
    }

    fn extract(
        &self,
        ctx: &SceneExtractionCtx<'_>,
        out: &mut DrawListBuildCtx<'_>,
    ) -> EngineResult<()> {
        if ctx.render_shadow_map {
            let _ = out.record(RenderDrawListKind::ShadowCasters, |this, r| {
                passes::draw_primitives_shadow(
                    this,
                    r,
                    ctx.scene,
                    ctx.lit,
                    ctx.shadow_frame.light_mvp,
                    &ctx.lights,
                    ctx.runtime,
                    ctx.rig.position,
                )
            })?;
        }

        let _ = out.record(RenderDrawListKind::OpaqueForward, |this, r| {
            passes::draw_primitives(
                this,
                r,
                ctx.scene,
                ctx.lit,
                ctx.viewproj,
                &ctx.lights,
                ctx.shadow_frame.texture,
                ctx.runtime,
                ctx.rig.position,
            )
        })?;

        Ok(())
    }
}
