#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{Extent2D, RenderTargetDesc, TextureFormat};
use newengine_core::{EngineError, EngineResult};

use super::controller::RuntimeRenderController;

impl RuntimeRenderController {
    #[inline]
    fn extent_eq(a: Extent2D, b: Extent2D) -> bool {
        a.width == b.width && a.height == b.height
    }

    pub(super) fn ensure_viewport_rt(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
        extent: Extent2D,
    ) -> EngineResult<newengine_core::render::RenderTargetId> {
        if let Some(rt) = self.viewport.render_target {
            if Self::extent_eq(self.viewport.render_target_extent, extent) {
                return Ok(rt);
            }

            // Do not destroy immediately: GPU may still reference the old framebuffer.
            self.retire_render_target(rt);
            self.viewport.render_target = None;
        }

        let rt = r.create_render_target(
            RenderTargetDesc::new(extent, TextureFormat::Bgra8Unorm)
                // Depth is critical for an game viewport: correct occlusion, stable gizmo/grid.
                .with_depth(TextureFormat::Depth32Float)
                .with_label("game_viewport_rt"),
        )?;

        self.viewport.render_target = Some(rt);
        self.viewport.render_target_extent = extent;

        let ui_tex = r.render_target_ui_tex_id(rt)?;
        self.bridges.viewport.publish_tex_user(ui_tex.0 as u64);

        Ok(rt)
    }

    #[allow(dead_code)]
    pub(super) fn viewport_rt(&self) -> Option<newengine_core::render::RenderTargetId> {
        self.viewport.render_target
    }

    #[allow(dead_code)]
    pub(super) fn viewport_rt_extent(&self) -> Extent2D {
        self.viewport.render_target_extent
    }

    #[inline]
    #[allow(dead_code)]
    pub(super) fn require_nonzero_viewport_extent(
        extent: Extent2D,
    ) -> Result<Extent2D, EngineError> {
        if extent.width == 0 || extent.height == 0 {
            return Err(EngineError::other("viewport extent is zero"));
        }
        Ok(extent)
    }
}
