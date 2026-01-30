use crossbeam_channel::unbounded;
use log::info;

use newengine_core::{
    Bus, Engine, EngineResult, Module, ModuleCtx, Services, ShutdownToken,
};
use newengine_modules_logging::{ConsoleLoggerConfig, ConsoleLoggerModule};
use newengine_platform_winit::run_winit_app;

#[derive(Debug, Clone)]
enum EngineEvent {
    Exit,
}

/* ============================
   Services
   ============================ */

struct AppServices;

impl AppServices {
    #[inline]
    fn new() -> Self {
        Self
    }
}

impl Services for AppServices {
    #[inline]
    fn logger(&self) -> &dyn log::Log {
        log::logger()
    }
}

/* ============================
   Main
   ============================ */

fn main() -> EngineResult<()> {
    let (tx, rx) = unbounded::<EngineEvent>();
    let bus: Bus<EngineEvent> = Bus::new(tx, rx);

    let services: Box<dyn Services> = Box::new(AppServices::new());
    let shutdown = ShutdownToken::new();

    let mut engine: Engine<EngineEvent> =
        Engine::new(16, services, bus, shutdown)?;

    engine.register_module(Box::new(
        ConsoleLoggerModule::new(ConsoleLoggerConfig::from_env()),
    ))?;

    engine.register_module(Box::new(EngineOperatorModule::new()))?;

    engine.start()?;

    info!("engine started");

    run_winit_app(engine)
}

/* ============================
   Operator Module
   ============================ */

struct EngineOperatorModule {
    queue: Vec<EngineEvent>,
}

impl EngineOperatorModule {
    #[inline]
    fn new() -> Self {
        Self {
            queue: Vec::new(),
        }
    }
}

impl Module<EngineEvent> for EngineOperatorModule {
    fn id(&self) -> &'static str {
        "engine-operator"
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, EngineEvent>) -> EngineResult<()> {
        ctx.bus().drain_into(&mut self.queue);

        for ev in self.queue.drain(..) {
            match ev {
                EngineEvent::Exit => ctx.request_exit(),
            }
        }

        Ok(())
    }
}
