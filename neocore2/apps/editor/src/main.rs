use crossbeam_channel::unbounded;

use newengine_assets::{AssetKey, AssetState};
use newengine_core::{
    assets::AssetManager,
    Bus, ConfigPaths, Engine, EngineResult, Services, ShutdownToken, StartupDefaults, StartupLoader,
};
use newengine_modules_logging::{ConsoleLoggerConfig, ConsoleLoggerModule};
use newengine_modules_render_vulkan_ash::VulkanAshRenderModule;
use newengine_platform_winit::{run_winit_app_with_config, WinitAppConfig, WinitWindowPlacement};

use std::time::{Duration, Instant};

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
    let paths = ConfigPaths::from_startup_str("startup.json");
    let defaults = StartupDefaults::default();


    let (startup, report) = StartupLoader::load_json(&paths, &defaults)?;

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
    

    let cfg = WinitAppConfig::default();
    // IMPORTANT: engine must be mutable here because we load plugins + assets in main.
    let mut engine = build_engine()?;

    // 1) Load plugins only (no module init/start => Vulkan isn't touched).
    // This is required so DDS importer can auto-register.
    engine.load_plugins_only()?;

    // 2) Load asset in main (entrypoint), before the window loop.
    {
        let am = engine
            .resources_mut()
            .get::<AssetManager>()
            .expect("AssetManager resource missing");

        let key = AssetKey::new("textures/dd_gaviota_01.dds", 0);

        let id = am.store().load(key).map_err(|e| {
            newengine_core::EngineError::Other(format!("asset load enqueue failed: {e}"))
        })?;

        let deadline = Instant::now() + Duration::from_secs(3);

        loop {
            am.pump();

            match am.store().state(id) {
                AssetState::Ready => {
                    if let Some(blob) = am.store().get_blob(id) {
                        log::info!(
                            target: "assets",
                            "entrypoint: asset.ready id={:032x} type='{}' format='{}' bytes={}",
                            id.to_u128(),
                            blob.type_id,
                            blob.format,
                            blob.payload.len()
                        );
                    }
                    break;
                }
                AssetState::Failed(err) => {
                    log::error!(
                        target: "assets",
                        "entrypoint: asset.failed id={:032x} error='{}'",
                        id.to_u128(),
                        err
                    );
                    break;
                }
                AssetState::Loading | AssetState::Unloaded => {
                    if Instant::now() >= deadline {
                        log::warn!(
                            target: "assets",
                            "entrypoint: asset.timeout id={:032x} state={:?}",
                            id.to_u128(),
                            am.store().state(id)
                        );
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
    }

    // 3) Now start the winit host normally.
    // DO NOT call engine.start() manually here; winit host controls lifecycle.
    run_winit_app_with_config(engine, cfg, |engine| {
        engine.register_module(Box::new(VulkanAshRenderModule::default()))?;
        Ok(())
    })?;

    println!("engine stopped");
    Ok(())
}
