#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::{Engine, EngineResult, StartupConfig};
use newengine_game_ready_profile::{
    GameReadyRuntimeProfile, RustGameDataProvider, GAME_APP_ASSETS_DIR_ENV, GAME_FIXED_DT_MS,
    GAME_READY_APP_DIR_NAME, GAME_READY_RUNTIME_ENV_DEFAULTS,
};
use newengine_runtime_host::app_launcher::{
    RuntimeHostAppProfile, RuntimeHostBootOption, RuntimeHostLaunchSpec, RuntimeHostLauncher,
};
use newengine_ui::{UiBuildFn, UiProviderKind};

const BOOT_OPTIONS: &[RuntimeHostBootOption] = &[
    RuntimeHostBootOption::RuntimeBootstrapOverlay,
    RuntimeHostBootOption::RuntimePlugins,
    RuntimeHostBootOption::PlatformWindow,
    RuntimeHostBootOption::RenderBackend,
];

#[derive(Clone)]
struct RendererDemoApp {
    profile: GameReadyRuntimeProfile,
}

impl Default for RendererDemoApp {
    #[inline]
    fn default() -> Self {
        Self {
            profile: GameReadyRuntimeProfile::new()
                .with_game_data_provider(std::sync::Arc::new(RustGameDataProvider)),
        }
    }
}

impl RendererDemoApp {
    #[inline]
    fn launch_spec() -> RuntimeHostLaunchSpec {
        RuntimeHostLaunchSpec {
            product_name: "NewEngine",
            app_name: "RendererDemo",
            app_version: env!("CARGO_PKG_VERSION"),
            startup_config_path: "config.json",
            fixed_dt_ms: GAME_FIXED_DT_MS,
            app_dir_name: GAME_READY_APP_DIR_NAME,
            app_assets_env: GAME_APP_ASSETS_DIR_ENV,
            window_title: "RendererDemo: Shaded Lighting Scene",
            early_log_file_name: "renderer-demo-early.log",
            default_profile_env: None,
            env_defaults: GAME_READY_RUNTIME_ENV_DEFAULTS,
        }
    }

    #[inline]
    fn run_process(self) -> ! {
        RuntimeHostLauncher::new(Self::launch_spec(), self).run_process()
    }
}

impl RuntimeHostAppProfile for RendererDemoApp {
    #[inline]
    fn register_modules(
        &self,
        engine: &mut Engine<()>,
        startup: &StartupConfig,
    ) -> EngineResult<()> {
        self.profile.register_modules(engine, startup)
    }

    #[inline]
    fn boot_options(&self) -> Option<&'static [RuntimeHostBootOption]> {
        Some(BOOT_OPTIONS)
    }

    #[inline]
    fn register_engine_provider_routes_best_effort(&self) {
        self.profile.register_engine_provider_routes_best_effort();
    }

    #[inline]
    fn bootstrap_content_best_effort(&self) {
        self.profile.bootstrap_content_best_effort();
    }

    #[inline]
    fn ui_build_from_startup(&self, startup: &StartupConfig) -> Option<Box<dyn UiBuildFn>> {
        self.profile.ui_build_from_startup(startup)
    }

    #[inline]
    fn ui_provider_kind_from_startup(&self, startup: &StartupConfig) -> UiProviderKind {
        self.profile.ui_provider_kind_from_startup(startup)
    }
}

fn main() {
    RendererDemoApp::default().run_process();
}
