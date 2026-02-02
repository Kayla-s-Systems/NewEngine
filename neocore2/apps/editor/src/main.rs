use crossbeam_channel::unbounded;

use newengine_core::{
    AssetManagerConfig, Bus, ConfigPaths, Engine, EngineConfig, EngineError, EngineResult, Services,
    ShutdownToken, StartupConfig, StartupLoader,
};
use newengine_modules_logging::{ConsoleLoggerConfig, ConsoleLoggerModule};
use newengine_modules_render_vulkan_ash::{VulkanAshRenderModule, VulkanRenderConfig};
use newengine_platform_winit::{run_winit_app_with_config, WinitAppConfig};

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

fn build_engine_from_startup(startup: &StartupConfig) -> EngineResult<Engine<()>> {
    let (tx, rx) = unbounded::<()>();
    let bus: Bus<()> = Bus::new(tx, rx);

    let services: Box<dyn Services> = Box::new(AppServices::new());
    let shutdown = ShutdownToken::new();

    let assets = AssetManagerConfig::new(startup.assets_root.clone())
        .with_pump_steps(startup.asset_pump_steps)
        .with_filesystem_source(startup.asset_filesystem_source);


    let config = EngineConfig::new(16, assets).with_plugins_dir(Option::from(startup.modules_dir.clone()));
    let mut engine: Engine<()> = Engine::new_with_config(config, services, bus, shutdown)?;

    engine.register_module(Box::new(ConsoleLoggerModule::new(
        ConsoleLoggerConfig::from_env(),
    )))?;

    Ok(engine)
}

fn main() -> EngineResult<()> {
    let paths = ConfigPaths::from_startup_str("config.json");
    let (startup, report) = StartupLoader::load_json(&paths)?;

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


    let engine = build_engine_from_startup(&startup)?;
    let render_backend = startup.render_backend.clone();
    let render_clear_color = startup.render_clear_color;
    let render_debug_text = startup.render_debug_text.clone();

    run_winit_app_with_config(engine, WinitAppConfig::default(), move |engine| {
        if render_backend.eq_ignore_ascii_case("vulkan_ash")
            || render_backend.eq_ignore_ascii_case("vulkan")
        {
            let debug_text_opt = if render_debug_text.trim().is_empty() {
                None
            } else {
                Some(render_debug_text.clone())
            };

            let config = VulkanRenderConfig {
                clear_color: render_clear_color,
                debug_text: debug_text_opt,
            };

            engine.register_module(Box::new(VulkanAshRenderModule::new(config)))?;
            Ok(())
        } else {
            Err(EngineError::other(format!(
                "unsupported render backend '{render_backend}'"
            )))
        }
    })?;

    println!("engine stopped");
    Ok(())
}