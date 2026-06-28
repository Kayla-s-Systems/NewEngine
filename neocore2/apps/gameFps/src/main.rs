#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::{Engine, EngineResult, StartupConfig};
use newengine_game_ready_profile::{
    GameReadyRuntimeProfile, GAME_APP_ASSETS_DIR_ENV, GAME_FIXED_DT_MS,
};
use newengine_runtime_host::app_launcher::{
    RuntimeHostAppProfile, RuntimeHostBootOption, RuntimeHostLaunchSpec, RuntimeHostLauncher,
};
use newengine_ui::{UiBuildFn, UiProviderKind};

const APP_NAME: &str = "gameFps";
const SCENE_PROFILE_ENV: &str = "NEWENGINE_SCENE_PROFILE";
const DEFAULT_SCENE_PROFILE: &str = "game_ready_highlands.ymap";

const BOOT_OPTIONS: &[RuntimeHostBootOption] = &[
    RuntimeHostBootOption::PreStartConfigWindow,
    RuntimeHostBootOption::RuntimeBootstrapOverlay,
    RuntimeHostBootOption::RuntimePlugins,
    RuntimeHostBootOption::PlatformWindow,
    RuntimeHostBootOption::RenderBackend,
];

const ENV_DEFAULTS: &[(&str, &str)] = &[
    ("NEWENGINE_GAME_FPS_DEMO", "1"),
    ("NEWENGINE_GAME_READY_DEMO", "1"),
    ("NEWENGINE_REQUIRE_RENDER_BACKEND", "1"),
    ("NEWENGINE_REQUIRE_ASSET_MANAGER", "1"),
    ("NEWENGINE_REQUIRE_MATERIALS_BACKEND", "1"),
    ("NEWENGINE_PLUGIN_TARGET", "runtime"),
    ("NEWENGINE_BOOTSTRAP_PLUGIN_PRELOAD", "deferred"),
    ("NEWENGINE_SHADER_ASYNC_PREBAKED_UNTIL_READY", "1"),
    ("NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_READY", "24"),
    ("NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_RATIO", "1.0"),
    ("NEWENGINE_SCENE_TEXTURE_GATE_SOFT_TIMEOUT_FRAMES", "1800"),
    // App-level direct profile request. This is data, not engine-side game logic:
    // engine.runtime remains the host executor and the app asks for game presentation.
    ("NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__profile", "game"),
    ("NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__game_ui_root_surface_id", "gameFps.ui.root"),
    ("NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__publish_editor_shell", "false"),
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
            default_profile_env: Some((SCENE_PROFILE_ENV, DEFAULT_SCENE_PROFILE)),
            env_defaults: ENV_DEFAULTS,
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
