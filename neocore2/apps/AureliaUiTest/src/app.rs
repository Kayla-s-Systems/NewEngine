use newengine_runtime_host::app_launcher::{RuntimeHostLaunchSpec, RuntimeHostLauncher};

use crate::options::{APP_ASSETS_ENV, APP_DIR_NAME, ENV_DEFAULTS, FIXED_DT_MS};
use crate::profile::AureliaUiTestApp;

pub fn launch_spec() -> RuntimeHostLaunchSpec {
    RuntimeHostLaunchSpec {
        product_name: "North Star Engine",
        app_name: "AureliaUiTest",
        app_version: env!("CARGO_PKG_VERSION"),
        startup_config_path: "apps/AureliaUiTest/config.json",
        fixed_dt_ms: FIXED_DT_MS,
        app_dir_name: APP_DIR_NAME,
        app_assets_env: APP_ASSETS_ENV,
        window_title: "North Star / Aurelia UI Test",
        early_log_file_name: "aurelia-ui-test-early.log",
        default_profile_env: None,
        env_defaults: ENV_DEFAULTS,
    }
}

pub fn run_process() -> ! {
    RuntimeHostLauncher::new(launch_spec(), AureliaUiTestApp::default()).run_process()
}
