#![forbid(unsafe_op_in_unsafe_fn)]

use crossbeam_channel::unbounded;

use abi_stable::std_types::{ROption, RVec};
use newengine_core::{
    Bus, ConfigPaths, Engine, EngineConfig, EngineError, EngineResult, ModuleFaultTolerance,
    PluginFaultTolerance, Services, ShutdownToken, StartupConfig, StartupLoader,
};

use newengine_modules_render_vulkan_ash::VulkanAshRenderModule;
use newengine_platform_api::PlatformAppIconV1;

use newengine_assets::{wait_ready, AssetAccess, AssetService, AssetServiceClient};
use newengine_ui::{UiBuildFn, UiMarkupDoc, UiProviderKind};

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod editor_camera;
mod material_pipeline;
mod platform_input;
mod platform_runtime;
mod plugin_manager;
mod render_controller;
mod scene_bootstrap;
mod scene_bridge;
mod scene_io_service;
mod shared;
mod ui;
mod ui_contrib;
mod viewport_bridge;

const FIXED_DT_MS: u32 = 16;
const UI_MARKUP_PATH: &str = "ui/editor.xml";

struct AppServices {
    registry: newengine_core::ServiceRegistry,
}

impl AppServices {
    #[inline]
    fn new() -> Self {
        Self {
            registry: newengine_core::ServiceRegistry::new(),
        }
    }
}

impl Services for AppServices {
    #[inline]
    fn logger(&self) -> &dyn log::Log {
        log::logger()
    }

    #[inline]
    fn service_registry(&self) -> &newengine_core::ServiceRegistry {
        &self.registry
    }
}


#[inline]
fn ui_provider_kind_from_startup(startup: &StartupConfig) -> UiProviderKind {
    match startup.ui_backend {
        newengine_core::startup::UiBackend::Disabled => UiProviderKind::Null,
        _ => UiProviderKind::Egui,
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

    let app_services = AppServices::new();
    newengine_transform::service::register(app_services.service_registry());
    let services: Box<dyn Services> = Box::new(app_services);
    let shutdown = ShutdownToken::new();

    let config = EngineConfig::new(FIXED_DT_MS)
        .with_plugins_dir(Some(startup.modules_dir.clone()))
        .with_module_fault_tolerance(ModuleFaultTolerance::Strict)
        .with_plugin_fault_tolerance(PluginFaultTolerance::Strict);

    let engine: Engine<()> = Engine::new_with_config(config, services, bus, shutdown)?;
    Ok(engine)
}

fn collect_editor_asset_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();

    if let Ok(p) = std::env::var("NEWENGINE_EDITOR_ASSETS_DIR") {
        roots.push(PathBuf::from(p));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.join("assets"));
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..6 {
            let Some(base) = cur.clone() else { break };

            let shared_assets = base.join("assets");
            if shared_assets.is_dir() {
                roots.push(shared_assets);
            }

            let cand = base.join("apps").join("editor").join("assets");
            if cand.is_dir() {
                roots.push(cand);
                break;
            }
            cur = base.parent().map(|p| p.to_path_buf());
        }
    }

    let mut out: Vec<PathBuf> = Vec::new();
    let mut set: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    for r in roots {
        if set.insert(r.clone()) {
            out.push(r);
        }
    }
    out
}

fn mount_asset_roots_best_effort(assets: &AssetServiceClient, roots: &[PathBuf]) {
    fn try_mount(assets: &AssetServiceClient, path: &std::path::Path) {
        if !path.is_dir() {
            return;
        }

        let p = path.to_string_lossy().to_string();
        if let Err(e) = assets.mount_dir(&p) {
            log::warn!(
                "editor startup: asset.mount_dir failed path='{}' err='{}'",
                path.display(),
                e
            );
        }
    }

    for r in roots.iter() {
        try_mount(assets, r);
    }
}

fn try_load_window_icon_best_effort(
    icon_path: Option<&str>,
    assets: Option<&AssetServiceClient>,
    roots: &[PathBuf],
) -> Option<PlatformAppIconV1> {
    let Some(path) = icon_path else {
        return None;
    };

    if let Some(assets) = assets {
        let id_hex32 = match assets.load(path) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("window icon: asset.load failed path='{path}' err='{e}'");
                return None;
            }
        };

        if let Err(e) = wait_ready(assets, &id_hex32, Duration::from_millis(500)) {
            log::warn!("window icon: wait_ready failed path='{path}' err='{e:?}'");
            return None;
        }

        let (_meta_json, payload) = match assets.blob_wire_v1(&id_hex32) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("window icon: blob_wire_v1 failed path='{path}' err='{e}'");
                return None;
            }
        };

        return decode_window_icon(&payload, path);
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    let p = PathBuf::from(path);

    if p.is_absolute() {
        candidates.push(p);
    } else {
        candidates.push(PathBuf::from(path));
        for r in roots.iter() {
            candidates.push(r.join(path));
        }
    }

    for c in candidates {
        if !c.is_file() {
            continue;
        }

        match std::fs::read(&c) {
            Ok(bytes) => {
                if let Some(icon) = decode_window_icon(&bytes, &c.to_string_lossy()) {
                    return Some(icon);
                }
            }
            Err(e) => {
                log::warn!(
                    "window icon: read failed file='{}' err='{}'",
                    c.display(),
                    e
                );
            }
        }
    }

    None
}

fn decode_window_icon(bytes: &[u8], label: &str) -> Option<PlatformAppIconV1> {
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            Some(PlatformAppIconV1 {
                rgba: RVec::from(rgba.into_raw()),
                width,
                height,
            })
        }
        Err(e) => {
            log::warn!("window icon: decode failed path='{}' err='{}'", label, e);
            None
        }
    }
}

#[inline]
fn shard_log_path_by_run_id(original: &str, run_id: &str) -> Option<String> {
    use std::path::Path;

    let s = original.trim();
    if s.is_empty() {
        return None;
    }

    let p = Path::new(s);
    let parent = p.parent();
    let file_name = p.file_name()?.to_string_lossy();
    let (stem, ext) = match (p.file_stem(), p.extension()) {
        (Some(stem), Some(ext)) => (stem.to_string_lossy(), Some(ext.to_string_lossy())),
        (Some(stem), None) => (stem.to_string_lossy(), None),
        _ => return None,
    };

    let new_file = match ext.as_deref() {
        Some("log") => format!("{stem}.{run_id}.log"),
        Some(e) if !e.is_empty() => format!("{stem}.{run_id}.{e}"),
        _ => format!("{file_name}.{run_id}.log"),
    };

    Some(
        parent
            .map(|d| d.join(&new_file).to_string_lossy().to_string())
            .unwrap_or(new_file),
    )
}

fn main() {
    if let Err(e) = main_impl() {
        if !matches!(e, EngineError::ExitRequested) {
            let _ = newengine_core::EngineErrorReporter::report_fatal_engine_error(&e);
            log::error!("editor fatal: {e}");
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        std::process::exit(0);
    }
}

fn main_impl() -> EngineResult<()> {
    let run_id = newengine_core::init_run_id().to_owned();
    std::env::set_var("NEWENGINE_RUN_ID", &run_id);

    newengine_core::EngineErrorReporter::install(newengine_core::EngineErrorReporterConfig {
        crash: newengine_core::crash::CrashReporterConfig {
            product_name: "NewEngine".to_owned(),
            app_name: "editor".to_owned(),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            ..Default::default()
        },
        ..Default::default()
    });

    let paths = ConfigPaths::from_startup_str("config.json");
    let (startup, _report) = StartupLoader::load_json(&paths)?;
    let startup = Arc::new(startup);

    if std::env::var_os("NEWENGINE_LOG_FILE").is_none() {
        if let Some(p) = startup
            .plugins
            .get("newengine.logging")
            .and_then(|v| {
                v.get("file")
                    .and_then(|x| x.as_str())
                    .or_else(|| v.get("file_path").and_then(|x| x.as_str()))
            })
        {
            if let Some(sharded) = shard_log_path_by_run_id(p, &run_id) {
                std::env::set_var("NEWENGINE_LOG_FILE", sharded);
            }
        }
    }

    let asset_roots = collect_editor_asset_roots();

    let viewport = Arc::new(viewport_bridge::ViewportBridge::new());
    let plugins = Arc::new(plugin_manager::PluginManagerBridge::new());
    let scene = Arc::new(scene_bridge::SceneBridge::new(newengine_scene::Scene::new()));
    let previews = Arc::new(parking_lot::Mutex::new(
        newengine_previews::PrimitivePreviewService::new(),
    ));

    let mut engine = build_engine_from_startup(&startup)?;

    register_render_from_startup(
        &mut engine,
        &startup,
        viewport.clone(),
        plugins.clone(),
        scene.clone(),
        previews.clone(),
    )?;

    engine.preload_bootstrap_plugins()?;
    engine.emit_plugins_diagnostics("after bootstrap preload");
    scene_io_service::register_scene_io_best_effort(scene.clone());

    let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let assets_available =
        newengine_plugin_host::has_service(newengine_assets::consts::ASSET_SERVICE_ID);

    if assets_available {
        mount_asset_roots_best_effort(&assets, &asset_roots);
    } else {
        log::info!(
            "editor startup: AssetManager service '{}' is not available during bootstrap/platform init; using filesystem fallback for early assets",
            newengine_assets::consts::ASSET_SERVICE_ID
        );
    }

    let runtime_path = platform_runtime::detect_platform_runtime_path(&startup.modules_dir)?;
    let resolved_platform =
        platform_runtime::resolve_platform_runtime_config(&startup, &runtime_path)?;

    log::info!(
        "editor startup: platform runtime plugin id='{}' path='{}'",
        resolved_platform.plugin_id,
        runtime_path.display()
    );

    let icon = try_load_window_icon_best_effort(
        resolved_platform.icon_path.as_deref(),
        if assets_available { Some(&assets) } else { None },
        &asset_roots,
    );

    let mut platform_cfg = resolved_platform.config;
    platform_cfg.icon = icon.map_or(ROption::RNone, ROption::RSome);

    let shared_doc: Arc<Mutex<Option<Arc<UiMarkupDoc>>>> = Arc::new(Mutex::new(None));
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

    if ui_build.is_some() {
        let assets_opt: Option<&dyn newengine_ui::AssetAccess> =
            if assets_available { Some(&assets) } else { None };

        match UiMarkupDoc::load_best_effort(
            assets_opt,
            &asset_roots,
            UI_MARKUP_PATH,
            Duration::from_millis(250),
        ) {
            Ok(doc) => {
                if let Ok(mut g) = shared_doc.lock() {
                    *g = Some(Arc::new(doc));
                }
            }
            Err(e) => {
                log::warn!(
                    "ui markup: load failed path='{}' err='{}' (degraded mode)",
                    UI_MARKUP_PATH,
                    e
                );
            }
        }
    }

    log::info!("editor: selected platform runtime {}", runtime_path.display());
    let runtime = platform_runtime::EditorPlatformRuntime::new(
        engine,
        ui_provider_kind_from_startup(&startup),
        ui_build,
    );

    runtime.run(&runtime_path, platform_cfg)?;

    log::info!("engine stopped");
    Ok(())
}
