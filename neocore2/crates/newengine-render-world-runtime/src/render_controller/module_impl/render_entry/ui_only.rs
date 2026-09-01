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

        if self.frame.frame_index <= 2 {
            let mesh_vertices = ui_layers
                .packets
                .iter()
                .map(|packet| packet.draw_list.mesh.vertices.len())
                .sum::<usize>();
            let mesh_indices = ui_layers
                .packets
                .iter()
                .map(|packet| packet.draw_list.mesh.indices.len())
                .sum::<usize>();
            let domains = ui_layers
                .packets
                .iter()
                .map(|packet| packet.domain.as_str())
                .collect::<Vec<_>>()
                .join(",");
            newengine_ulog_api::ulog::info!(
                "render ui-only: input frame={} packets={} domains='{}' mesh_vertices={} mesh_indices={}",
                self.frame.frame_index,
                ui_layers.packets.len(),
                domains,
                mesh_vertices,
                mesh_indices,
            );
        }

        if ui_layers.is_empty() {
            if self.frame.frame_index <= 2 {
                newengine_ulog_api::ulog::warn!(
                    "render ui-only: empty packet set frame={}",
                    self.frame.frame_index,
                );
            }
            return Ok(());
        }

        let envelope = build_ui_layer_frame_envelope(
            self.frame.frame_index,
            self.viewport.clear_color,
            Extent2D::new(scope.w, scope.h),
            ui_layers,
        );
        if self.frame.frame_index <= 2 {
            let passes = envelope
                .graph
                .passes
                .iter()
                .map(|pass| pass.label.as_str())
                .collect::<Vec<_>>()
                .join(",");
            newengine_ulog_api::ulog::info!(
                "render ui-only: submit frame={} label='{}' graph_passes={} passes='{}' ui_packets={}",
                self.frame.frame_index,
                envelope.label.as_deref().unwrap_or(""),
                envelope.graph.passes.len(),
                passes,
                envelope.ui_layers.packets.len(),
            );
        }
        let report = r.submit_frame(envelope)?;
        if self.frame.frame_index <= 2 {
            newengine_ulog_api::ulog::info!(
                "render ui-only: submit complete frame={} executed_passes={} skipped_passes={} compile_passes={} execution_order={}",
                self.frame.frame_index,
                report.executed_passes,
                report.skipped_passes,
                report.compile.pass_count,
                report.compile.execution_order.len(),
            );
        }
        Ok(())
    }
}
