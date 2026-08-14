use std::sync::Arc;

use newengine_core::{Engine, EngineError, EngineResult, StartupConfig};
use newengine_game_data::GameDataProvider;
use newengine_game_data_lua::{LuaGameDataProvider, LUA_GAME_DATA_PROVIDER_ID};
use newengine_game_module_composition::{resolve_project_game_module, GameModuleTarget};
use newengine_project_api::ProjectScriptRegistry;
use newengine_project_runtime::ProjectRuntimeContext;
use newengine_render_feature_gameready::GameReadyRenderFeaturePack;
use newengine_render_ui_bridge::EngineUiDrawListBridgeProvider;
use newengine_runtime_host::physics_runtime::PhysicsBackendRuntimeModule;
use newengine_runtime_host::render_runtime::RenderBackendRuntimeModule;
use newengine_ui::{UiBuildFn, UiProviderKind};

use crate::scene_bootstrap::{GameReadySceneBootstrapModule, GameReadyWorldSceneBootstrapProvider};
use crate::validation::GameReadyValidationModule;
use crate::world_runtime::GameReadyWorldRuntimeProvider;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameReadyRuntimeKind {
    StandaloneGame,
}

impl Default for GameReadyRuntimeKind {
    #[inline]
    fn default() -> Self {
        Self::StandaloneGame
    }
}

#[derive(Clone)]
pub struct GameReadyRuntimeProfile {
    pub(crate) viewport: Arc<newengine_engine_runtime::ViewportBridge>,
    pub(crate) plugins: Arc<newengine_engine_runtime::PluginManagerBridge>,
    pub(crate) scene: Arc<newengine_scene_runtime::SceneBridge>,
    game_data_provider: Option<Arc<dyn GameDataProvider>>,
    kind: GameReadyRuntimeKind,
}

impl Default for GameReadyRuntimeProfile {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl GameReadyRuntimeProfile {
    #[inline]
    pub fn new() -> Self {
        let scene = Arc::new(newengine_scene_runtime::SceneBridge::new(
            newengine_scene::Scene::new(),
        ));
        Self {
            viewport: Arc::new(newengine_engine_runtime::ViewportBridge::new()),
            plugins: Arc::new(newengine_engine_runtime::PluginManagerBridge::new()),
            scene,
            game_data_provider: None,
            kind: GameReadyRuntimeKind::StandaloneGame,
        }
    }

    #[inline]
    pub fn standalone_game() -> Self {
        Self::new().with_kind(GameReadyRuntimeKind::StandaloneGame)
    }

    #[inline]
    pub fn with_kind(mut self, kind: GameReadyRuntimeKind) -> Self {
        self.kind = kind;
        self
    }

    /// Replaces the active authored game-data source without changing gameplay/world systems.
    /// The default profile uses `LuaGameDataProvider`; tests/tools may inject Rust or custom providers.
    pub fn with_game_data_provider(mut self, provider: Arc<dyn GameDataProvider>) -> Self {
        self.scene
            .set_scene_bootstrap_provider(GameReadyWorldSceneBootstrapProvider::shared(
                Arc::clone(&provider),
            ));
        self.game_data_provider = Some(provider);
        self
    }

    #[inline]
    pub fn game_data_provider_id(&self) -> Option<&'static str> {
        self.game_data_provider
            .as_ref()
            .map(|provider| provider.id())
    }

    #[inline]
    pub const fn kind(&self) -> GameReadyRuntimeKind {
        self.kind
    }

    #[inline]
    pub(crate) fn register_input_bindings_gateway_best_effort(&self) {
        let input_profile = newengine_input_profile_gameready::game_ready_game_input_profile();
        newengine_input_bindings_runtime::register_input_bindings_gateway_best_effort(
            input_profile,
        );
    }

    #[inline]
    pub fn register_modules(
        &self,
        engine: &mut Engine<()>,
        startup: &StartupConfig,
    ) -> EngineResult<()> {
        self.register_input_bindings_gateway_best_effort();

        let project_scripts = engine
            .resources_mut()
            .get::<ProjectScriptRegistry>()
            .cloned();
        let project_context = engine
            .resources_mut()
            .get::<ProjectRuntimeContext>()
            .cloned();
        let game_module = if let Some(project) = project_context.as_ref() {
            resolve_project_game_module(project, GameModuleTarget::from(project.launch.profile))
                .map_err(EngineError::Other)?
        } else {
            None
        };
        let project_game_data_binding = project_scripts
            .as_ref()
            .and_then(|registry| registry.binding(LUA_GAME_DATA_PROVIDER_ID));
        let game_data_provider: Arc<dyn GameDataProvider> = if let Some(binding) =
            project_game_data_binding
        {
            let operation = binding.operation.ok_or_else(|| {
                EngineError::Other(format!(
                    "project scripting binding '{}' must declare an operation",
                    LUA_GAME_DATA_PROVIDER_ID
                ))
            })?;
            Arc::new(LuaGameDataProvider::new(binding.script_ref).with_operation(operation))
        } else {
            self.game_data_provider.clone().ok_or_else(|| {
                EngineError::Other(format!(
                    "world/render profile requires a project scripting binding for '{}' or an explicitly injected GameDataProvider",
                    LUA_GAME_DATA_PROVIDER_ID
                ))
            })?
        };
        self.scene
            .set_scene_bootstrap_provider(GameReadyWorldSceneBootstrapProvider::shared(
                Arc::clone(&game_data_provider),
            ));

        engine.register_module(Box::new(PhysicsBackendRuntimeModule::new(
            startup.modules_dir.clone(),
        )))?;
        engine.register_module(Box::new(RenderBackendRuntimeModule::new(
            startup.modules_dir.clone(),
        )))?;

        let render_features = GameReadyRenderFeaturePack::new();
        let mut render_controller = newengine_engine_runtime::RuntimeRenderController::new(
            Arc::clone(&self.viewport),
            Arc::clone(&self.plugins),
            Arc::clone(&self.scene),
        )
        .with_world_runtime_provider(GameReadyWorldRuntimeProvider::shared())
        .with_material_pipeline_provider(render_features.material_pipeline_provider())
        .with_primary_lit_material_domain(render_features.primary_lit_material_domain());

        if let Some(game_module) = game_module.as_ref() {
            let descriptor = game_module.descriptor();
            let target = project_context
                .as_ref()
                .map(|project| GameModuleTarget::from(project.launch.profile))
                .unwrap_or(GameModuleTarget::Client);
            let providers = game_module.providers(target).map_err(EngineError::Other)?;
            providers
                .validate_against_descriptor(&descriptor)
                .map_err(EngineError::Other)?;
            newengine_ulog_api::ulog::info!(
                "game module composition: id='{}' version='{}' target={:?} content={} systems={} ui={} physics_queries={}",
                descriptor.module_id,
                descriptor.version,
                target,
                providers.gameplay_content.len(),
                providers.gameplay_systems.len(),
                providers.gameplay_ui.len(),
                providers.gameplay_physics_queries.len(),
            );
            render_controller = providers.apply_to_render_controller(render_controller);
        }

        render_controller =
            render_controller.with_draw_list_provider(EngineUiDrawListBridgeProvider::shared());
        for provider in render_features.draw_list_providers() {
            render_controller = render_controller.with_draw_list_provider(provider);
        }
        for provider in render_features.light_extraction_providers() {
            render_controller = render_controller.with_light_extraction_provider(provider);
        }

        engine.register_module(Box::new(render_controller))?;
        let startup_scene_configured =
            std::env::var(newengine_project_api::PROJECT_STARTUP_SCENE_ENV)
                .ok()
                .is_some_and(|value| !value.trim().is_empty())
                || std::env::var(newengine_game_data::GAME_READY_PROFILE_ENV)
                    .ok()
                    .is_some_and(|value| !value.trim().is_empty());
        if startup_scene_configured {
            engine.register_module(Box::new(GameReadySceneBootstrapModule::new(Arc::clone(
                &self.scene,
            ))))?;
        } else if matches!(self.kind, GameReadyRuntimeKind::StandaloneGame) {
            return Err(EngineError::Other(
                "standalone game launch requires game.toml startup_scene; no implicit scene fallback exists"
                    .to_owned(),
            ));
        } else {
            newengine_ulog_api::ulog::info!(
                "project editor: no startup_scene declared; opening empty staging world"
            );
        }
        if let Some(validation) = GameReadyValidationModule::from_env() {
            engine.register_module(Box::new(validation))?;
        }
        Ok(())
    }

    #[inline]
    pub fn ui_build_from_startup(&self, _startup: &StartupConfig) -> Option<Box<dyn UiBuildFn>> {
        None
    }

    #[inline]
    pub fn ui_provider_kind_from_startup(&self, _startup: &StartupConfig) -> UiProviderKind {
        UiProviderKind::Null
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_game_data::GameData;

    struct TestGameDataProvider;

    impl GameDataProvider for TestGameDataProvider {
        fn id(&self) -> &'static str {
            "test.lua-ready.game-data"
        }

        fn load(&self) -> Result<GameData, String> {
            let mut data = GameData::default();
            data.player.move_speed = 13.0;
            Ok(data)
        }
    }

    #[test]
    fn default_profile_has_no_hardcoded_game_data_provider() {
        let profile = GameReadyRuntimeProfile::new();
        assert_eq!(profile.game_data_provider_id(), None);
    }

    #[test]
    fn runtime_profile_has_no_built_in_gameplay_module_switch() {
        assert_eq!(
            GameReadyRuntimeProfile::new().kind(),
            GameReadyRuntimeKind::StandaloneGame
        );
        assert_eq!(
            GameReadyRuntimeProfile::standalone_game().kind(),
            GameReadyRuntimeKind::StandaloneGame
        );
    }

    #[test]
    fn profile_accepts_replaceable_game_data_provider() {
        let profile =
            GameReadyRuntimeProfile::new().with_game_data_provider(Arc::new(TestGameDataProvider));
        assert_eq!(
            profile.game_data_provider_id(),
            Some("test.lua-ready.game-data")
        );
    }
}
