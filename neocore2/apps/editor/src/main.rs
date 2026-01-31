use crossbeam_channel::unbounded;

use newengine_core::{
    Bus, ConfigPaths, Engine, EngineResult, Services, ShutdownToken, StartupDefaults, StartupLoader,
};
use newengine_modules_logging::{ConsoleLoggerConfig, ConsoleLoggerModule};
use newengine_modules_render_vulkan_ash::VulkanAshRenderModule;
use newengine_platform_winit::{run_winit_app_with_config, WinitAppConfig, WinitWindowPlacement};

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

fn build_engine() -> EngineResult<Engine<()>> {
    let (tx, rx) = unbounded::<()>();
    let bus: Bus<()> = Bus::new(tx, rx);

    let services: Box<dyn Services> = Box::new(AppServices::new());
    let shutdown = ShutdownToken::new();

    let mut engine: Engine<()> = Engine::new(16, services, bus, shutdown)?;

    engine.register_module(Box::new(ConsoleLoggerModule::new(
        ConsoleLoggerConfig::from_env(),
    )))?;

    Ok(engine)
}

fn main() -> EngineResult<()> {
    // App provides only path + defaults
    let paths = ConfigPaths::from_startup_str("config.json");
    let defaults = StartupDefaults {
        log_level: Some("info".to_owned()),
        window_title: Some("NewEngine Editor".to_owned()),
        window_size: Some((1600, 900)),
        window_placement: None,
        modules_dir: None,
    };

    // Single reusable call: returns config + ready-to-log report
    let (startup, report) = StartupLoader::load_json(&paths, &defaults)?;

    // IMPORTANT: engine logger isn't installed yet (ConsoleLoggerModule starts after engine.start()).
    // So we must print startup report directly to stdout/stderr to guarantee visibility.
    println!(
        "startup: loaded source={:?} file={:?} resolved_from={:?} overrides={}",
        report.source,
        report.file,
        report.resolved_from,
        report.overrides.len()
    );
    for ov in report.overrides.iter() {
        println!("startup: override {}: '{}' -> '{}'", ov.key, ov.from, ov.to);
    }

    // Use resolved config (apply result)
    let title = startup
        .window_title
        .clone()
        .unwrap_or_else(|| "NewEngine".to_owned());
    let (w, h) = startup.window_size.unwrap_or((1280, 720));

    let cfg = WinitAppConfig {
        title,
        size: (w, h),
        placement: WinitWindowPlacement::Centered { offset: (0, -24) },
        ..WinitAppConfig::default()
    };

    let engine = build_engine()?;

    run_winit_app_with_config(engine, cfg, |engine| {
        engine.register_module(Box::new(VulkanAshRenderModule::default()))?;
        Ok(())
    })?;

    println!("engine stopped");
    Ok(())
}