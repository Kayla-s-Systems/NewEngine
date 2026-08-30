use newengine_project_api::{ProjectManifest, ResolvedProjectLaunch, RuntimeLaunchProfile};

use crate::shared_ui::effective_project_ui_presentation_flow;

pub const UI_SCREEN_PROFILE_ENV: &str =
    "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__profile";
pub const UI_PRESENTATION_INITIAL_STATE_ENV: &str =
    "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__presentation_flow__initial_state";
pub const UI_PRESENTATION_FLOW_ENV: &str =
    "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__presentation_flow";
pub const UI_ROOT_SURFACE_ENV: &str =
    "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__game_ui_root_surface_id";
pub const UI_DOCUMENT_ENV: &str =
    "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__game_ui_document_ref";
pub const UI_PUBLISH_EDITOR_SHELL_ENV: &str =
    "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__publish_editor_shell";

fn set_default_env(key: &str, value: &str) {
    if std::env::var_os(key).is_none() {
        std::env::set_var(key, value);
    }
}

pub fn apply_project_launch_profile_env(profile: RuntimeLaunchProfile) {
    set_default_env("NEWENGINE_LAUNCH_PROFILE", profile.id());
    match profile {
        RuntimeLaunchProfile::Game => {
            set_default_env("NEWENGINE_HEADLESS", "0");
            set_default_env(UI_SCREEN_PROFILE_ENV, "game");
        }
        RuntimeLaunchProfile::Server => {
            set_default_env("NEWENGINE_HEADLESS", "1");
            set_default_env(UI_SCREEN_PROFILE_ENV, "headless");
            set_default_env("NEWENGINE_PLUGIN_TARGET", "runtime");
        }
        RuntimeLaunchProfile::Test => {
            set_default_env("NEWENGINE_HEADLESS", "1");
            set_default_env(UI_SCREEN_PROFILE_ENV, "headless");
            set_default_env("NEWENGINE_HEADLESS_FRAMES", "1");
            set_default_env("NEWENGINE_PLUGIN_TARGET", "runtime");
        }
    }
}

pub fn apply_project_startup_presentation_state_env(state: &str) {
    let state = state.trim();
    if !state.is_empty() {
        set_default_env(UI_PRESENTATION_INITIAL_STATE_ENV, state);
    }
}

pub fn apply_project_ui_env(manifest: &ProjectManifest) {
    if let Some(value) = manifest
        .ui
        .screen_profile
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        std::env::set_var(UI_SCREEN_PROFILE_ENV, value);
    }
    if let Some(value) = manifest
        .ui
        .root_surface
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        std::env::set_var(UI_ROOT_SURFACE_ENV, value);
    }
    if let Some(value) = manifest
        .ui
        .document
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        std::env::set_var(UI_DOCUMENT_ENV, value);
    }
    let requested_initial_state = std::env::var(UI_PRESENTATION_INITIAL_STATE_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if let Some(flow) =
        effective_project_ui_presentation_flow(manifest, requested_initial_state.as_deref())
    {
        // Fold launch-state overrides into the complete authored/shared graph so
        // environment iteration order cannot replace the object with a scalar subtree.
        std::env::remove_var(UI_PRESENTATION_INITIAL_STATE_ENV);
        if let Ok(encoded) = serde_json::to_string(&flow) {
            std::env::set_var(UI_PRESENTATION_FLOW_ENV, encoded);
        }
    } else {
        std::env::remove_var(UI_PRESENTATION_FLOW_ENV);
    }
    if let Some(value) = manifest.ui.publish_editor_shell {
        std::env::set_var(
            UI_PUBLISH_EDITOR_SHELL_ENV,
            if value { "true" } else { "false" },
        );
    }
}

pub fn apply_resolved_project_launch_env(launch: &ResolvedProjectLaunch) {
    apply_project_launch_profile_env(launch.profile);
    if let Some(state) = launch.startup_presentation_state.as_deref() {
        apply_project_startup_presentation_state_env(state);
    }
}
