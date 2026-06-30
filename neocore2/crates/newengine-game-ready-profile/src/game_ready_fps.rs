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
    GameReadyRuntimeProfile, GAME_APP_ASSETS_DIR_ENV, GAME_FIXED_DT_MS, GAME_READY_APP_DIR_NAME,
};

pub const GAME_READY_RUNTIME_ENV_DEFAULTS: &[(&str, &str)] = &[
    ("NEWENGINE_GAME_READY_DEMO", "1"),
    ("NEWENGINE_REQUIRE_RENDER_BACKEND", "1"),
    ("NEWENGINE_REQUIRE_ASSET_MANAGER", "1"),
    ("NEWENGINE_REQUIRE_MATERIALS_BACKEND", "1"),
    ("NEWENGINE_PLUGIN_TARGET", "runtime"),
    // Game-ready launch must not dlopen bootstrap DLLs before platform/runtime

    // diagnostics are visible. Bootstrap plugins are loaded together with the

    // engine phase; stale DLLs can otherwise terminate the process with SEH

    // STATUS_ACCESS_VIOLATION before Rust can report an error.
    ("NEWENGINE_BOOTSTRAP_PLUGIN_PRELOAD", "deferred"),
    // Profile-owned render startup policy: keep the viewport alive while the

    // real authored GLSL is being compiled asynchronously by engine.jobs.

    // Users can override this env value before launch; it is still reported in

    // shader diagnostics as an explicit degraded policy.
    ("NEWENGINE_SHADER_ASYNC_PREBAKED_UNTIL_READY", "1"),
    // Keep the loading projection visible until the first playable frame is

    // visually coherent. Heavy .ytd dictionaries may continue streaming later,

    // but the profile must not reveal a mostly untextured world.
    ("NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_READY", "24"),
    ("NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_RATIO", "1.0"),
    ("NEWENGINE_SCENE_TEXTURE_GATE_SOFT_TIMEOUT_FRAMES", "1800"),
];

/// Standalone game viewport defaults layered on top of the game-ready runtime
/// defaults. Kept in the profile crate so app binaries do not duplicate engine
/// policy keys or hardcode the UI projection route.
pub const GAME_READY_GAME_UI_ENV_DEFAULTS: &[(&str, &str)] = &[
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
    (
        "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__profile",
        "game",
    ),
    (
        "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__game_ui_root_surface_id",
        "gameFps.ui.root",
    ),
    (
        "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__publish_editor_shell",
        "false",
    ),
];

pub const GAME_READY_PROFILE_ENV: &str = "NEWENGINE_SCENE_PROFILE";

pub const GAME_READY_DEFAULT_PROFILE_ASSET: &str = "maps/game_ready_highlands.ymap";

#[derive(Clone)]
pub struct GameReadyFpsApp {
    profile: GameReadyRuntimeProfile,
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
            profile: GameReadyRuntimeProfile::new(),
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

            env_defaults: GAME_READY_RUNTIME_ENV_DEFAULTS,
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

#[inline]
pub fn run_game_ready_fps_process() -> ! {
    GameReadyFpsApp::default().run_process()
}
