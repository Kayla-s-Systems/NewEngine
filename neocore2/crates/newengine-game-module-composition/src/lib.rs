#![forbid(unsafe_op_in_unsafe_fn)]

use std::{collections::BTreeMap, sync::Arc};

use newengine_engine_runtime::{
    gameplay::{
        GameplayContentProvider, GameplayPhysicsQueryProvider, GameplaySystemProvider,
        GameplayUiProvider,
    },
    RuntimeRenderController,
};
use newengine_game_module_api::{GameModuleDescriptorV2, GameModuleGameplayProviderRole};
use newengine_project_api::RuntimeLaunchProfile;
use newengine_project_runtime::{ProjectRuntimeContext, RuntimeCompositionContext};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameModuleTarget {
    Client,
    Server,
    Test,
}

impl From<RuntimeLaunchProfile> for GameModuleTarget {
    fn from(value: RuntimeLaunchProfile) -> Self {
        match value {
            RuntimeLaunchProfile::Game => Self::Client,
            RuntimeLaunchProfile::Server => Self::Server,
            RuntimeLaunchProfile::Test => Self::Test,
        }
    }
}

#[derive(Clone, Default)]
pub struct GameModuleProviderSet {
    /// Ordered authored-scene enrichment contributed by the selected game module.
    /// Runtime profiles own generic bootstrap lifecycle only and never name concrete game assembly.
    pub scene_bootstrap:
        Vec<Arc<dyn newengine_authored_world_runtime::AuthoredMapSceneBootstrapContributor>>,
    pub gameplay_content: Vec<Arc<dyn GameplayContentProvider>>,
    pub gameplay_systems: Vec<Arc<dyn GameplaySystemProvider>>,
    pub gameplay_ui: Vec<Arc<dyn GameplayUiProvider>>,
    pub gameplay_physics_queries: Vec<Arc<dyn GameplayPhysicsQueryProvider>>,
}

impl GameModuleProviderSet {
    pub fn validate_against_descriptor(
        &self,
        descriptor: &GameModuleDescriptorV2,
    ) -> Result<(), String> {
        self.validate_against_descriptor_with_runtime_capabilities(
            descriptor,
            &std::collections::BTreeSet::new(),
        )
    }

    pub fn validate_against_descriptor_with_runtime_capabilities(
        &self,
        descriptor: &GameModuleDescriptorV2,
        runtime_capabilities: &std::collections::BTreeSet<String>,
    ) -> Result<(), String> {
        // V2 runtime subsystem requirements are capability-first and independent of provider ids.
        for requirement in descriptor
            .requirements
            .iter()
            .filter(|requirement| requirement.required)
        {
            if !runtime_capabilities.contains(requirement.capability.trim()) {
                return Err(format!(
                    "game-module '{}' requires runtime capability '{}', but the runtime-unit composition report does not provide it",
                    descriptor.module_id, requirement.capability
                ));
            }
        }

        // Provider references now cover only in-process gameplay traits.
        for required in descriptor
            .providers
            .iter()
            .filter(|provider| provider.required)
        {
            let role = required.role;
            let found = match role {
                GameModuleGameplayProviderRole::GameplayContent => self
                    .gameplay_content
                    .iter()
                    .any(|provider| provider.id() == required.provider_id),
                GameModuleGameplayProviderRole::GameplaySystem => self
                    .gameplay_systems
                    .iter()
                    .any(|provider| provider.id() == required.provider_id),
                GameModuleGameplayProviderRole::GameplayUi => self
                    .gameplay_ui
                    .iter()
                    .any(|provider| provider.id() == required.provider_id),
                GameModuleGameplayProviderRole::GameplayPhysicsQueries => self
                    .gameplay_physics_queries
                    .iter()
                    .any(|provider| provider.id() == required.provider_id),
            };
            if !found {
                return Err(format!(
                    "game-module '{}' required provider '{}' role={:?} was declared but not resolved for this runtime target",
                    descriptor.module_id, required.provider_id, role
                ));
            }
        }
        Ok(())
    }

    pub fn apply_to_render_controller(
        &self,
        mut controller: RuntimeRenderController,
    ) -> RuntimeRenderController {
        for provider in &self.gameplay_content {
            controller = controller.with_gameplay_content_provider(Arc::clone(provider));
        }
        for provider in &self.gameplay_systems {
            controller = controller.with_gameplay_system_provider(Arc::clone(provider));
        }
        for provider in &self.gameplay_ui {
            controller = controller.with_gameplay_ui_provider(Arc::clone(provider));
        }
        for provider in &self.gameplay_physics_queries {
            controller = controller.with_gameplay_physics_query_provider(Arc::clone(provider));
        }
        controller
    }
}

pub trait GameModuleComposition: Send + Sync {
    fn descriptor(&self) -> GameModuleDescriptorV2;
    fn providers(&self, target: GameModuleTarget) -> Result<GameModuleProviderSet, String>;

    fn supports(&self, target: GameModuleTarget) -> bool {
        self.providers(target).is_ok()
    }
}

pub type GameModuleFactory = fn(
    &RuntimeCompositionContext,
    GameModuleTarget,
) -> Result<Arc<dyn GameModuleComposition>, String>;
pub type GameModuleDescriptorFactory = fn() -> GameModuleDescriptorV2;
pub type GameModuleActivation = fn() -> Result<(), String>;

#[derive(Clone, Copy)]
pub struct GameModuleFactoryRegistration {
    pub module_id: &'static str,
    pub factory: GameModuleFactory,
    pub descriptor: Option<GameModuleDescriptorFactory>,
    pub activation: Option<GameModuleActivation>,
}

impl GameModuleFactoryRegistration {
    pub const fn new(module_id: &'static str, factory: GameModuleFactory) -> Self {
        Self {
            module_id,
            factory,
            descriptor: None,
            activation: None,
        }
    }

    #[inline]
    pub const fn with_descriptor(mut self, descriptor: GameModuleDescriptorFactory) -> Self {
        self.descriptor = Some(descriptor);
        self
    }

    #[inline]
    pub const fn with_activation(mut self, activation: GameModuleActivation) -> Self {
        self.activation = Some(activation);
        self
    }
}

#[derive(Clone, Default)]
pub struct GameModuleFactoryRegistry {
    entries: BTreeMap<&'static str, GameModuleFactoryRegistration>,
}

impl GameModuleFactoryRegistry {
    pub fn register(&mut self, registration: GameModuleFactoryRegistration) -> Result<(), String> {
        if registration.module_id.trim().is_empty() {
            return Err("game-module factory id must not be empty".to_owned());
        }
        match self.entries.get(registration.module_id) {
            Some(existing)
                if existing.factory as usize == registration.factory as usize
                    && existing.descriptor.map(|f| f as usize)
                        == registration.descriptor.map(|f| f as usize)
                    && existing.activation.map(|f| f as usize)
                        == registration.activation.map(|f| f as usize) =>
            {
                Ok(())
            }
            Some(_) => Err(format!(
                "game-module factory '{}' already registered by another producer",
                registration.module_id
            )),
            None => {
                self.entries.insert(registration.module_id, registration);
                Ok(())
            }
        }
    }

    #[inline]
    pub fn contains(&self, module_id: &str) -> bool {
        self.entries.contains_key(module_id.trim())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn descriptor_runtime(
        &self,
        runtime: &RuntimeCompositionContext,
    ) -> Result<Option<GameModuleDescriptorV2>, String> {
        let Some(module_id) = runtime
            .game_module
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };
        let registration = self.entries.get(module_id).copied().ok_or_else(|| {
            let available = self.entries.keys().copied().collect::<Vec<_>>().join(", ");
            format!(
                "runtime requires game_module '{module_id}', but this Engine instance has no matching composition factory; available=[{available}]"
            )
        })?;
        let descriptor_factory = registration.descriptor.ok_or_else(|| {
            format!(
                "game-module '{}' has no construction-free descriptor metadata; register GameModuleFactoryRegistration::with_descriptor()",
                module_id
            )
        })?;
        let descriptor = descriptor_factory();
        descriptor
            .validate()
            .map_err(|errors| format!("game-module descriptor invalid: {}", errors.join("; ")))?;
        if descriptor.module_id != module_id {
            return Err(format!(
                "game-module descriptor identity mismatch runtime='{}' metadata='{}'",
                module_id, descriptor.module_id
            ));
        }
        Ok(Some(descriptor))
    }

    pub fn activate_runtime(&self, runtime: &RuntimeCompositionContext) -> Result<(), String> {
        let Some(module_id) = runtime
            .game_module
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(());
        };
        let registration = self.entries.get(module_id).copied().ok_or_else(|| {
            let available = self.entries.keys().copied().collect::<Vec<_>>().join(", ");
            format!(
                "runtime requires game_module '{module_id}', but this Engine instance has no matching composition factory; available=[{available}]"
            )
        })?;
        let activation = registration.activation.ok_or_else(|| {
            format!(
                "game-module '{}' has no runtime activation callback; register GameModuleFactoryRegistration::with_activation()",
                module_id
            )
        })?;
        activation()
    }

    pub fn resolve_runtime(
        &self,
        runtime: &RuntimeCompositionContext,
        target: GameModuleTarget,
    ) -> Result<Option<Arc<dyn GameModuleComposition>>, String> {
        let Some(module_id) = runtime
            .game_module
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };
        let registration = self.entries.get(module_id).copied().ok_or_else(|| {
            let available = self.entries.keys().copied().collect::<Vec<_>>().join(", ");
            format!(
                "runtime requires game_module '{module_id}', but this Engine instance has no matching composition factory; available=[{available}]"
            )
        })?;
        let module = (registration.factory)(runtime, target)?;
        let descriptor = module.descriptor();
        descriptor
            .validate()
            .map_err(|errors| format!("game-module descriptor invalid: {}", errors.join("; ")))?;
        if descriptor.module_id != module_id {
            return Err(format!(
                "game-module composition identity mismatch runtime='{}' factory='{}'",
                module_id, descriptor.module_id
            ));
        }
        Ok(Some(module))
    }

    #[inline]
    pub fn resolve_project(
        &self,
        project: &ProjectRuntimeContext,
        target: GameModuleTarget,
    ) -> Result<Option<Arc<dyn GameModuleComposition>>, String> {
        self.resolve_runtime(&RuntimeCompositionContext::from_project(project), target)
    }
}

pub fn resolve_runtime_game_module(
    registry: &GameModuleFactoryRegistry,
    runtime: &RuntimeCompositionContext,
    target: GameModuleTarget,
) -> Result<Option<Arc<dyn GameModuleComposition>>, String> {
    registry.resolve_runtime(runtime, target)
}

pub fn resolve_project_game_module(
    registry: &GameModuleFactoryRegistry,
    project: &ProjectRuntimeContext,
    target: GameModuleTarget,
) -> Result<Option<Arc<dyn GameModuleComposition>>, String> {
    registry.resolve_project(project, target)
}

#[derive(Clone, Copy)]
pub struct GameModuleBootstrapRegistration {
    pub module_id: &'static str,
    pub activate: fn() -> Result<(), String>,
}

impl GameModuleBootstrapRegistration {
    pub const fn new(module_id: &'static str, activate: fn() -> Result<(), String>) -> Self {
        Self {
            module_id,
            activate,
        }
    }
}

#[derive(Default)]
pub struct GameModuleBootstrapRegistry {
    entries: BTreeMap<&'static str, GameModuleBootstrapRegistration>,
}

impl GameModuleBootstrapRegistry {
    pub fn register(
        &mut self,
        registration: GameModuleBootstrapRegistration,
    ) -> Result<(), String> {
        if registration.module_id.trim().is_empty() {
            return Err("game-module bootstrap id must not be empty".to_owned());
        }
        if self
            .entries
            .insert(registration.module_id, registration)
            .is_some()
        {
            return Err(format!(
                "game-module bootstrap '{}' is already registered",
                registration.module_id
            ));
        }
        Ok(())
    }

    pub fn activate(&self, module_id: &str) -> Result<(), String> {
        let module_id = module_id.trim();
        let registration = self.entries.get(module_id).ok_or_else(|| {
            let available = self.entries.keys().copied().collect::<Vec<_>>().join(", ");
            format!(
                "game_module '{module_id}' is not available in this NewEngine distribution; available=[{available}]"
            )
        })?;
        (registration.activate)()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_profiles_map_to_distinct_game_module_targets() {
        assert_eq!(
            GameModuleTarget::from(RuntimeLaunchProfile::Test),
            GameModuleTarget::Test
        );
        assert_eq!(
            GameModuleTarget::from(RuntimeLaunchProfile::Game),
            GameModuleTarget::Client
        );
        assert_eq!(
            GameModuleTarget::from(RuntimeLaunchProfile::Server),
            GameModuleTarget::Server
        );
    }

    fn rejected_test_factory(
        _runtime: &RuntimeCompositionContext,
        _target: GameModuleTarget,
    ) -> Result<Arc<dyn GameModuleComposition>, String> {
        Err("test factory is not intended to resolve".to_owned())
    }

    #[test]
    fn factory_registries_are_instance_scoped() {
        let mut engine_a = GameModuleFactoryRegistry::default();
        let engine_b = GameModuleFactoryRegistry::default();

        engine_a
            .register(GameModuleFactoryRegistration::new(
                "test.module.a",
                rejected_test_factory,
            ))
            .unwrap();

        assert!(engine_a.contains("test.module.a"));
        assert!(!engine_b.contains("test.module.a"));
        assert_eq!(engine_a.len(), 1);
        assert_eq!(engine_b.len(), 0);
    }

    static TEST_ACTIVATIONS: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);

    fn test_activation() -> Result<(), String> {
        TEST_ACTIVATIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    #[test]
    fn selected_factory_registration_owns_runtime_activation() {
        TEST_ACTIVATIONS.store(0, std::sync::atomic::Ordering::SeqCst);
        let mut registry = GameModuleFactoryRegistry::default();
        registry
            .register(
                GameModuleFactoryRegistration::new("test.module.active", rejected_test_factory)
                    .with_activation(test_activation),
            )
            .unwrap();
        let runtime = RuntimeCompositionContext {
            manifest_path: std::path::PathBuf::from("game.toml"),
            runtime_root: std::path::PathBuf::from("."),
            runtime_profile: "test.profile".to_owned(),
            game_module: Some("test.module.active".to_owned()),
            launch_profile: RuntimeLaunchProfile::Game,
            startup_scene: None,
            startup_presentation_state: None,
            definitions: Vec::new(),
            mounts: newengine_project_api::ContentMountRegistry::default(),
            scripts: newengine_project_api::ProjectScriptRegistry::default(),
        };
        registry.activate_runtime(&runtime).unwrap();
        assert_eq!(
            TEST_ACTIVATIONS.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[test]
    fn generic_game_module_requirement_is_validated_from_runtime_capability_report() {
        let descriptor = GameModuleDescriptorV2 {
            module_id: "test.generic-game-module".to_owned(),
            requirements: vec![
                newengine_service_api::RuntimeUnitRequirementDescriptor::required(
                    newengine_game_module_api::GAME_SCENE_BOOTSTRAP_CAPABILITY,
                ),
            ],
            ..GameModuleDescriptorV2::default()
        };
        let providers = GameModuleProviderSet::default();
        let missing = std::collections::BTreeSet::new();
        assert!(providers
            .validate_against_descriptor_with_runtime_capabilities(&descriptor, &missing)
            .is_err());

        let resolved = [newengine_game_module_api::GAME_SCENE_BOOTSTRAP_CAPABILITY.to_owned()]
            .into_iter()
            .collect();
        providers
            .validate_against_descriptor_with_runtime_capabilities(&descriptor, &resolved)
            .expect("generic requirement must validate from runtime-unit capability report");
    }
}
