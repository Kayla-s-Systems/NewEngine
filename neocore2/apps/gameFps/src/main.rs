#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::{Engine, EngineResult, StartupConfig};
use newengine_game_ready_profile::{
    GameReadyRuntimeProfile, GAME_APP_ASSETS_DIR_ENV, GAME_FIXED_DT_MS,
    GAME_READY_DEFAULT_PROFILE_ASSET, GAME_READY_GAME_UI_ENV_DEFAULTS, GAME_READY_PROFILE_ENV,
};
use newengine_runtime_host::app_launcher::{
    RuntimeHostAppProfile, RuntimeHostBootOption, RuntimeHostLaunchSpec, RuntimeHostLauncher,
};
use newengine_ui::{UiBuildFn, UiProviderKind};

const APP_NAME: &str = "gameFps";
const BOOT_OPTIONS: &[RuntimeHostBootOption] = &[
    RuntimeHostBootOption::PreStartConfigWindow,
    RuntimeHostBootOption::RuntimeBootstrapOverlay,
    RuntimeHostBootOption::RuntimePlugins,
    RuntimeHostBootOption::PlatformWindow,
    RuntimeHostBootOption::RenderBackend,
];

#[derive(Clone)]
struct GameFpsApp {
    profile: GameReadyRuntimeProfile,
}

impl Default for GameFpsApp {
    fn default() -> Self {
        Self {
            profile: GameReadyRuntimeProfile::new().without_editor_tools(),
        }
    }
}

impl GameFpsApp {
    fn launch_spec() -> RuntimeHostLaunchSpec {
        RuntimeHostLaunchSpec {
            product_name: "North Star",
            app_name: APP_NAME,
            app_version: env!("CARGO_PKG_VERSION"),
            startup_config_path: "config.json",
            fixed_dt_ms: GAME_FIXED_DT_MS,
            app_dir_name: APP_NAME,
            app_assets_env: GAME_APP_ASSETS_DIR_ENV,
            window_title: "North Star gameFps: 3D FPS Demo",
            early_log_file_name: "gameFps-early.log",
            default_profile_env: Some((GAME_READY_PROFILE_ENV, GAME_READY_DEFAULT_PROFILE_ASSET)),
            env_defaults: GAME_READY_GAME_UI_ENV_DEFAULTS,
        }
    }

    fn run_process(self) -> ! {
        RuntimeHostLauncher::new(Self::launch_spec(), self).run_process()
    }
}

impl RuntimeHostAppProfile for GameFpsApp {
    fn register_modules(
        &self,
        engine: &mut Engine<()>,
        startup: &StartupConfig,
    ) -> EngineResult<()> {
        self.profile.register_modules(engine, startup)
    }

    fn boot_options(&self) -> Option<&'static [RuntimeHostBootOption]> {
        Some(BOOT_OPTIONS)
    }

    fn register_engine_provider_routes_best_effort(&self) {
        self.profile.register_engine_provider_routes_best_effort();
    }

    fn bootstrap_content_best_effort(&self) {
        self.profile.bootstrap_content_best_effort();
    }

    fn ui_build_from_startup(&self, startup: &StartupConfig) -> Option<Box<dyn UiBuildFn>> {
        self.profile.ui_build_from_startup(startup)
    }

    fn ui_provider_kind_from_startup(&self, startup: &StartupConfig) -> UiProviderKind {
        self.profile.ui_provider_kind_from_startup(startup)
    }
}

fn main() {
    GameFpsApp::default().run_process();
}
