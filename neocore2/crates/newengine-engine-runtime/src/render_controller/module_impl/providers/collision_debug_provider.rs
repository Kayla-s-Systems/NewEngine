#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::EngineResult;
use newengine_core::render::RenderDrawListKind;

use super::super::draw_lists::{opaque_forward_list, DrawListBuildCtx, RenderDrawListProvider, SceneExtractionCtx};
use super::super::passes;

pub(super) struct CollisionDebugProvider;

impl RenderDrawListProvider for CollisionDebugProvider {
    #[inline]
    fn id(&self) -> &'static str {
        "runtime.collision_debug"
    }

    #[inline]
    fn provided_draw_lists(&self, ctx: &SceneExtractionCtx<'_>) -> &'static [RenderDrawListKind] {
        // This is a 3D viewport debug list, so it is routed through OpaqueForward until the
        // backend has a separate viewport-space Debug3D pass distinct from surface overlays.
        opaque_forward_list(ctx.editor_overlays)
    }

    fn extract(
        &self,
        ctx: &SceneExtractionCtx<'_>,
        out: &mut DrawListBuildCtx<'_>,
    ) -> EngineResult<()> {
        if !ctx.editor_overlays {
            return Ok(());
        }
        let _ = out.record(RenderDrawListKind::OpaqueForward, |this, r| {
            passes::draw_collision_wireframe(this, r, ctx.scene, ctx.viewproj)
        })?;
        Ok(())
    }
}
