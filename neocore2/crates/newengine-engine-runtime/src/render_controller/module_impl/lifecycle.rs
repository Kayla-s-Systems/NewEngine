use std::time::Instant;

use newengine_core::host_events::CursorState;
use newengine_core::render::require_render_api;
use newengine_core::{EngineResult, Module, ModuleCtx};

use super::super::controller::RuntimeRenderController;

impl RuntimeRenderController {
    pub(super) fn start_runtime_module<E: Send + 'static>(
        &mut self,
        ctx: &mut ModuleCtx<'_, E>,
    ) -> EngineResult<()> {
        if let Ok(api) = require_render_api(ctx) {
            let started_at = Instant::now();
            log::info!("render controller: loading-screen pipeline warmup begin");
            let mut r = api.lock();
            if let Err(e) = self.gpu.require_primary_lit_pipeline(&mut **r) {
                log::warn!(
                    "render controller: loading-screen pipeline warmup skipped err='{}' elapsed_ms={:.2}",
                    e,
                    started_at.elapsed().as_secs_f64() * 1000.0
                );
            } else {
                let _ = r.pump_uploads(
                    newengine_core::render::UploadPumpDesc::loading_screen_warmup(),
                );
                log::info!(
                    "render controller: loading-screen pipeline warmup completed elapsed_ms={:.2}",
                    started_at.elapsed().as_secs_f64() * 1000.0
                );
            }
        } else {
            log::warn!("render controller: loading-screen pipeline warmup skipped because engine.render gateway is unavailable");
        }
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
