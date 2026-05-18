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
        collect_app_asset_roots, mount_asset_roots_best_effort, shard_log_path_by_run_id,
        try_load_window_icon_best_effort,
    },
    engine_factory::{build_engine_from_startup, ui_provider_kind_from_startup},
    path_display::display_abs_path,
    platform_runtime::{
        detect_platform_runtime_path, resolve_platform_runtime_config, HostPlatformRuntime,
    },
};

use std::{fmt, io::Write, path::PathBuf, sync::{Arc, atomic::{AtomicU64, Ordering}}, time::{SystemTime, UNIX_EPOCH}};


static GAME_READY_EARLY_SEQ: AtomicU64 = AtomicU64::new(1);

fn find_neocore2_root() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.file_name().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("neocore2")) {
            return cwd;
        }
        let nested = cwd.join("NewEngine").join("neocore2");
        if nested.exists() {
            return nested;
        }
        for ancestor in cwd.ancestors() {
            if ancestor.file_name().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("neocore2")) {
                return ancestor.to_path_buf();
            }
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors() {
            if ancestor.file_name().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("neocore2")) {
                return ancestor.to_path_buf();
            }
        }
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn cache_root_from_env_or_neocore2() -> PathBuf {
    if std::env::var_os(newengine_core::CACHE_FILES_READY_ENV).is_some() {
        if let Some(path) = std::env::var_os(newengine_core::CACHE_FILES_ENV)
            .or_else(|| std::env::var_os(newengine_core::CACHE_FILES_ENV_LEGACY))
            .filter(|v| !v.as_os_str().is_empty())
        {
            return PathBuf::from(path);
        }
    }

    find_neocore2_root().join("cache")
}

fn game_ready_early_log_path_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    out.push(cache_root_from_env_or_neocore2().join("logs").join("game-ready-early.log"));

    out
}

fn game_ready_early_log(args: fmt::Arguments<'_>) {
    let seq = GAME_READY_EARLY_SEQ.fetch_add(1, Ordering::Relaxed);
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    for path in game_ready_early_log_path_candidates() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        else {
            continue;
        };
        let _ = writeln!(file, "[{now_ms}] [{seq:06}] {args}");
        let _ = file.flush();
        return;
    }
}

macro_rules! game_ready_early_log {
    ($($arg:tt)*) => {{
        game_ready_early_log(format_args!($($arg)*));
    }};
}



fn configure_default_game_ready_profile() {
    if std::env::var_os("NEWENGINE_GAME_READY_PROFILE").is_some() {
        return;
    }

    // Logical AssetManager/VFS path. Do not publish absolute filesystem paths here:
    // runtime scene text must be resolved through AssetManager after the engine
    // plugin phase has registered `asset.manager`.
    std::env::set_var("NEWENGINE_GAME_READY_PROFILE", "game_ready_highlands.scene.json");
}

fn main() {
    game_ready_early_log!("main.entry exe={:?} cwd={:?}", std::env::current_exe().ok(), std::env::current_dir().ok());
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
    game_ready_early_log!("main_impl.begin");
    let run_id = newengine_core::init_run_id().to_owned();
    game_ready_early_log!("run_id.init.ok run_id={}", run_id);
    newengine_core::crash::record_breadcrumb(format!(
        "game-ready fps launcher: main_impl start run_id={}",
        run_id
    ));
    std::env::set_var("NEWENGINE_RUN_ID", &run_id);
    std::env::set_var("NEWENGINE_GAME_READY_DEMO", "1");
    std::env::set_var("NEWENGINE_REQUIRE_RENDER_BACKEND", "1");
    std::env::set_var("NEWENGINE_REQUIRE_ASSET_MANAGER", "1");
    std::env::set_var("NEWENGINE_REQUIRE_PLATFORM_BACKEND", "1");
    std::env::set_var("NEWENGINE_PLUGIN_TARGET", "runtime");
    // Game-ready launch must not dlopen bootstrap DLLs before platform/runtime
    // diagnostics are visible. Bootstrap plugins are loaded together with the
    // engine phase; stale DLLs can otherwise terminate the process with SEH
    // STATUS_ACCESS_VIOLATION before Rust can report an error.
    std::env::set_var("NEWENGINE_BOOTSTRAP_PLUGIN_PRELOAD", "deferred");
    configure_default_game_ready_profile();

    game_ready_early_log!("error_reporter.install.begin");
    newengine_core::EngineErrorReporter::install(newengine_core::EngineErrorReporterConfig {
        crash: newengine_core::crash::CrashReporterConfig {
            product_name: "NewEngine".to_owned(),
            app_name: "game-ready-fps".to_owned(),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            // Standalone game launches should write a deterministic crash report
            // but not emit noisy "reporter not found" diagnostics unless a real
            // reporter binary is shipped/configured.
            spawn_reporter: std::env::var_os("NEWENGINE_CRASH_REPORTER_PATH").is_some(),
            ..Default::default()
        },
        ..Default::default()
    });
    game_ready_early_log!("error_reporter.install.ok");

    let paths = ConfigPaths::from_startup_str("config.json");
    game_ready_early_log!("startup.load.begin path=config.json");
    let (startup, _report) = StartupLoader::load_json(&paths)?;
    game_ready_early_log!(
        "startup.load.ok modules_dir={} cache_files={}",
        startup.modules_dir.display(),
        startup.resolved_cache_files_dir().display()
    );
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
    game_ready_early_log!("asset_roots.collected count={}", asset_roots.len());
    let profile = StandaloneGameRuntimeProfile::new();

    game_ready_early_log!("engine.build.begin");
    let mut engine = build_engine_from_startup(&startup, GAME_FIXED_DT_MS)?;
    game_ready_early_log!("engine.build.ok");
    newengine_core::crash::record_breadcrumb("game-ready fps launcher: host engine constructed");

    game_ready_early_log!("profile.register_modules.begin");
    profile.register_modules(&mut engine, &startup)?;
    game_ready_early_log!("profile.register_modules.ok");
    newengine_core::crash::record_breadcrumb("game-ready fps launcher: standalone game runtime profile registered");

    game_ready_early_log!("engine.preload_bootstrap_plugins.begin");
    engine.preload_bootstrap_plugins()?;
    game_ready_early_log!("engine.preload_bootstrap_plugins.ok");
    profile.register_scene_io_best_effort();
    profile.register_ecs_gateway_best_effort();
    profile.register_entity_gateway_best_effort();
    newengine_core::crash::record_breadcrumb("game-ready fps launcher: bootstrap plugins preloaded");

    // The standalone game scene is intentionally not built here.
    // AssetManager and geometryImporter are engine plugins, so the game-ready scene
    // is assembled by the runtime profile during engine.start(), after plugins are live.
    profile.bootstrap_game_ready_scene_best_effort();

    let assets = newengine_assets::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let assets_available =
        newengine_plugin_host::has_service(newengine_assets::consts::ASSET_SERVICE_ID);

    if assets_available {
        mount_asset_roots_best_effort(&assets, &asset_roots);
    } else {
        log::info!(
            "game-ready fps launcher: AssetManager service '{}' is not available during platform init; loading screen assets will retry through AssetManager after services are live",
            newengine_assets::consts::ASSET_SERVICE_ID
        );
    }

    game_ready_early_log!("platform.detect.begin modules_dir={}", startup.modules_dir.display());
    let runtime_path = detect_platform_runtime_path(&startup.modules_dir)?;
    game_ready_early_log!("platform.detect.ok path={}", display_abs_path(&runtime_path));
    newengine_core::crash::record_breadcrumb(format!(
        "game-ready fps launcher: platform runtime detected path='{}'",
        display_abs_path(&runtime_path)
    ));

    game_ready_early_log!("platform.config.resolve.begin");
    let mut resolved_platform = resolve_platform_runtime_config(&startup, &runtime_path)?;
    game_ready_early_log!("platform.config.resolve.ok id={} name={} version={}", resolved_platform.plugin_id, resolved_platform.plugin_name, resolved_platform.plugin_version);
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
        if assets_available { Some(&assets) } else { None },
        &asset_roots,
    );

    let mut platform_cfg = resolved_platform.config.clone();
    platform_cfg.title = RString::from("KAYLA FPS: Procedural Highlands");
    platform_cfg.icon = icon.map_or(ROption::RNone, ROption::RSome);

    let ui_build = profile.ui_build_from_startup(&startup);

    resolved_platform.config = platform_cfg;

    game_ready_early_log!("host_runtime.new.begin");
    let runtime = HostPlatformRuntime::new(
        engine,
        ui_provider_kind_from_startup(&startup),
        ui_build,
    );

    game_ready_early_log!("host_runtime.new.ok");
    newengine_core::crash::record_breadcrumb("game-ready fps launcher: entering host platform runtime");
    game_ready_early_log!("runtime.run.begin path={} id={}", display_abs_path(&runtime_path), resolved_platform.plugin_id);
    runtime.run(&runtime_path, &resolved_platform)?;
    game_ready_early_log!("runtime.run.returned");

    log::info!("game-ready fps stopped");
    Ok(())
}
