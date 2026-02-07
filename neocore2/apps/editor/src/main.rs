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
use std::time::{Duration, Instant};

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

    WinitAppConfig {
        title: startup.window_title.clone(),
        size: startup.window_size,
        placement,
        ui_backend: startup.ui_backend.clone(),
        icon: None,
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

    // The logger module installs the global logger in `init()`. We still bootstrap logging
    // before Engine::start() so early plugin/importer logs are visible.
    engine.register_module(Box::new(ConsoleLoggerModule::new(configure_logger(startup))))?;

    Ok(engine)
}

#[inline]
fn configure_logger(startup: &StartupConfig) -> ConsoleLoggerConfig {
    let mut cfg = ConsoleLoggerConfig::from_env();

    // If NEWENGINE_LOG is set, keep it as authoritative (filter string).
    if cfg.filter.is_some() {
        return cfg;
    }

    if let Ok(level) = startup.log_level.parse::<log::LevelFilter>() {
        cfg.level = level;
    }

    cfg
}

#[inline]
fn bootstrap_logging(startup: &StartupConfig) {
    // Ensure logs are available before Engine::start() and before plugin loading.
    // The ConsoleLoggerModule will later attempt to install the logger and will no-op.
    let mut builder = env_logger::Builder::new();

    if let Ok(level) = startup.log_level.parse::<log::LevelFilter>() {
        builder.filter_level(level);
    } else {
        builder.filter_level(log::LevelFilter::Info);
    }

    let _ = builder.try_init();
}

fn load_asset_blob_with_timeout(
    engine: &Engine<()>,
    logical_path: &str,
    timeout: Duration,
) -> EngineResult<std::sync::Arc<newengine_assets::AssetBlob>> {
    use newengine_assets::AssetState;

    let am = engine
        .resources
        .get::<newengine_core::assets::AssetManager>()
        .ok_or_else(|| EngineError::other("AssetManager missing in engine.resources"))?;

    let store = am.store();
    let id = store
        .load_path(logical_path)
        .map_err(|e| EngineError::other(format!("asset.load failed path='{logical_path}' err='{e}'")))?;

    let t0 = Instant::now();

    loop {
        am.pump();

        match store.state(id) {
            AssetState::Ready => {
                let blob = store
                    .get_blob(id)
                    .ok_or_else(|| EngineError::other("asset: Ready but blob is missing"))?;
                return Ok(blob);
            }
            AssetState::Failed(e) => {
                return Err(EngineError::other(format!(
                    "asset: failed path='{logical_path}' err='{e}'"
                )));
            }
            _ => {
                if t0.elapsed() >= timeout {
                    return Err(EngineError::other(format!(
                        "asset: timeout path='{logical_path}' timeout_ms={}"
                        , timeout.as_millis()
                    )));
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

fn try_load_window_icon(engine: &Engine<()>, startup: &StartupConfig) -> Option<WinitAppIcon> {
    let Some(path) = startup.window_icon_path.as_deref() else {
        return None;
    };

    let blob = match load_asset_blob_with_timeout(engine, path, Duration::from_millis(500)) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("window icon: load failed path='{path}' err='{e}'");
            return None;
        }
    };

    match WinitAppIcon::from_png_bytes(&blob.payload) {
        Ok(icon) => Some(icon),
        Err(e) => {
            log::warn!("window icon: decode failed path='{path}' err='{e}'");
            None
        }
    }
}

fn main() -> EngineResult<()> {
    let paths = ConfigPaths::from_startup_str("config.json");
    let (startup, report) = StartupLoader::load_json(&paths)?;

    // Bootstrap logging as early as possible, before any plugin/importer activity.
    bootstrap_logging(&startup);

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

    let mut engine = build_engine_from_startup(&startup)?;

    // 1) Register render (backend + controller) so the module set is complete before window creation.
    register_render_from_startup(&mut engine, &startup)?;

    // 2) Load plugins/importers BEFORE creating winit (required: plugins/providers must exist).
    engine.load_plugins_once()?;

    // 3) Resolve window icon via AssetManager + existing importers (no new image reading logic).
    let icon = try_load_window_icon(&engine, &startup);

    let mut winit_cfg = winit_config_from_startup(&startup);
    winit_cfg.icon = icon;

    // UI builder exists immediately; document is loaded after importers are ready.
    let shared_doc: Arc<Mutex<Option<UiMarkupDoc>>> = Arc::new(Mutex::new(None));
    let ui_build: Option<Box<dyn UiBuildFn>> = match startup.ui_backend {
        newengine_core::startup::UiBackend::Disabled => None,
        _ => Some(Box::new(ui::EditorUiBuild::new(shared_doc.clone()))),
    };

    let startup_for_after = Arc::clone(&startup);

    // Importer for .xml is guaranteed to be registered now -> load markup via AssetManager.
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
            Duration::from_millis(250),
        )
            .map_err(|e| EngineError::other(format!("ui: load failed: {e}")))?;

        if let Ok(mut g) = shared_doc.lock() {
            *g = Some(doc);
        }
    }

    run_winit_app_with_config(engine, winit_cfg, ui_build, move |_engine| {
        // Window-dependent work is handled by modules via WinitWindowHandles.
        // Keep this closure intentionally minimal.
        let _startup = &startup_for_after;
        Ok(())
    })?;

    println!("engine stopped");
    Ok(())
}