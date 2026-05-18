#![forbid(unsafe_op_in_unsafe_fn)]

//! Declarative launcher binding for the Game Ready FPS vertical slice.
//!
//! The binary entrypoint delegates here instead of manually assembling runtime
//! host services, engine modules, plugin preload, asset roots and platform
//! runtime execution.

use newengine_core::{Engine, EngineResult, StartupConfig};
use newengine_runtime_host::app_launcher::{
    RuntimeHostAppProfile, RuntimeHostLaunchSpec, RuntimeHostLauncher,
};
use newengine_ui::{UiBuildFn, UiProviderKind};

use crate::{
    StandaloneGameRuntimeProfile, GAME_APP_ASSETS_DIR_ENV, GAME_FIXED_DT_MS,
    GAME_READY_APP_DIR_NAME,
};

const GAME_READY_ENV_DEFAULTS: &[(&str, &str)] = &[
    ("NEWENGINE_GAME_READY_DEMO", "1"),
    ("NEWENGINE_REQUIRE_RENDER_BACKEND", "1"),
    ("NEWENGINE_REQUIRE_ASSET_MANAGER", "1"),
    ("NEWENGINE_REQUIRE_PLATFORM_BACKEND", "1"),
    ("NEWENGINE_PLUGIN_TARGET", "runtime"),
    // Game-ready launch must not dlopen bootstrap DLLs before platform/runtime
    // diagnostics are visible. Bootstrap plugins are loaded together with the
    // engine phase; stale DLLs can otherwise terminate the process with SEH
    // STATUS_ACCESS_VIOLATION before Rust can report an error.
    ("NEWENGINE_BOOTSTRAP_PLUGIN_PRELOAD", "deferred"),
];

pub const GAME_READY_PROFILE_ENV: &str = "NEWENGINE_GAME_READY_PROFILE";
pub const GAME_READY_DEFAULT_PROFILE_ASSET: &str = "game_ready_highlands.scene.json";

#[derive(Clone)]
pub struct GameReadyFpsApp {
    profile: StandaloneGameRuntimeProfile,
}

impl Default for GameReadyFpsApp {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl GameReadyFpsApp {
    #[inline]
    pub fn new() -> Self {
        Self {
            profile: StandaloneGameRuntimeProfile::new(),
        }
    }

    #[inline]
    pub fn launch_spec() -> RuntimeHostLaunchSpec {
        RuntimeHostLaunchSpec {
            product_name: "NewEngine",
            app_name: "game-ready-fps",
            app_version: env!("CARGO_PKG_VERSION"),
            startup_config_path: "config.json",
            fixed_dt_ms: GAME_FIXED_DT_MS,
            app_dir_name: GAME_READY_APP_DIR_NAME,
            app_assets_env: GAME_APP_ASSETS_DIR_ENV,
            window_title: "KAYLA FPS: Procedural Highlands",
            early_log_file_name: "game-ready-early.log",
            default_profile_env: Some((GAME_READY_PROFILE_ENV, GAME_READY_DEFAULT_PROFILE_ASSET)),
            env_defaults: GAME_READY_ENV_DEFAULTS,
        }
    }

    #[inline]
    pub fn run_process(self) -> ! {
        RuntimeHostLauncher::new(Self::launch_spec(), self).run_process()
    }
}

impl RuntimeHostAppProfile for GameReadyFpsApp {
    #[inline]
    fn register_modules(
        &self,
        engine: &mut Engine<()>,
        startup: &StartupConfig,
    ) -> EngineResult<()> {
        self.profile.register_modules(engine, startup)
    }

    #[inline]
    fn register_engine_owned_gateways_best_effort(&self) {
        self.profile.register_scene_io_best_effort();
        self.profile.register_ecs_gateway_best_effort();
        self.profile.register_entity_gateway_best_effort();
    }

    #[inline]
    fn bootstrap_content_best_effort(&self) {
        self.profile.bootstrap_game_ready_scene_best_effort();
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

#[inline]
pub fn run_game_ready_fps_process() -> ! {
    GameReadyFpsApp::default().run_process()
}
