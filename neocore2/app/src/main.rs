use std::time::Duration;

use engine_core::{
    Engine,
    EngineConfig,
    frame::FrameContext,
    module::Module,
    phase::FramePhase,
};

struct DebugModule;

impl Module for DebugModule {
    fn id(&self) -> &'static str {
        "debug"
    }

    fn on_start(&mut self, ctx: &mut FrameContext<'_>) {
        let _ = ctx.window;
        ctx.telemetry.record_scope("Debug:on_start", Duration::ZERO);
    }

    fn on_phase(&mut self, phase: FramePhase, ctx: &mut FrameContext<'_>) {
        if phase == FramePhase::BeginFrame && ctx.time.frame_index == 1 {
            ctx.telemetry
                .record_scope("Debug:first_frame", Duration::ZERO);
        }

        if phase == FramePhase::FixedUpdate && (ctx.time.fixed_tick_index % 60 == 0) {
            let _tick = ctx.time.fixed_tick_index;
        }
    }

    fn on_shutdown(&mut self, ctx: &mut FrameContext<'_>) {
        ctx.telemetry
            .record_scope("Debug:shutdown", Duration::ZERO);
    }
}

fn main() -> anyhow::Result<()> {
    let mut engine = Engine::new(EngineConfig::default());
    engine.add_module(DebugModule);
    engine.run()
}