use std::sync::Arc;

use newengine_core::{Engine, EngineError, EngineResult, StartupConfig};
use newengine_game_data::GameDataProvider;
use newengine_game_module_composition::{
    resolve_runtime_game_module, GameModuleFactoryRegistration, GameModuleFactoryRegistry,
    GameModuleTarget,
};
use newengine_project_runtime::RuntimeCompositionContext;
use newengine_ui::{UiBuildFn, UiProviderKind};

use crate::validation::GameReadyValidationModule;

#[derive(Clone)]
pub struct GameReadyRuntimeProfile {
    pub(crate) viewport: Arc<newengine_engine_runtime::ViewportBridge>,
    pub(crate) plugins: Arc<newengine_engine_runtime::PluginManagerBridge>,
    pub(crate) scene: Arc<newengine_scene_runtime::SceneBridge>,
    pub(crate) game_data_provider: Option<Arc<dyn GameDataProvider>>,
    game_module_factories: GameModuleFactoryRegistry,
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
        // GameReady exposes live F2 world authoring when the EditingTools capability is present.
        // SceneBridge intentionally owns no authoring implementation, so the product profile
        // injects the focused provider explicitly.
        scene.set_scene_authoring_provider(Arc::new(
            newengine_scene_authoring_runtime::SceneAuthoringRuntime::default(),
        ));
        Self {
            viewport: Arc::new(newengine_engine_runtime::ViewportBridge::new()),
            plugins: Arc::new(newengine_engine_runtime::PluginManagerBridge::new()),
            scene,
            game_data_provider: None,
            game_module_factories: GameModuleFactoryRegistry::default(),
        }
    }

    #[inline]
    pub fn standalone_game() -> Self {
        Self::new()
    }

    /// Replaces the active authored game-data source without changing gameplay/world systems.
    /// The default profile uses `LuaGameDataProvider`; tests/tools may inject Rust or custom providers.
    pub fn with_game_data_provider(mut self, provider: Arc<dyn GameDataProvider>) -> Self {
        self.game_data_provider = Some(provider);
        self
    }

    /// Adds a game-owned composition factory to this runtime instance.
    /// Factories are ordinary owned profile state; no process-global registry is consulted.
    pub fn with_game_module_factory(mut self, factory: GameModuleFactoryRegistration) -> Self {
        self.game_module_factories
            .register(factory)
            .expect("duplicate game-module factory in one runtime profile");
        self
    }

    #[inline]
    pub fn game_module_factory_count(&self) -> usize {
        self.game_module_factories.len()
    }

    #[inline]
    pub(crate) fn game_module_factory_registry(&self) -> GameModuleFactoryRegistry {
        self.game_module_factories.clone()
    }

    #[inline]
    pub fn game_data_provider_id(&self) -> Option<&'static str> {
        self.game_data_provider
            .as_ref()
            .map(|provider| provider.id())
    }

    pub fn runtime_unit_requirements_for_runtime(
        &self,
        runtime: Option<&RuntimeCompositionContext>,
    ) -> Result<Vec<newengine_service_api::RuntimeUnitRequirementDescriptor>, String> {
        let Some(runtime) = runtime else {
            return Ok(Vec::new());
        };
        let Some(descriptor) = self.game_module_factories.descriptor_runtime(runtime)? else {
            return Ok(Vec::new());
        };
        Ok(descriptor.runtime_unit_requirements())
    }

    #[inline]
    pub fn register_modules(
        &self,
        engine: &mut Engine<()>,
        _startup: &StartupConfig,
    ) -> EngineResult<()> {
        let runtime_context = engine
            .resources_mut()
            .get::<RuntimeCompositionContext>()
            .cloned();
        let runtime_unit_capabilities = engine
            .resources_mut()
            .get::<newengine_runtime_host::app_launcher::RuntimeUnitCompositionReport>()
            .map(|report| report.provided_capabilities.clone())
            .unwrap_or_default();
        eprintln!(
            "GameReady profile composition probe: instance_factories={} runtime_game_module={}",
            self.game_module_factories.len(),
            runtime_context
                .as_ref()
                .and_then(|runtime| runtime.game_module.as_deref())
                .unwrap_or("<none>")
        );
        let game_module = if let Some(runtime) = runtime_context.as_ref() {
            self.game_module_factories
                .activate_runtime(runtime)
                .map_err(EngineError::Other)?;
            let target = GameModuleTarget::from(runtime.launch_profile);
            resolve_runtime_game_module(&self.game_module_factories, runtime, target)
                .map_err(EngineError::Other)?
        } else {
            None
        };

        let render_contributions = engine
            .resources_mut()
            .remove::<newengine_engine_runtime::RuntimeRenderContributionRegistry>()
            .unwrap_or_default();
        let selected_render_features = render_contributions.render_feature_count();
        let selected_world_runtimes = render_contributions.world_runtime_provider_count();
        let mut render_controller = render_contributions.apply_to(
            newengine_engine_runtime::RuntimeRenderController::new(
                Arc::clone(&self.viewport),
                Arc::clone(&self.plugins),
                Arc::clone(&self.scene),
            )
            .with_gameplay_physics_query_provider(
                newengine_engine_runtime::AudioOcclusionPhysicsQueryProvider::shared(),
            )
            .with_gameplay_physics_query_provider(
                newengine_engine_runtime::AudioDiffractionPhysicsQueryProvider::shared(),
            )
            .with_gameplay_physics_query_provider(
                newengine_engine_runtime::AudioReflectionPhysicsQueryProvider::shared(),
            ),
        );

        if let Some(game_module) = game_module.as_ref() {
            let descriptor = game_module.descriptor();
            let target = runtime_context
                .as_ref()
                .map(|runtime| GameModuleTarget::from(runtime.launch_profile))
                .unwrap_or(GameModuleTarget::Client);
            let providers = game_module.providers(target).map_err(EngineError::Other)?;
            providers
                .validate_against_descriptor_with_runtime_capabilities(
                    &descriptor,
                    &runtime_unit_capabilities,
                )
                .map_err(EngineError::Other)?;
            newengine_ulog_api::ulog::info!(
                "game module composition: id='{}' version='{}' target={:?} runtime_world={} render_features={} content={} systems={} ui={} physics_queries={}",
                descriptor.module_id,
                descriptor.version,
                target,
                selected_world_runtimes,
                selected_render_features,
                providers.gameplay_content.len(),
                providers.gameplay_systems.len(),
                providers.gameplay_ui.len(),
                providers.gameplay_physics_queries.len(),
            );
            render_controller = providers.apply_to_render_controller(render_controller);
        }

        engine.register_module(Box::new(render_controller))?;
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
    fn gameready_profile_installs_scene_authoring_provider_for_f2_world_editor() {
        let profile = GameReadyRuntimeProfile::new();
        assert!(profile.scene.scene_authoring_available());
        assert!(profile.scene.set_in_game_editor_enabled(true));
        assert!(profile.scene.in_game_editor_enabled());
        assert!(!profile.scene.set_in_game_editor_enabled(false));
        assert!(!profile.scene.in_game_editor_enabled());
    }

    #[test]
    fn default_profile_has_no_hardcoded_game_data_provider() {
        let profile = GameReadyRuntimeProfile::new();
        assert_eq!(profile.game_data_provider_id(), None);
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
