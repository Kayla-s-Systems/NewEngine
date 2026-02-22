#![forbid(unsafe_op_in_unsafe_fn)]

use crossbeam_channel::unbounded;

use newengine_core::{
    Bus, ConfigPaths, Engine, EngineConfig, EngineError, EngineResult, Services, ShutdownToken,
    StartupConfig, StartupLoader,
};

use newengine_modules_render_vulkan_ash::VulkanAshRenderModule;

use newengine_platform_winit::app::config::WinitAppIcon;
use newengine_platform_winit::{run_winit_app_with_config, WinitAppConfig, WinitWindowPlacement};

use newengine_assets::asset_access::AssetService;
use newengine_assets::{wait_ready, AssetAccess, AssetServiceClient};
use newengine_ui::markup::UiMarkupDoc;
use newengine_ui::UiBuildFn;

use std::sync::{Arc, Mutex};
use std::time::Duration;

mod render_controller;
mod ui;
mod ui_contrib;
mod viewport_bridge;
mod plugin_manager;
mod scene_bridge;
mod scene_bootstrap;
mod editor_camera;
mod shared;

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
    viewport: Arc<viewport_bridge::ViewportBridge>,
    plugins: Arc<plugin_manager::PluginManagerBridge>,
    scene: Arc<scene_bridge::SceneBridge>,
    previews: Arc<parking_lot::Mutex<newengine_previews::PrimitivePreviewService>>,
) -> EngineResult<()> {
    let backend = startup.render_backend.trim();

    if backend.eq_ignore_ascii_case("vulkan_ash") || backend.eq_ignore_ascii_case("vulkan") {
        engine.register_module(Box::new(VulkanAshRenderModule::new()))?;

        engine.register_module(Box::new(render_controller::EditorRenderController::new(
            startup.render_clear_color,
            viewport,
            plugins,
            scene,
            previews,
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

    let config = EngineConfig::new(FIXED_DT_MS)
        .with_plugins_dir(Some(startup.modules_dir.clone()));
        // IMPORTANT:
        // StartupLoader already applied config.json overrides.
        // Engine must initialize process-wide logging from that resolved startup config,
        // otherwise defaults/env vars will silently win.
    // .with_startup_logging(startup.logging.clone());

    let engine: Engine<()> = Engine::new_with_config(config, services, bus, shutdown)?;

    Ok(engine)
}


#[inline]

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
    let (startup, _report) = StartupLoader::load_json(&paths)?;

    let startup = Arc::new(startup);

    let viewport = Arc::new(viewport_bridge::ViewportBridge::new());
    let plugins = Arc::new(plugin_manager::PluginManagerBridge::new());
    let scene = Arc::new(scene_bridge::SceneBridge::new(newengine_scene::Scene::new()));
    let previews = Arc::new(parking_lot::Mutex::new(newengine_previews::PrimitivePreviewService::new()));

    let mut engine = build_engine_from_startup(&startup)?;

    // 1) Register render (backend + controller) so the module set is complete before window creation.
    register_render_from_startup(
        &mut engine,
        &startup,
        viewport.clone(),
        plugins.clone(),
        scene.clone(),
        previews.clone(),
    )?;

    // 2) Load plugins BEFORE creating winit (required: providers must exist).
    engine.load_plugins_once()?;

    // 2.1) Mount editor-local assets directory into AssetManager.
    // This keeps the editor self-contained: icons, UI markup, shader fallbacks, etc.
    {
        let assets = AssetServiceClient::new(newengine_core::plugins::default_host_api());

        fn try_mount(assets: &AssetServiceClient, path: &std::path::Path) {
            if !path.is_dir() {
                return;
            }
            let p = path.to_string_lossy().to_string();
            match assets.mount_dir(&p) {
                Ok(()) => {
                    log::debug!("asset.mount_dir ok path='{p}'");
                }
                Err(e) => {
                    log::warn!("asset.mount_dir failed path='{p}' err='{e}'");
                }
            }
        }

        // 1) Explicit override.
        if let Ok(p) = std::env::var("NEWENGINE_EDITOR_ASSETS_DIR") {
            try_mount(&assets, std::path::Path::new(&p));
        }

        // 2) Next to the executable.
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                try_mount(&assets, &dir.join("assets"));
            }
        }

        // 3) Dev mode: search for `apps/editor/assets` upwards.
        if let Ok(exe) = std::env::current_exe() {
            let mut cur = exe.parent().map(|p| p.to_path_buf());
            for _ in 0..6 {
                let Some(base) = cur.clone() else { break };
                let cand = base.join("apps").join("editor").join("assets");
                if cand.is_dir() {
                    try_mount(&assets, &cand);
                    break;
                }
                cur = base.parent().map(|p| p.to_path_buf());
            }
        }
    }

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
            scene.clone(),
            previews.clone(),
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

    log::info!("engine stopped");
    Ok(())
}