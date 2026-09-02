#![forbid(unsafe_op_in_unsafe_fn)]

//! Generic Game Ready world/runtime profile composition.
//! The crate root is intentionally a thin facade; composition and service routing live separately.

mod entity_archetypes;
mod env_config;
mod game_ready_fps;
mod profile;
mod provider_routes;
mod runtime_units;
mod scene_bootstrap;
mod validation;
mod world_runtime;

use newengine_asset_bootstrap_runtime::{ContentSetSpec, ProfileMountSpec};

pub use game_ready_fps::{
    apply_game_ready_fps_env_policy, run_game_ready_fps_process, GameReadyFpsApp,
    GAME_READY_CORE_ENV_POLICY, GAME_READY_FPS_BOOT_OPTIONS, GAME_READY_FPS_ENV_POLICY,
    GAME_READY_UI_PROFILE_GAME, GAME_READY_UI_PUBLISH_EDITOR_SHELL_ENV,
    GAME_READY_UI_SCREEN_PROFILE_ENV,
};
pub use newengine_game_data::{
    GameData, GameDataProvider, GameDataSnapshot, RustGameDataProvider, GAME_APP_ASSETS_DIR_ENV,
    GAME_FIXED_DT_MS, GAME_READY_APP_DIR_NAME, GAME_READY_DEFAULT_PROFILE_ASSET,
    GAME_READY_FPS_APP_NAME, GAME_READY_FPS_EARLY_LOG_FILE, GAME_READY_FPS_WINDOW_TITLE,
    GAME_READY_PROFILE_ENV,
};
pub use profile::GameReadyRuntimeProfile;

pub const GAME_READY_RUNTIME_PROFILE_ID: &str = "newengine.runtime-profile.game-ready";

/// Launches the generic GameReady runtime for a resolved game manifest.
///
/// Development tooling may pass a project-owned `game.toml`; packaged game/server builds place the
/// same game descriptor next to the runtime executable. Engine `runtime.toml` is not
/// a game descriptor and is never parsed here.
pub fn launch_game_ready_profile_with(
    manifest_path: &std::path::Path,
    profile: GameReadyRuntimeProfile,
) -> Result<(), String> {
    let launch_id = std::env::var(newengine_project_api::PROJECT_LAUNCH_PRESET_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty());
    newengine_project_runtime::load_project_from_request_with_launch(
        manifest_path,
        launch_id.as_deref(),
    )?;
    std::env::set_var(newengine_project_api::GAME_MANIFEST_ENV, manifest_path);
    std::env::remove_var("NEWENGINE_PROJECT");
    apply_game_ready_fps_env_policy();
    GameReadyFpsApp::with_profile(profile).run_process()
}

pub fn launch_registered_game_ready_profile(manifest_path: &std::path::Path) -> Result<(), String> {
    // Distribution-owned implementations live with the runtime profile, never inside a
    // game-module descriptor plugin. The selected module is still resolved from game.toml;
    // registering a factory does not activate it unless the runtime requests that module id.
    let profile = GameReadyRuntimeProfile::standalone_game()
        .with_game_module_factory(newengine_game_module_fps::factory_registration());
    launch_game_ready_profile_with(manifest_path, profile)
}

/// Registration consumed by the generic NewEngine runtime-profile registry.
/// NewEngine no longer branches on this profile id; compiled profiles register
/// themselves through the same versioned project-runtime contract.
pub fn runtime_profile_registration() -> newengine_project_runtime::RuntimeProfileRegistration {
    newengine_project_runtime::RuntimeProfileRegistration::new(
        GAME_READY_RUNTIME_PROFILE_ID,
        "Game Ready World",
        launch_registered_game_ready_profile,
    )
}

pub const GAME_READY_CONTENT_SETS: &[ContentSetSpec] = &[ContentSetSpec::runtime_app(
    "game-ready.primary",
    GAME_READY_APP_DIR_NAME,
    &[GAME_APP_ASSETS_DIR_ENV],
)];
pub const GAME_READY_MOUNT_SPEC: ProfileMountSpec =
    ProfileMountSpec::new("game-ready", GAME_READY_CONTENT_SETS);
