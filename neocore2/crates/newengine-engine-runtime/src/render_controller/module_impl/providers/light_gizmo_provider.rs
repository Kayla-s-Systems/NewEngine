#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::EngineResult;
use newengine_core::render::RenderDrawListKind;

use super::super::draw_lists::{opaque_forward_list, DrawListBuildCtx, RenderDrawListProvider, SceneExtractionCtx};
use super::super::{passes, quat_from_forward_z};

pub(super) struct LightGizmoProvider;

impl RenderDrawListProvider for LightGizmoProvider {
    #[inline]
    fn id(&self) -> &'static str {
        "runtime.light_gizmos"
    }

    #[inline]
    fn provided_draw_lists(&self, ctx: &SceneExtractionCtx<'_>) -> &'static [RenderDrawListKind] {
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
            passes::draw_light_gizmos(
                this,
                r,
                ctx.scene,
                ctx.lit,
                ctx.viewproj,
                &ctx.lights,
                quat_from_forward_z,
                false,
            )
        })?;
        Ok(())
    }
}
