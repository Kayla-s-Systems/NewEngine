#![forbid(unsafe_op_in_unsafe_fn)]

use crossbeam_channel::unbounded;

use newengine_core::{
    Bus, ConfigPaths, Engine, EngineConfig, EngineError, EngineResult, Services, ShutdownToken,
    StartupConfig, StartupLoader,
};

use newengine_modules_logging::{ConsoleLoggerConfig, ConsoleLoggerModule};
use newengine_modules_render_vulkan_ash::VulkanAshRenderModule;

use newengine_platform_winit::app::config::WinitAppIcon;
use newengine_platform_winit::{run_winit_app_with_config, WinitAppConfig, WinitWindowPlacement};

use newengine_ui::asset_access::wait_ready;
use newengine_ui::markup::UiMarkupDoc;
use newengine_ui::UiBuildFn;
use newengine_ui::{AssetAccess, AssetServiceClient};

use std::sync::{Arc, Mutex};
use std::time::Duration;

mod render_controller;
mod ui;
mod viewport_bridge;
mod plugin_manager;
mod shared;
mod scene_components;

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
fn register_render_from_startup(
    engine: &mut Engine<()>,
    startup: &StartupConfig,
    viewport: std::sync::Arc<viewport_bridge::ViewportBridge>,
    plugins: std::sync::Arc<plugin_manager::PluginManagerBridge>,
) -> EngineResult<()> {
    let backend = startup.render_backend.trim();

    if backend.eq_ignore_ascii_case("vulkan_ash") || backend.eq_ignore_ascii_case("vulkan") {
        engine.register_module(Box::new(VulkanAshRenderModule::new()))?;

        engine.register_module(Box::new(render_controller::EditorRenderController::new(
            startup.render_clear_color,
            viewport,
            plugins,
        )))?;

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

    let config = EngineConfig::new(FIXED_DT_MS).with_plugins_dir(Some(startup.modules_dir.clone()));

    let mut engine: Engine<()> = Engine::new_with_config(config, services, bus, shutdown)?;

    // The logger module installs the global logger in `init()`. We still bootstrap logging
    // before Engine::start() so early plugin logs are visible.
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

fn try_load_window_icon(startup: &StartupConfig) -> Option<WinitAppIcon> {
    let Some(path) = startup.window_icon_path.as_deref() else {
        return None;
    };

    // AssetManager is a plugin now; load via service client.
    let assets = AssetServiceClient::new(newengine_core::plugins::default_host_api());

    let id_hex32 = match assets.load(path) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("window icon: asset.load failed path='{path}' err='{e}'");
            return None;
        }
    };

    match wait_ready(&assets, &id_hex32, Duration::from_millis(500)) {
        Ok(()) => {}
        Err(e) => {
            log::warn!("window icon: wait_ready failed path='{path}' err='{e:?}'");
            return None;
        }
    }

    let (_meta_json, payload) = match assets.blob_wire_v1(&id_hex32) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("window icon: blob_wire_v1 failed path='{path}' err='{e}'");
            return None;
        }
    };

    match WinitAppIcon::from_png_bytes(&payload) {
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

    // Bootstrap logging as early as possible, before any plugin activity.
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

    let viewport = std::sync::Arc::new(viewport_bridge::ViewportBridge::new());
    let plugins = std::sync::Arc::new(plugin_manager::PluginManagerBridge::new());

    let mut engine = build_engine_from_startup(&startup)?;

    // 1) Register render (backend + controller) so the module set is complete before window creation.
    register_render_from_startup(&mut engine, &startup, viewport.clone(), plugins.clone())?;

    // 2) Load plugins BEFORE creating winit (required: providers must exist).
    engine.load_plugins_once()?;

    // 3) Resolve window icon via AssetManager service.
    let icon = try_load_window_icon(&startup);

    let mut winit_cfg = winit_config_from_startup(&startup);
    winit_cfg.icon = icon;

    // UI builder exists immediately; document is loaded after plugins are ready.
    let shared_doc: Arc<Mutex<Option<UiMarkupDoc>>> = Arc::new(Mutex::new(None));
    let ui_build: Option<Box<dyn UiBuildFn>> = match startup.ui_backend {
        newengine_core::startup::UiBackend::Disabled => None,
        _ => Some(Box::new(ui::EditorUiBuild::new(
            shared_doc.clone(),
            viewport.clone(),
            plugins.clone(),
        ))),
    };


    // Load markup via AssetManager service (no AssetStore in-process).
    if !matches!(startup.ui_backend, newengine_core::startup::UiBackend::Disabled) {
        let assets = AssetServiceClient::new(newengine_core::plugins::default_host_api());

        let doc = UiMarkupDoc::load(&assets, UI_MARKUP_PATH, Duration::from_millis(250))
            .map_err(|e| EngineError::other(format!("ui: load failed: {e}")))?;

        if let Ok(mut g) = shared_doc.lock() {
            *g = Some(doc);
        }
    }

    let startup_for_after = Arc::clone(&startup);

    run_winit_app_with_config(engine, winit_cfg, ui_build, move |_engine| {
        // Window-dependent work is handled by modules via WinitWindowHandles.
        // Keep this closure intentionally minimal.
        let _startup = &startup_for_after;
        Ok(())
    })?;

    println!("engine stopped");
    Ok(())
}
