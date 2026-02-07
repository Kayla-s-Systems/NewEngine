#![forbid(unsafe_op_in_unsafe_fn)]

use crossbeam_channel::unbounded;

use newengine_core::{
    AssetManagerConfig, Bus, ConfigPaths, Engine, EngineConfig, EngineError, EngineResult, Services,
    ShutdownToken, StartupConfig, StartupLoader,
};

use newengine_modules_logging::{ConsoleLoggerConfig, ConsoleLoggerModule};
use newengine_modules_render_vulkan_ash::VulkanAshRenderModule;

use newengine_platform_winit::app::config::WinitAppIcon;
use newengine_platform_winit::{run_winit_app_with_config, WinitAppConfig, WinitWindowPlacement};

use newengine_ui::markup::UiMarkupDoc;
use newengine_ui::UiBuildFn;

use std::sync::{Arc, Mutex};

mod render_controller;
mod ui;

const FIXED_DT_MS: u32 = 16;
const UI_MARKUP_PATH: &str = "ui/editor.xml";

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

#[inline]
fn winit_config_from_startup(startup: &StartupConfig) -> WinitAppConfig {
    let placement = match startup.window_placement {
        newengine_core::startup::WindowPlacement::Default => WinitWindowPlacement::OsDefault,
        newengine_core::startup::WindowPlacement::Centered { offset } => {
            WinitWindowPlacement::Centered { offset }
        }
    };

    let icon = startup.window_icon_png.as_deref().and_then(|png| {
        match WinitAppIcon::from_png_bytes(png) {
            Ok(icon) => Some(icon),
            Err(e) => {
                log::warn!("failed to decode window icon: {e}");
                None
            }
        }
    });

    WinitAppConfig {
        title: startup.window_title.clone(),
        size: startup.window_size,
        placement,
        ui_backend: startup.ui_backend.clone(),
        icon,
    }
}

#[inline]
fn register_render_from_startup(engine: &mut Engine<()>, startup: &StartupConfig) -> EngineResult<()> {
    let backend = startup.render_backend.trim();

    if backend.eq_ignore_ascii_case("vulkan_ash") || backend.eq_ignore_ascii_case("vulkan") {
        engine.register_module(Box::new(VulkanAshRenderModule::new()))?;

        engine.register_module(Box::new(
            render_controller::EditorRenderController::new(startup.render_clear_color),
        ))?;

        return Ok(());
    }

    Err(EngineError::other(format!(
        "unsupported render backend '{backend}'"
    )))
}

fn build_engine_from_startup(startup: &StartupConfig) -> EngineResult<Engine<()>> {
    let (tx, rx) = unbounded::<()>();
    let bus: Bus<()> = Bus::new(tx, rx);

    let services: Box<dyn Services> = Box::new(AppServices::new());
    let shutdown = ShutdownToken::new();

    let assets = AssetManagerConfig::new(startup.assets_root.clone())
        .with_pump_steps(startup.asset_pump_steps)
        .with_filesystem_source(startup.asset_filesystem_source);

    let config =
        EngineConfig::new(FIXED_DT_MS, assets).with_plugins_dir(Some(startup.modules_dir.clone()));

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

    let startup = Arc::new(startup);

    let engine = build_engine_from_startup(&startup)?;
    let winit_cfg = winit_config_from_startup(&startup);

    // UI builder exists immediately; document is loaded after importers are ready.
    let shared_doc: Arc<Mutex<Option<UiMarkupDoc>>> = Arc::new(Mutex::new(None));
    let ui_build: Option<Box<dyn UiBuildFn>> = match startup.ui_backend {
        newengine_core::startup::UiBackend::Disabled => None,
        _ => Some(Box::new(ui::EditorUiBuild::new(shared_doc.clone()))),
    };

    let startup_for_after = Arc::clone(&startup);

    run_winit_app_with_config(engine, winit_cfg, ui_build, move |engine| {
        let startup = &startup_for_after;

        // 1) Register render (backend + controller).
        register_render_from_startup(engine, startup)?;

        // 2) Load plugins/importers.
        engine.load_plugins_once()?;

        // 3) Importer for .xml must be registered now -> load markup via AssetManager.
        if !matches!(startup.ui_backend, newengine_core::startup::UiBackend::Disabled) {
            let am = engine
                .resources
                .get::<newengine_core::assets::AssetManager>()
                .ok_or_else(|| EngineError::other("AssetManager missing in engine.resources"))?;

            let store = am.store();

            let mut pump = || {
                am.pump();
            };

            let doc = UiMarkupDoc::load_from_store(
                store,
                &mut pump,
                UI_MARKUP_PATH,
                std::time::Duration::from_millis(250),
            )
                .map_err(|e| EngineError::other(format!("ui: load failed: {e}")))?;

            if let Ok(mut g) = shared_doc.lock() {
                *g = Some(doc);
            }
        }

        Ok(())
    })?;

    println!("engine stopped");
    Ok(())
}