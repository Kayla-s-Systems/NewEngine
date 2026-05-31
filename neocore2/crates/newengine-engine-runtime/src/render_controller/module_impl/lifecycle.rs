use newengine_core::host_events::CursorState;
use newengine_core::{EngineResult, Module, ModuleCtx};

use super::super::controller::RuntimeRenderController;

impl RuntimeRenderController {
    pub(super) fn start_runtime_module<E: Send + 'static>(
        &mut self,
        ctx: &mut ModuleCtx<'_, E>,
    ) -> EngineResult<()> {
        self.gpu.lifetimes.resources.subscribe(ctx.events());
        // Do not build the GameReady lit pipeline at module start. In Editor/Edit
        // this was the first visible hitch and it initialized gameplay render work
        // before the user pressed Simulate or Play. The pipeline provider remains
        // lazy: render_controller builds it only when a real scene viewport frame is
        // submitted.
        log::info!(
            "render controller: scene pipeline warmup deferred until Simulate/Play or game profile viewport"
        );
        Ok(())
    }

    pub(super) fn shutdown_runtime_module<E: Send + 'static>(
        &mut self,
        ctx: &mut ModuleCtx<'_, E>,
    ) -> EngineResult<()> {
        newengine_core::crash::record_breadcrumb(
            "render controller: shutdown begin".to_string(),
        );
        self.sync_cursor_state(ctx, CursorState::released());
        self.viewport.pass_disabled = true;
        Ok(())
    }
}

impl<E: Send + 'static> Module<E> for RuntimeRenderController {
    fn id(&self) -> &'static str {
        "engine.render_controller"
    }

    fn start(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        self.start_runtime_module(ctx)
    }

    fn shutdown(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        self.shutdown_runtime_module(ctx)
    }

    fn render(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        self.render_runtime_module(ctx)
    }
}
