#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;
use std::time::Duration;

use abi_stable::std_types::ROption;
use newengine_assets::AssetServiceClient;
use newengine_core::{ConfigPaths, EngineError, EngineResult, StartupLoader};
use newengine_editor_runtime::{
    EditorRuntimeProfile, EDITOR_APP_ASSETS_DIR_ENV, EDITOR_APP_DIR_NAME, EDITOR_FIXED_DT_MS,
    EDITOR_UI_MARKUP_PATH,
};
use newengine_runtime_host::{
    asset_bootstrap::{
        collect_app_asset_roots, mount_asset_roots_best_effort, shard_log_path_by_run_id,
        try_load_window_icon_best_effort,
    },
    engine_factory::{build_engine_from_startup, ui_provider_kind_from_startup},
    platform_runtime::{
        detect_platform_runtime_path, resolve_platform_runtime_config, HostPlatformRuntime,
    },
};

#[derive(Clone, Debug)]
pub struct LauncherEnv {
    pub key: &'static str,
    pub value: &'static str,
}

#[derive(Clone, Debug)]
pub struct LauncherConfig {
    pub app_name: &'static str,
    pub log_prefix: &'static str,
    pub app_version: &'static str,
    pub stopped_message: &'static str,
    pub env: Vec<LauncherEnv>,
}

impl LauncherConfig {
    #[inline]
    pub fn editor(app_version: &'static str) -> Self {
        Self {
            app_name: "editor",
            log_prefix: "editor launcher",
            app_version,
            stopped_message: "engine stopped",
            env: Vec::new(),
        }
    }

    #[inline]
    pub fn game_ready_fps(app_version: &'static str) -> Self {
        Self {
            app_name: "game-ready-fps",
            log_prefix: "game-ready fps launcher",
            app_version,
            stopped_message: "game-ready fps stopped",
            env: vec![LauncherEnv {
                key: "NEWENGINE_GAME_READY_DEMO",
                value: "1",
            }],
        }
    }
}

pub fn run_with_process_exit(config: LauncherConfig) {
    newengine_core::crash::record_breadcrumb(format!("{}: main entry", config.log_prefix));

    if let Err(e) = run(config.clone()) {
        if !matches!(e, EngineError::ExitRequested) {
            newengine_core::crash::record_breadcrumb(format!(
                "{}: fatal error='{}'",
                config.log_prefix, e
            ));
            let report = newengine_core::EngineErrorReporter::report_fatal_engine_error(&e);
            match report {
                Some(path) => log::error!(
                    "{} fatal: {} | crash_report='{}'",
                    config.log_prefix,
                    e,
                    path.display()
                ),
                None => log::error!("{} fatal: {e}", config.log_prefix),
            }
            eprintln!("Error: {e}");
            std::process::exit(1);
        }

        newengine_core::crash::record_breadcrumb(format!(
            "{}: exit requested",
            config.log_prefix
        ));
        std::process::exit(0);
    }
}

pub fn run(config: LauncherConfig) -> EngineResult<()> {
    let run_id = newengine_core::init_run_id().to_owned();
    newengine_core::crash::record_breadcrumb(format!(
        "{}: run start run_id={}",
        config.log_prefix, run_id
    ));

    std::env::set_var("NEWENGINE_RUN_ID", &run_id);
    for env in &config.env {
        std::env::set_var(env.key, env.value);
    }

    newengine_core::EngineErrorReporter::install(newengine_core::EngineErrorReporterConfig {
        crash: newengine_core::crash::CrashReporterConfig {
            product_name: "NewEngine".to_owned(),
            app_name: config.app_name.to_owned(),
            app_version: config.app_version.to_owned(),
            ..Default::default()
        },
        ..Default::default()
    });

    let paths = ConfigPaths::from_startup_str("config.json");
    let (startup, _report) = StartupLoader::load_json(&paths)?;
    let startup = Arc::new(startup);
    newengine_core::crash::record_breadcrumb(format!(
        "{}: startup config loaded",
        config.log_prefix
    ));

    install_log_file_override(&startup, &run_id);

    let asset_roots = collect_app_asset_roots(EDITOR_APP_DIR_NAME, EDITOR_APP_ASSETS_DIR_ENV);
    let profile = EditorRuntimeProfile::new();
    profile.install_plugin_root_editor_auto_wiring(true);

    let mut engine = build_engine_from_startup(&startup, EDITOR_FIXED_DT_MS)?;
    newengine_core::crash::record_breadcrumb(format!(
        "{}: host engine constructed",
        config.log_prefix
    ));

    profile.register_modules(&mut engine, &startup)?;
    newengine_core::crash::record_breadcrumb(format!(
        "{}: editor runtime profile registered",
        config.log_prefix
    ));

    engine.preload_bootstrap_plugins()?;
    profile.register_scene_io_best_effort();
    newengine_core::crash::record_breadcrumb(format!(
        "{}: bootstrap plugins preloaded",
        config.log_prefix
    ));

    let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let assets_available =
        newengine_plugin_host::has_service(newengine_assets::consts::ASSET_SERVICE_ID);

    if assets_available {
        mount_asset_roots_best_effort(&assets, &asset_roots);
    } else {
        log::info!(
            "{}: AssetManager service '{}' is not available during bootstrap/platform init; using filesystem fallback for early assets",
            config.log_prefix,
            newengine_assets::consts::ASSET_SERVICE_ID
        );
    }

    let runtime_path = detect_platform_runtime_path(&startup.modules_dir)?;
    newengine_core::crash::record_breadcrumb(format!(
        "{}: platform runtime detected path='{}'",
        config.log_prefix,
        display_abs_path(&runtime_path)
    ));

    let mut resolved_platform = resolve_platform_runtime_config(&startup, &runtime_path)?;
    newengine_core::crash::record_breadcrumb(format!(
        "{}: platform runtime resolved id='{}'",
        config.log_prefix, resolved_platform.plugin_id
    ));

    log::info!(
        "{}: platform runtime plugin id='{}' path='{}'",
        config.log_prefix,
        resolved_platform.plugin_id,
        display_abs_path(&runtime_path)
    );

    let icon = try_load_window_icon_best_effort(
        resolved_platform.icon_path.as_deref(),
        if assets_available { Some(&assets) } else { None },
        &asset_roots,
    );

    let mut platform_cfg = resolved_platform.config.clone();
    platform_cfg.icon = icon.map_or(ROption::RNone, ROption::RSome);

    let ui_build = profile.ui_build_from_startup(&startup);
    if ui_build.is_some() {
        let assets_opt: Option<&dyn newengine_assets::AssetAccess> =
            if assets_available { Some(&assets) } else { None };
        profile.load_markup_best_effort(
            assets_opt,
            &asset_roots,
            EDITOR_UI_MARKUP_PATH,
            Duration::from_millis(250),
        );
    }

    resolved_platform.config = platform_cfg;

    let runtime = HostPlatformRuntime::new(engine, ui_provider_kind_from_startup(&startup), ui_build);

    newengine_core::crash::record_breadcrumb(format!(
        "{}: entering host platform runtime",
        config.log_prefix
    ));
    runtime.run(&runtime_path, &resolved_platform)?;

    log::info!("{}", config.stopped_message);
    Ok(())
}

#[inline]
fn install_log_file_override(startup: &newengine_core::StartupConfig, run_id: &str) {
    if std::env::var_os("NEWENGINE_LOG_FILE").is_some() {
        return;
    }

    let Some(path) = startup.plugins.get("newengine.logging").and_then(|v| {
        v.get("file")
            .and_then(|x| x.as_str())
            .or_else(|| v.get("file_path").and_then(|x| x.as_str()))
    }) else {
        return;
    };

    if let Some(sharded) = shard_log_path_by_run_id(path, run_id) {
        std::env::set_var("NEWENGINE_LOG_FILE", sharded);
    }
}

#[inline]
fn display_abs_path(path: &std::path::Path) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = canonical.to_string_lossy();
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    let s = s.strip_prefix("//?/").unwrap_or(s);
    s.replace('\\', "/")
}
