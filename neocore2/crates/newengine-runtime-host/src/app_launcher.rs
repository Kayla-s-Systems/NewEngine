#![forbid(unsafe_op_in_unsafe_fn)]

//! Declarative host-side app launcher.
//!
//! Apps should describe what they are and which runtime profile they want to run.
//! They should not manually assemble config loading, engine construction,
//! gateway/module registration, asset bootstrap, platform discovery and host
//! runtime execution in their binary entrypoint.

mod boot_options;
pub use boot_options::RuntimeHostBootOption;

use std::{
    fmt,
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use abi_stable::std_types::{ROption, RString};
use boot_options::{apply_declared_boot_options_env, boot_option_enabled};
use newengine_assets::AssetServiceClient;
use newengine_core::{
    ConfigPaths, Engine, EngineError, EngineResult, StartupConfig, StartupLoader,
};
use newengine_ui::{UiBuildFn, UiProviderKind};

use crate::{
    asset_bootstrap::{
        collect_app_asset_roots, mount_asset_roots_best_effort, shard_log_path_by_run_id,
        try_load_window_icon_best_effort,
    },
    engine_factory::{build_engine_from_startup, ui_provider_kind_from_startup},
    headless_cli::HeadlessCliRuntime,
    path_display::display_abs_path,
    platform_runtime::{
        detect_platform_runtime_path, resolve_platform_runtime_config, HostPlatformRuntime,
    },
};

static APP_LAUNCH_EARLY_SEQ: AtomicU64 = AtomicU64::new(1);

/// Product/application declaration for a standalone runtime-host launch.
#[derive(Clone, Debug)]
pub struct RuntimeHostLaunchSpec {
    pub product_name: &'static str,
    pub app_name: &'static str,
    pub app_version: &'static str,
    pub startup_config_path: &'static str,
    pub fixed_dt_ms: u32,
    pub app_dir_name: &'static str,
    pub app_assets_env: &'static str,
    pub window_title: &'static str,
    pub early_log_file_name: &'static str,
    pub default_profile_env: Option<(&'static str, &'static str)>,
    pub env_defaults: &'static [(&'static str, &'static str)],
}

impl RuntimeHostLaunchSpec {
    #[inline]
    pub fn apply_env_defaults(&self) {
        for &(key, value) in self.env_defaults {
            if std::env::var_os(key).is_none() {
                std::env::set_var(key, value);
            }
        }

        if let Some((key, value)) = self.default_profile_env {
            if std::env::var_os(key).is_none() {
                std::env::set_var(key, value);
            }
        }
    }
}

/// Game/profile-specific hooks used by the generic runtime-host launcher.
///
/// The launcher owns startup orchestration. The profile owns game/runtime
/// composition: modules, engine-runtime route bridges, UI projection and scene
/// bootstrap policy.
pub trait RuntimeHostAppProfile {
    fn register_modules(
        &self,
        engine: &mut Engine<()>,
        startup: &StartupConfig,
    ) -> EngineResult<()>;

    #[inline]
    fn boot_options(&self) -> Option<&'static [RuntimeHostBootOption]> {
        None
    }

    #[inline]
    fn register_engine_provider_routes_best_effort(&self) {}

    #[inline]
    fn bootstrap_content_best_effort(&self) {}

    #[inline]
    fn ui_build_from_startup(&self, _startup: &StartupConfig) -> Option<Box<dyn UiBuildFn>> {
        None
    }

    #[inline]
    fn ui_provider_kind_from_startup(&self, startup: &StartupConfig) -> UiProviderKind {
        ui_provider_kind_from_startup(startup)
    }
}

pub struct RuntimeHostLauncher<P> {
    spec: RuntimeHostLaunchSpec,
    profile: P,
}

impl<P> RuntimeHostLauncher<P>
where
    P: RuntimeHostAppProfile,
{
    #[inline]
    pub fn new(spec: RuntimeHostLaunchSpec, profile: P) -> Self {
        Self { spec, profile }
    }

    /// Run the app and terminate the process with the correct code.
    ///
    /// This keeps binary entrypoints thin and ensures all standalone products use
    /// the same fatal-error reporting and ExitRequested policy.
    pub fn run_process(self) -> ! {
        self.early_log(format_args!(
            "process.entry exe={:?} cwd={:?}",
            std::env::current_exe().ok(),
            std::env::current_dir().ok()
        ));
        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: main entry",
            self.spec.app_name
        ));

        match self.run() {
            Ok(()) | Err(EngineError::ExitRequested) => {
                newengine_core::crash::record_breadcrumb(format!(
                    "{} launcher: exit requested",
                    self.spec.app_name
                ));
                std::process::exit(0);
            }
            Err(e) => {
                newengine_core::crash::record_breadcrumb(format!(
                    "{} launcher: fatal error='{}'",
                    self.spec.app_name, e
                ));
                let report = newengine_core::EngineErrorReporter::report_fatal_engine_error(&e);
                match report {
                    Some(path) => newengine_ulog_api::ulog::error!(
                        "{} launcher fatal: {} | crash_report='{}'",
                        self.spec.app_name,
                        e,
                        path.display()
                    ),
                    None => newengine_ulog_api::ulog::error!(
                        "{} launcher fatal: {e}",
                        self.spec.app_name
                    ),
                }
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }

    pub fn run(&self) -> EngineResult<()> {
        self.early_log(format_args!("run.begin app={}", self.spec.app_name));
        let run_id = newengine_core::init_run_id().to_owned();
        self.early_log(format_args!("run_id.init.ok run_id={}", run_id));
        newengine_ulog_api::ulog::info_event!(
            "engine.startup.run_id",
            "Run ID initialized",
            {
                "app_name": self.spec.app_name,
                "run_id": run_id.as_str()
            }
        );
        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: run start run_id={}",
            self.spec.app_name, run_id
        ));

        std::env::set_var("NEWENGINE_RUN_ID", &run_id);
        self.spec.apply_env_defaults();
        let boot_options = self.profile.boot_options();
        apply_declared_boot_options_env(self.spec.app_name, boot_options);

        self.install_error_reporter();

        let startup = self.load_startup_config()?;
        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: startup config loaded",
            self.spec.app_name
        ));

        self.configure_sharded_log_file(&startup, &run_id);

        let asset_roots = collect_app_asset_roots(self.spec.app_dir_name, self.spec.app_assets_env);
        self.early_log(format_args!(
            "asset_roots.collected count={}",
            asset_roots.len()
        ));

        let mut engine = self.build_engine(&startup)?;
        self.profile.register_modules(&mut engine, &startup)?;
        if boot_option_enabled(boot_options, RuntimeHostBootOption::RuntimePlugins) {
            engine.preload_bootstrap_plugins()?;
        }

        self.profile.register_engine_provider_routes_best_effort();
        self.profile.bootstrap_content_best_effort();
        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: profile registered and bootstrap plugin phase evaluated",
            self.spec.app_name
        ));

        let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
        let assets_available =
            newengine_core::has_engine_gateway_route(newengine_assets_api::ENGINE_ASSET_SERVICE_ID)
                || newengine_plugin_host::has_service(newengine_assets::consts::ASSET_SERVICE_ID);
        self.early_log(format_args!(
            "asset_service.availability available={} gateway={}",
            assets_available,
            newengine_assets_api::ENGINE_ASSET_SERVICE_ID
        ));

        if assets_available {
            mount_asset_roots_best_effort(&assets, &asset_roots);
            self.early_log(format_args!(
                "asset_roots.mount.requested count={}",
                asset_roots.len()
            ));
        } else {
            newengine_ulog_api::ulog::warn!(
                "{} launcher: engine.assets route unavailable after profile registration; asset root mount skipped until provider readiness",
                self.spec.app_name
            );
            newengine_core::crash::record_breadcrumb(format!(
                "{} launcher: engine.assets unavailable during initial asset mount",
                self.spec.app_name
            ));
        }

        if self.headless_mode_requested() {
            newengine_ulog_api::ulog::warn!(
                "{} launcher: headless mode requested; skipping platform runtime discovery and entering CLI host",
                self.spec.app_name
            );
            newengine_core::crash::record_breadcrumb(format!(
                "{} launcher: explicit headless mode requested",
                self.spec.app_name
            ));
            return HeadlessCliRuntime::new(engine, self.spec.fixed_dt_ms)
                .run("NEWENGINE_HEADLESS requested");
        }

        let runtime_path = match self.detect_platform_runtime(&startup) {
            Ok(path) => path,
            Err(err) if self.platform_missing_can_fallback_to_headless(&err) => {
                newengine_ulog_api::ulog::warn!(
                    "{} launcher: platform runtime unavailable; falling back to headless CLI mode: {}",
                    self.spec.app_name,
                    err
                );
                newengine_core::crash::record_breadcrumb(format!(
                    "{} launcher: platform unavailable; entering headless CLI fallback",
                    self.spec.app_name
                ));
                return HeadlessCliRuntime::new(engine, self.spec.fixed_dt_ms).run(err.to_string());
            }
            Err(err) => return Err(err),
        };
        let mut resolved_platform = match self.resolve_platform_runtime(&startup, &runtime_path) {
            Ok(resolved) => resolved,
            Err(err) if self.platform_missing_can_fallback_to_headless(&err) => {
                newengine_ulog_api::ulog::warn!(
                    "{} launcher: platform runtime could not be resolved; falling back to headless CLI mode: {}",
                    self.spec.app_name,
                    err
                );
                newengine_core::crash::record_breadcrumb(format!(
                    "{} launcher: platform metadata/ABI unavailable; entering headless CLI fallback",
                    self.spec.app_name
                ));
                return HeadlessCliRuntime::new(engine, self.spec.fixed_dt_ms).run(err.to_string());
            }
            Err(err) => return Err(err),
        };

        newengine_ulog_api::ulog::info!(
            "{} launcher: platform runtime plugin id='{}' path='{}'",
            self.spec.app_name,
            resolved_platform.plugin_id,
            display_abs_path(&runtime_path)
        );

        let icon = try_load_window_icon_best_effort(
            resolved_platform.icon_path.as_deref(),
            if assets_available {
                Some(&assets)
            } else {
                None
            },
            &asset_roots,
        );

        let mut platform_cfg = resolved_platform.config.clone();
        platform_cfg.title = RString::from(self.spec.window_title);
        platform_cfg.icon = icon.map_or(ROption::RNone, ROption::RSome);
        resolved_platform.config = platform_cfg;

        let ui_build = self.profile.ui_build_from_startup(&startup);
        let ui_kind = self.profile.ui_provider_kind_from_startup(&startup);

        self.early_log(format_args!("host_runtime.new.begin"));
        let runtime = HostPlatformRuntime::new(engine, ui_kind, ui_build);
        self.early_log(format_args!("host_runtime.new.ok"));

        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: entering host platform runtime",
            self.spec.app_name
        ));
        self.early_log(format_args!(
            "runtime.run.begin path={} id={}",
            display_abs_path(&runtime_path),
            resolved_platform.plugin_id
        ));
        runtime.run(&runtime_path, &resolved_platform)?;
        self.early_log(format_args!("runtime.run.returned"));

        newengine_ulog_api::ulog::info!("{} stopped", self.spec.app_name);
        Ok(())
    }

    fn install_error_reporter(&self) {
        self.early_log(format_args!("error_reporter.install.begin"));
        newengine_core::EngineErrorReporter::install(newengine_core::EngineErrorReporterConfig {
            crash: newengine_core::crash::CrashReporterConfig {
                product_name: self.spec.product_name.to_owned(),
                app_name: self.spec.app_name.to_owned(),
                app_version: self.spec.app_version.to_owned(),
                spawn_reporter: std::env::var_os("NEWENGINE_CRASH_REPORTER_PATH").is_some(),
                ..Default::default()
            },
            ..Default::default()
        });
        self.early_log(format_args!("error_reporter.install.ok"));
    }

    fn load_startup_config(&self) -> EngineResult<Arc<StartupConfig>> {
        let paths = ConfigPaths::from_startup_str(self.spec.startup_config_path);
        self.early_log(format_args!(
            "startup.load.begin path={}",
            self.spec.startup_config_path
        ));
        let (startup, _report) = StartupLoader::load_json(&paths)?;
        self.early_log(format_args!(
            "startup.load.ok modules_dir={} cache_files={} config={}",
            startup.modules_dir.display(),
            startup.resolved_cache_files_dir().display(),
            startup.resolved_config_dir().display()
        ));
        Ok(Arc::new(startup))
    }

    fn configure_sharded_log_file(&self, startup: &StartupConfig, run_id: &str) {
        if std::env::var_os("NEWENGINE_LOG_FILE").is_some() {
            return;
        }

        let Some(path) = startup
            .plugins
            .get("engine.logging.chronicle")
            .and_then(|v| {
                v.get("file")
                    .and_then(|x| x.as_str())
                    .or_else(|| v.get("file_path").and_then(|x| x.as_str()))
            })
        else {
            return;
        };

        if let Some(sharded) = shard_log_path_by_run_id(path, run_id) {
            std::env::set_var("NEWENGINE_LOG_FILE", sharded);
        }
    }

    fn build_engine(&self, startup: &StartupConfig) -> EngineResult<Engine<()>> {
        self.early_log(format_args!("engine.build.begin"));
        let engine = build_engine_from_startup(startup, self.spec.fixed_dt_ms)?;
        self.early_log(format_args!("engine.build.ok"));
        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: host engine constructed",
            self.spec.app_name
        ));
        Ok(engine)
    }

    fn detect_platform_runtime(&self, startup: &StartupConfig) -> EngineResult<PathBuf> {
        self.early_log(format_args!(
            "platform.detect.begin modules_dir={}",
            startup.modules_dir.display()
        ));
        let path = detect_platform_runtime_path(&startup.modules_dir)?;
        self.early_log(format_args!(
            "platform.detect.ok path={}",
            display_abs_path(&path)
        ));
        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: platform runtime detected path='{}'",
            self.spec.app_name,
            display_abs_path(&path)
        ));
        Ok(path)
    }

    fn resolve_platform_runtime(
        &self,
        startup: &StartupConfig,
        runtime_path: &std::path::Path,
    ) -> EngineResult<crate::platform_runtime::ResolvedPlatformRuntimeConfig> {
        self.early_log(format_args!("platform.config.resolve.begin"));
        let resolved = resolve_platform_runtime_config(startup, runtime_path)?;
        self.early_log(format_args!(
            "platform.config.resolve.ok id={} name={} version={}",
            resolved.plugin_id, resolved.plugin_name, resolved.plugin_version
        ));
        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: platform runtime resolved id='{}'",
            self.spec.app_name, resolved.plugin_id
        ));
        Ok(resolved)
    }

    fn headless_mode_requested(&self) -> bool {
        if env_bool("NEWENGINE_REQUIRE_PLATFORM", false)
            || env_bool("NEWENGINE_REQUIRE_PLATFORM_BACKEND", false)
        {
            return false;
        }
        env_bool("NEWENGINE_HEADLESS", false)
    }

    fn platform_missing_can_fallback_to_headless(&self, err: &EngineError) -> bool {
        let headless_requested = env_bool("NEWENGINE_HEADLESS", false);
        if env_bool("NEWENGINE_REQUIRE_PLATFORM", false)
            || env_bool("NEWENGINE_REQUIRE_PLATFORM_BACKEND", false)
        {
            return false;
        }
        if std::env::var_os("NEWENGINE_PLATFORM_RUNTIME").is_some() && !headless_requested {
            return false;
        }
        if headless_requested {
            return true;
        }

        let text = err.to_string();
        text.contains("platform runtime DLL not found")
            || text.contains("No platform runtime")
            || text.contains("platform runtime unavailable")
            || text.contains("platform runtime metadata load failed")
            || text.contains("platform config defaults failed")
            || text.contains("platform config apply failed")
            || text.contains("platform config decode failed")
            || text.contains("platform runtime symbol missing")
            || text.contains("platform runtime load failed")
    }

    fn early_log(&self, args: fmt::Arguments<'_>) {
        let seq = APP_LAUNCH_EARLY_SEQ.fetch_add(1, Ordering::Relaxed);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let message = args.to_string();
        let payload = serde_json::json!({
            "schema": "northstar.ulog.event.v1",
            "timestamp_utc": format!("{}.{}Z", now_ms / 1000, now_ms % 1000),
            "level": "DEBUG",
            "event_id": "engine.runtime_host.early",
            "message": message,
            "source": {
                "kind": "engine",
                "name": "newengine-runtime-host"
            },
            "context": {
                "run_id": null,
                "session_id": null
            },
            "location": {
                "module": "newengine_runtime_host::app_launcher",
                "file": null,
                "line": null
            },
            "fields": {
                "app_name": self.spec.app_name,
                "early_source": self.spec.early_log_file_name,
                "sequence": seq
            }
        });
        let line = match serde_json::to_string(&payload) {
            Ok(line) => line,
            Err(_) => return,
        };

        for path in self.early_log_path_candidates() {
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
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
            return;
        }
    }

    fn early_log_path_candidates(&self) -> Vec<PathBuf> {
        vec![cache_root_from_env_or_neocore2()
            .join("logs")
            .join("current.ulog.ndjson")]
    }
}

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn cache_root_from_env_or_neocore2() -> PathBuf {
    if std::env::var_os(newengine_core::CACHE_FILES_READY_ENV).is_some() {
        if let Some(path) = std::env::var_os(newengine_core::CACHE_FILES_ENV)
            .or_else(|| std::env::var_os(newengine_core::CACHE_FILES_ALIAS_ENV))
            .filter(|v| !v.as_os_str().is_empty())
        {
            return PathBuf::from(path);
        }
    }

    crate::path_resolver::find_neocore2_root().join("cache")
}
