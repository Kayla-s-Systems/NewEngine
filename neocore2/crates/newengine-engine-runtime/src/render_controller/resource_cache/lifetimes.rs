use newengine_core::render::RenderTargetId;

use super::super::controller::RuntimeRenderController;

impl RuntimeRenderController {
    pub(in crate::render_controller) fn collect_render_lifetime_events(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
    ) {
        self.gpu
            .lifetimes
            .resources
            .collect(r, self.frame.frame_index, self.backend_execution);
    }

    pub(in crate::render_controller) fn retire_render_target(&mut self, rt: RenderTargetId) {
        self.gpu
            .lifetimes
            .resources
            .retire_render_target_after_frame(rt, self.frame.frame_index);
    }

    pub(in crate::render_controller) fn gc_deferred_rts(
        &mut self,
        r: &mut dyn newengine_core::render::RenderApi,
    ) {
        self.collect_render_lifetime_events(r);
    }

    pub(in crate::render_controller) fn bridge_render_backend_events<E: Send + 'static>(
        &mut self,
        ctx: &mut newengine_core::ModuleCtx<'_, E>,
        r: &mut dyn newengine_core::render::RenderApi,
    ) {
        self.gpu.lifetimes.resources.subscribe(ctx.events());
        match r.drain_backend_events() {
            Ok(events) => {
                for event in events {
                    self.gpu.material.registry.observe_backend_event(&event);
                    let _ = ctx.events().publish(event);
                }
            }
            Err(err) => {
                newengine_ulog_api::ulog::warn!(
                    "render controller: failed to drain renderer backend events err='{}'",
                    err
                );
            }
        }
        self.collect_render_lifetime_events(r);
    }
}
