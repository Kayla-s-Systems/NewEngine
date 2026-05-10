#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{ROption, RString};
use newengine_core::{
    ConfigPaths, EngineError, EngineResult, StartupLoader,
};
use newengine_game_runtime::{
    StandaloneGameRuntimeProfile, GAME_APP_ASSETS_DIR_ENV, GAME_FIXED_DT_MS,
    GAME_READY_APP_DIR_NAME,
};
use newengine_runtime_host::{
    asset_bootstrap::{
        collect_app_asset_roots, shard_log_path_by_run_id,
        try_load_window_icon_best_effort,
    },
    engine_factory::build_engine_from_startup,
    platform_runtime::{
        detect_platform_runtime_path, resolve_platform_runtime_config, HostPlatformRuntime,
    },
};

use std::sync::Arc;

#[inline]
fn display_abs_path(path: &std::path::Path) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = canonical.to_string_lossy();
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    let s = s.strip_prefix("//?/").unwrap_or(s);
    s.replace('\\', "/")
}


fn configure_default_game_ready_profile() {
    if std::env::var_os("NEWENGINE_GAME_READY_PROFILE").is_some() {
        return;
    }

    let profile_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("game_ready_highlands.scene.json");

    if profile_path.is_file() {
        std::env::set_var("NEWENGINE_GAME_READY_PROFILE", profile_path);
    }
}

fn main() {
    newengine_core::crash::record_breadcrumb("game-ready fps launcher: main entry");
    if let Err(e) = main_impl() {
        if !matches!(e, EngineError::ExitRequested) {
            newengine_core::crash::record_breadcrumb(format!(
                "game-ready fps launcher: fatal error='{}'",
                e
            ));
            let report = newengine_core::EngineErrorReporter::report_fatal_engine_error(&e);
            match report {
                Some(path) => log::error!(
                    "game-ready fps launcher fatal: {} | crash_report='{}'",
                    e,
                    path.display()
                ),
                None => log::error!("game-ready fps launcher fatal: {e}"),
            }
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        newengine_core::crash::record_breadcrumb("game-ready fps launcher: exit requested");
        std::process::exit(0);
    }
}

fn main_impl() -> EngineResult<()> {
    let run_id = newengine_core::init_run_id().to_owned();
    newengine_core::crash::record_breadcrumb(format!(
        "game-ready fps launcher: main_impl start run_id={}",
        run_id
    ));
    std::env::set_var("NEWENGINE_RUN_ID", &run_id);
    std::env::set_var("NEWENGINE_GAME_READY_DEMO", "1");
    std::env::set_var("NEWENGINE_REQUIRE_RENDER_BACKEND", "1");
    std::env::set_var("NEWENGINE_PLUGIN_TARGET", "runtime");
    configure_default_game_ready_profile();

    newengine_core::EngineErrorReporter::install(newengine_core::EngineErrorReporterConfig {
        crash: newengine_core::crash::CrashReporterConfig {
            product_name: "NewEngine".to_owned(),
            app_name: "game-ready-fps".to_owned(),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            ..Default::default()
        },
        ..Default::default()
    });

    let paths = ConfigPaths::from_startup_str("config.json");
    let (startup, _report) = StartupLoader::load_json(&paths)?;
    let startup = Arc::new(startup);
    newengine_core::crash::record_breadcrumb("game-ready fps launcher: startup config loaded");

    if std::env::var_os("NEWENGINE_LOG_FILE").is_none() {
        if let Some(path) = startup.plugins.get("newengine.logging").and_then(|v| {
            v.get("file")
                .and_then(|x| x.as_str())
                .or_else(|| v.get("file_path").and_then(|x| x.as_str()))
        }) {
            if let Some(sharded) = shard_log_path_by_run_id(path, &run_id) {
                std::env::set_var("NEWENGINE_LOG_FILE", sharded);
            }
        }
    }

    let asset_roots = collect_app_asset_roots(GAME_READY_APP_DIR_NAME, GAME_APP_ASSETS_DIR_ENV);
    let profile = StandaloneGameRuntimeProfile::new();

    let mut engine = build_engine_from_startup(&startup, GAME_FIXED_DT_MS)?;
    newengine_core::crash::record_breadcrumb("game-ready fps launcher: host engine constructed");

    profile.register_modules(&mut engine, &startup)?;
    newengine_core::crash::record_breadcrumb("game-ready fps launcher: standalone game runtime profile registered");

    engine.preload_bootstrap_plugins()?;
    profile.register_scene_io_best_effort();
    newengine_core::crash::record_breadcrumb("game-ready fps launcher: bootstrap plugins preloaded");

    // The standalone game scene is intentionally not built here.
    // AssetManager and geometryImporter are engine plugins, so the game-ready scene
    // is assembled by the runtime profile during engine.start(), after plugins are live.
    profile.bootstrap_game_ready_scene_best_effort();

    let runtime_path = detect_platform_runtime_path(&startup.modules_dir)?;
    newengine_core::crash::record_breadcrumb(format!(
        "game-ready fps launcher: platform runtime detected path='{}'",
        display_abs_path(&runtime_path)
    ));

    let mut resolved_platform = resolve_platform_runtime_config(&startup, &runtime_path)?;
    newengine_core::crash::record_breadcrumb(format!(
        "game-ready fps launcher: platform runtime resolved id='{}'",
        resolved_platform.plugin_id
    ));

    log::info!(
        "game-ready fps launcher: platform runtime plugin id='{}' path='{}'",
        resolved_platform.plugin_id,
        display_abs_path(&runtime_path)
    );

    let icon = try_load_window_icon_best_effort(
        resolved_platform.icon_path.as_deref(),
        None,
        &asset_roots,
    );

    let mut platform_cfg = resolved_platform.config.clone();
    platform_cfg.title = RString::from("KAYLA FPS: Procedural Highlands");
    platform_cfg.icon = icon.map_or(ROption::RNone, ROption::RSome);

    let ui_build = profile.ui_build_from_startup(&startup);

    resolved_platform.config = platform_cfg;

    let runtime = HostPlatformRuntime::new(
        engine,
        profile.ui_provider_kind(),
        ui_build,
    );

    newengine_core::crash::record_breadcrumb("game-ready fps launcher: entering host platform runtime");
    runtime.run(&runtime_path, &resolved_platform)?;

    log::info!("game-ready fps stopped");
    Ok(())
}
