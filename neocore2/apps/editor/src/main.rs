use crossbeam_channel::unbounded;
use log::info;

use newengine_core::{Bus, Engine, EngineResult, Services, ShutdownToken};
use newengine_modules_logging::{ConsoleLoggerConfig, ConsoleLoggerModule};
use newengine_platform_winit::{run_winit_app_with_config, WinitAppConfig, WinitWindowPlacement};

use newengine_modules_render_vulkan_ash::VulkanAshRenderModule;

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

fn main() -> EngineResult<()> {
    let (tx, rx) = unbounded::<()>();
    let bus: Bus<()> = Bus::new(tx, rx);

    let services: Box<dyn Services> = Box::new(AppServices::new());
    let shutdown = ShutdownToken::new();
    let mut engine: Engine<()> = Engine::new(16, services, bus, shutdown)?;
    let cfg = WinitAppConfig {
        title: "NEØCORE Editor".to_owned(),
        size: (1600, 900),
        placement: WinitWindowPlacement::Centered { offset: (0, -24) },
    };

    engine.register_module(Box::new(ConsoleLoggerModule::new(
        ConsoleLoggerConfig::from_env(),
    )))?;

    run_winit_app_with_config(engine, cfg, |engine| {
        engine.register_module(Box::new(VulkanAshRenderModule::default()))?;
        Ok(())
    })?;

    info!("engine stopped");
    Ok(())
}