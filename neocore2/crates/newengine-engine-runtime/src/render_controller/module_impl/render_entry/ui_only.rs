use super::*;

impl RuntimeRenderController {
    pub(in crate::render_controller::module_impl) fn render_ui_only_frame<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        r: &mut dyn newengine_core::render::RenderApi,
        ui: Option<UiDrawList>,
        scope: RenderFrameScope,
    ) -> EngineResult<()> {
        self.sync_cursor_state(ctx, newengine_core::host_events::CursorState::released());
        if let Some(ui) = ui {
            let win_extent = Extent2D::new(scope.w, scope.h);
            r.set_viewport(Viewport::full(win_extent))?;
            r.set_scissor(RectI32::new(0, 0, scope.w as i32, scope.h as i32))?;
            r.set_ui_draw_list(ui);
        }
        Ok(())
    }
}
