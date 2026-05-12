#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::ROption;
use newengine_assets::AssetServiceClient;
use newengine_core::{
    ConfigPaths, EngineError, EngineResult, StartupLoader,
};
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

use std::sync::Arc;
use std::time::Duration;

#[inline]
fn display_abs_path(path: &std::path::Path) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = canonical.to_string_lossy();
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    let s = s.strip_prefix("//?/").unwrap_or(s);
    s.replace('\\', "/")
}

fn main() {
    newengine_core::crash::record_breadcrumb("editor launcher: main entry");
    if let Err(e) = main_impl() {
        if !matches!(e, EngineError::ExitRequested) {
            newengine_core::crash::record_breadcrumb(format!(
                "editor launcher: fatal error='{}'",
                e
            ));
            let report = newengine_core::EngineErrorReporter::report_fatal_engine_error(&e);
            match report {
                Some(path) => log::error!(
                    "editor launcher fatal: {} | crash_report='{}'",
                    e,
                    path.display()
                ),
                None => log::error!("editor launcher fatal: {e}"),
            }
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        newengine_core::crash::record_breadcrumb("editor launcher: exit requested");
        std::process::exit(0);
    }
}

fn main_impl() -> EngineResult<()> {
    let run_id = newengine_core::init_run_id().to_owned();
    newengine_core::crash::record_breadcrumb(format!(
        "editor launcher: main_impl start run_id={}",
        run_id
    ));
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
    newengine_core::crash::record_breadcrumb("editor launcher: startup config loaded");

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

    let asset_roots = collect_app_asset_roots(EDITOR_APP_DIR_NAME, EDITOR_APP_ASSETS_DIR_ENV);
    let profile = EditorRuntimeProfile::new();
    profile.install_plugin_root_editor_auto_wiring(true);

    let mut engine = build_engine_from_startup(&startup, EDITOR_FIXED_DT_MS)?;
    newengine_core::crash::record_breadcrumb("editor launcher: host engine constructed");

    profile.register_modules(&mut engine, &startup)?;
    newengine_core::crash::record_breadcrumb("editor launcher: editor runtime profile registered");

    engine.preload_bootstrap_plugins()?;
    profile.register_scene_io_best_effort();
    newengine_core::crash::record_breadcrumb("editor launcher: bootstrap plugins preloaded");

    let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let assets_available =
        newengine_plugin_host::has_service(newengine_assets::consts::ASSET_SERVICE_ID);

    if assets_available {
        mount_asset_roots_best_effort(&assets, &asset_roots);
    } else {
        log::info!(
            "editor launcher: AssetManager service '{}' is not available during bootstrap/platform init; early visual assets are skipped and will be requested through AssetManager after services are live",
            newengine_assets::consts::ASSET_SERVICE_ID
        );
    }

    let runtime_path = detect_platform_runtime_path(&startup.modules_dir)?;
    newengine_core::crash::record_breadcrumb(format!(
        "editor launcher: platform runtime detected path='{}'",
        display_abs_path(&runtime_path)
    ));

    let mut resolved_platform = resolve_platform_runtime_config(&startup, &runtime_path)?;
    newengine_core::crash::record_breadcrumb(format!(
        "editor launcher: platform runtime resolved id='{}'",
        resolved_platform.plugin_id
    ));

    log::info!(
        "editor launcher: platform runtime plugin id='{}' path='{}'",
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

    let runtime = HostPlatformRuntime::new(
        engine,
        ui_provider_kind_from_startup(&startup),
        ui_build,
    );

    newengine_core::crash::record_breadcrumb("editor launcher: entering host platform runtime");
    runtime.run(&runtime_path, &resolved_platform)?;

    log::info!("engine stopped");
    Ok(())
}
