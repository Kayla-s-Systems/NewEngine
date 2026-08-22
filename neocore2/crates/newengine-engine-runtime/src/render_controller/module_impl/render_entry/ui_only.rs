use super::*;
use crate::render_controller::module_impl::frame_envelope_builder::build_ui_layer_frame_envelope;

impl RuntimeRenderController {
    pub(in crate::render_controller::module_impl) fn render_ui_only_frame<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        r: &mut dyn newengine_core::render::RenderApi,
        ui_layers: UiLayerDrawPacketSet,
        scope: RenderFrameScope,
    ) -> EngineResult<()> {
        self.sync_cursor_state(ctx, newengine_core::host_events::CursorState::released());
        if ui_layers.is_empty() {
            return Ok(());
        }

        let envelope = build_ui_layer_frame_envelope(
            self.frame.frame_index,
            self.viewport.clear_color,
            Extent2D::new(scope.w, scope.h),
            ui_layers,
        );
        let _ = r.submit_frame(envelope)?;
        Ok(())
    }
}
