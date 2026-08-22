#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_core::{Engine, EngineResult, StartupConfig};
use newengine_game_data::RustGameDataProvider;
use newengine_runtime_host::app_launcher::{
    RuntimeHostAppProfile, RuntimeHostBootOption, RuntimeHostLaunchSpec, RuntimeHostLauncher,
};
use newengine_ui::{UiBuildFn, UiProviderKind};
use newengine_windowed_host_runtime::{WindowedHostFrontend, WindowedRuntimeHostProfile};

use crate::{
    GameReadyRuntimeProfile, GAME_APP_ASSETS_DIR_ENV, GAME_FIXED_DT_MS, GAME_READY_APP_DIR_NAME,
};

pub const PROJECT_EDITOR_RUNTIME_PROFILE_ID: &str = "newengine.runtime-profile.editor";

const PROJECT_EDITOR_BOOT_OPTIONS: &[RuntimeHostBootOption] = &[
    RuntimeHostBootOption::RuntimeBootstrapOverlay,
    RuntimeHostBootOption::RuntimePlugins,
    RuntimeHostBootOption::PlatformWindow,
    RuntimeHostBootOption::RenderBackend,
    RuntimeHostBootOption::UiBackend,
];

const PROJECT_EDITOR_ENV_POLICY: &[(&str, &str)] = &[
    ("NEWENGINE_GAME_READY_DEMO", "1"),
    ("NEWENGINE_REQUIRE_RENDER_BACKEND", "1"),
    ("NEWENGINE_REQUIRE_ASSET_MANAGER", "1"),
    ("NEWENGINE_REQUIRE_MATERIALS_BACKEND", "1"),
    ("NEWENGINE_REQUIRE_UI_BACKEND", "1"),
    ("NEWENGINE_PLUGIN_TARGET", "runtime"),
    ("NEWENGINE_BOOTSTRAP_PLUGIN_PRELOAD", "deferred"),
    ("NEWENGINE_SHADER_ASYNC_PREBAKED_UNTIL_READY", "1"),
    ("NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_READY", "0"),
    ("NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_RATIO", "1.00"),
    ("NEWENGINE_SCENE_TEXTURE_GATE_SOFT_TIMEOUT_FRAMES", "1800"),
    ("NEWENGINE_SCENE_TEXTURE_GATE_SOFT_TIMEOUT_MS", "90000"),
    (
        "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__profile",
        "editor",
    ),
    (
        "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__publish_editor_shell",
        "true",
    ),
];

#[derive(Clone)]
struct ProjectEditorApp {
    profile: GameReadyRuntimeProfile,
}

impl ProjectEditorApp {
    fn new() -> Self {
        Self {
            profile: GameReadyRuntimeProfile::editor_tools()
                .with_game_data_provider(Arc::new(RustGameDataProvider)),
        }
    }

    fn launch_spec() -> RuntimeHostLaunchSpec {
        RuntimeHostLaunchSpec {
            product_name: "North Star",
            app_name: "ProjectEditor",
            app_version: env!("CARGO_PKG_VERSION"),
            startup_config_path: "config.json",
            fixed_dt_ms: GAME_FIXED_DT_MS,
            app_dir_name: GAME_READY_APP_DIR_NAME,
            app_assets_env: GAME_APP_ASSETS_DIR_ENV,
            early_log_file_name: "newengine-project-editor-early.log",
            default_profile_env: None,
            env_defaults: PROJECT_EDITOR_ENV_POLICY,
        }
    }

    fn run_process(self) -> ! {
        RuntimeHostLauncher::new(Self::launch_spec(), self)
            .run_process_with_frontend(WindowedHostFrontend::new("North Star Editor"))
    }
}

impl RuntimeHostAppProfile for ProjectEditorApp {
    fn register_modules(
        &self,
        engine: &mut Engine<()>,
        startup: &StartupConfig,
    ) -> EngineResult<()> {
        self.profile.register_modules(engine, startup)
    }

    fn boot_options(&self) -> Option<&'static [RuntimeHostBootOption]> {
        Some(PROJECT_EDITOR_BOOT_OPTIONS)
    }

    fn initialize_composition_services(
        &self,
        engine: &mut Engine<()>,
        host_preinit: &newengine_runtime_host::HostPreInitSnapshot,
        runtime: Option<&newengine_project_runtime::RuntimeCompositionContext>,
    ) -> EngineResult<()> {
        self.profile
            .initialize_composition_services(engine, host_preinit, runtime)
    }

    fn register_engine_provider_routes_best_effort(&self) {
        self.profile.register_engine_provider_routes_best_effort();
    }

    fn bootstrap_content_best_effort(&self) {
        self.profile.bootstrap_content_best_effort();
    }

}

impl WindowedRuntimeHostProfile for ProjectEditorApp {
    fn ui_build_from_startup(&self, startup: &StartupConfig) -> Option<Box<dyn UiBuildFn>> {
        self.profile.ui_build_from_startup(startup)
    }

    fn ui_provider_kind_from_startup(&self, startup: &StartupConfig) -> UiProviderKind {
        self.profile.ui_provider_kind_from_startup(startup)
    }
}

/// Launches the generic project editor for a resolved project manifest.
/// Public only for stable-ABI runtime-profile wrappers; NewEngine.exe remains implementation-agnostic.
pub fn launch_registered_project_editor_profile(
    manifest_path: &std::path::Path,
) -> Result<(), String> {
    let launch_id = std::env::var(newengine_project_api::PROJECT_LAUNCH_PRESET_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty());
    let project = newengine_project_runtime::load_project_from_request_with_launch(
        manifest_path,
        launch_id.as_deref(),
    )?;
    if project.launch.profile != newengine_project_api::RuntimeLaunchProfile::Editor {
        return Err(format!(
            "generic project editor runtime only accepts editor launch profiles; project '{}' resolved '{}'",
            project.manifest.id,
            project.launch.profile.id()
        ));
    }
    std::env::set_var("NEWENGINE_PROJECT", manifest_path);
    ProjectEditorApp::new().run_process()
}

pub fn runtime_profile_registration() -> newengine_project_runtime::RuntimeProfileRegistration {
    newengine_project_runtime::RuntimeProfileRegistration::new(
        PROJECT_EDITOR_RUNTIME_PROFILE_ID,
        "North Star Project Editor",
        launch_registered_project_editor_profile,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_editor_requires_runtime_ui_and_editor_screen_profile() {
        assert!(PROJECT_EDITOR_BOOT_OPTIONS.contains(&RuntimeHostBootOption::UiBackend));
        let value = |key: &str| {
            PROJECT_EDITOR_ENV_POLICY
                .iter()
                .find_map(|(candidate, value)| (*candidate == key).then_some(*value))
        };
        assert_eq!(
            value("NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__profile"),
            Some("editor")
        );
        assert_eq!(
            value("NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__publish_editor_shell"),
            Some("true")
        );
    }
}
