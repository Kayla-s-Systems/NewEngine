#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::EngineResult;
use newengine_core::render::{RectI32, RenderDrawListKind, Viewport};

use super::super::draw_lists::{ui_list, DrawListBuildCtx, RenderDrawListProvider, SceneExtractionCtx};

pub(super) struct UiProvider;

impl RenderDrawListProvider for UiProvider {
    #[inline]
    fn id(&self) -> &'static str {
        "runtime.ui"
    }

    #[inline]
    fn provided_draw_lists(&self, ctx: &SceneExtractionCtx<'_>) -> &'static [RenderDrawListKind] {
        ui_list(ctx.ui.is_some())
    }

    fn extract(
        &self,
        ctx: &SceneExtractionCtx<'_>,
        out: &mut DrawListBuildCtx<'_>,
    ) -> EngineResult<()> {
        let Some(ui) = ctx.ui else {
            return Ok(());
        };
        let extent = ctx.surface_extent;
        let _ = out.record(RenderDrawListKind::Ui, |_this, r| {
            r.set_viewport(Viewport::full(extent))?;
            r.set_scissor(RectI32::new(
                0,
                0,
                extent.width as i32,
                extent.height as i32,
            ))?;
            r.set_ui_draw_list(ui.clone());
            Ok(())
        })?;
        Ok(())
    }
}
