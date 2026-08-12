use std::sync::Arc;

use newengine_assets::AssetServiceClient;
use newengine_core::{ConfigPaths, Engine, EngineResult, StartupConfig, StartupLoader};

use crate::{
    asset_bootstrap::{collect_app_asset_roots, mount_asset_roots_best_effort},
    engine_factory::build_engine_from_startup,
};

use super::boot_options::apply_declared_boot_options_env;
use super::types::{RuntimeHostAppProfile, RuntimeHostLauncher};

impl<P> RuntimeHostLauncher<P>
where
    P: RuntimeHostAppProfile,
{
    pub fn run(&self) -> EngineResult<()> {
        self.early_log(format_args!("run.begin app={}", self.spec.app_name));
        let run_id = newengine_core::init_run_id().to_owned();
        self.bind_early_log_to_run(&run_id);
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

        let mut startup = self.load_startup_config()?;
        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: startup config loaded",
            self.spec.app_name
        ));
        self.configure_sharded_log_files(Arc::make_mut(&mut startup), &run_id);

        let asset_roots = collect_app_asset_roots(self.spec.app_dir_name, self.spec.app_assets_env);
        self.early_log(format_args!(
            "asset_roots.collected count={}",
            asset_roots.len()
        ));

        let mut engine = self.build_engine(&startup)?;
        self.initialize_profile_and_plugins(&mut engine, &startup, boot_options)?;

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

        self.launch_runtime(engine, &startup, assets_available, &assets, &asset_roots)
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
}
