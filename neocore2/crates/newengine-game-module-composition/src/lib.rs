#![forbid(unsafe_op_in_unsafe_fn)]

use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock, RwLock},
};

use newengine_engine_runtime::{
    gameplay::{
        GameplayContentProvider, GameplayPhysicsQueryProvider, GameplaySystemProvider,
        GameplayUiProvider,
    },
    RuntimeRenderController,
};
use newengine_game_module_api::GameModuleDescriptorV1;
use newengine_project_api::RuntimeLaunchProfile;
use newengine_project_runtime::ProjectRuntimeContext;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameModuleTarget {
    Editor,
    Client,
    Server,
    Test,
}

impl From<RuntimeLaunchProfile> for GameModuleTarget {
    fn from(value: RuntimeLaunchProfile) -> Self {
        match value {
            RuntimeLaunchProfile::Editor => Self::Editor,
            RuntimeLaunchProfile::Game => Self::Client,
            RuntimeLaunchProfile::Server => Self::Server,
            RuntimeLaunchProfile::Test => Self::Test,
        }
    }
}

#[derive(Clone, Default)]
pub struct GameModuleProviderSet {
    pub gameplay_content: Vec<Arc<dyn GameplayContentProvider>>,
    pub gameplay_systems: Vec<Arc<dyn GameplaySystemProvider>>,
    pub gameplay_ui: Vec<Arc<dyn GameplayUiProvider>>,
    pub gameplay_physics_queries: Vec<Arc<dyn GameplayPhysicsQueryProvider>>,
}

impl GameModuleProviderSet {
    pub fn validate_against_descriptor(
        &self,
        descriptor: &GameModuleDescriptorV1,
    ) -> Result<(), String> {
        use newengine_game_module_api::GameModuleProviderRole;
        for required in descriptor
            .providers
            .iter()
            .filter(|provider| provider.required)
        {
            let Some(role) = required.role.as_ref() else {
                return Err(format!(
                    "game-module provider '{}' has no role",
                    required.provider_id
                ));
            };
            let found = match role {
                GameModuleProviderRole::GameplayContent => self
                    .gameplay_content
                    .iter()
                    .any(|provider| provider.id() == required.provider_id),
                GameModuleProviderRole::GameplaySystem => self
                    .gameplay_systems
                    .iter()
                    .any(|provider| provider.id() == required.provider_id),
                GameModuleProviderRole::GameplayUi => self
                    .gameplay_ui
                    .iter()
                    .any(|provider| provider.id() == required.provider_id),
                GameModuleProviderRole::GameplayPhysicsQueries => self
                    .gameplay_physics_queries
                    .iter()
                    .any(|provider| provider.id() == required.provider_id),
                GameModuleProviderRole::SceneBootstrap
                | GameModuleProviderRole::WorldRuntime
                | GameModuleProviderRole::InputProfile
                | GameModuleProviderRole::RenderFeature => false,
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
    fn descriptor(&self) -> GameModuleDescriptorV1;
    fn providers(&self, target: GameModuleTarget) -> Result<GameModuleProviderSet, String>;

    fn supports(&self, target: GameModuleTarget) -> bool {
        self.providers(target).is_ok()
    }
}

pub type GameModuleFactory =
    fn(&ProjectRuntimeContext, GameModuleTarget) -> Result<Arc<dyn GameModuleComposition>, String>;

#[derive(Clone, Copy)]
pub struct GameModuleFactoryRegistration {
    pub module_id: &'static str,
    pub factory: GameModuleFactory,
}

impl GameModuleFactoryRegistration {
    pub const fn new(module_id: &'static str, factory: GameModuleFactory) -> Self {
        Self { module_id, factory }
    }
}

static FACTORIES: OnceLock<RwLock<BTreeMap<&'static str, GameModuleFactoryRegistration>>> =
    OnceLock::new();

fn factories() -> &'static RwLock<BTreeMap<&'static str, GameModuleFactoryRegistration>> {
    FACTORIES.get_or_init(|| RwLock::new(BTreeMap::new()))
}

pub fn register_game_module_factory(
    registration: GameModuleFactoryRegistration,
) -> Result<(), String> {
    if registration.module_id.trim().is_empty() {
        return Err("game-module factory id must not be empty".to_owned());
    }
    let mut registry = factories()
        .write()
        .map_err(|_| "game-module factory registry poisoned".to_owned())?;
    match registry.get(registration.module_id) {
        Some(existing) if existing.factory as usize == registration.factory as usize => Ok(()),
        Some(_) => Err(format!(
            "game-module factory '{}' already registered by another producer",
            registration.module_id
        )),
        None => {
            registry.insert(registration.module_id, registration);
            Ok(())
        }
    }
}

pub fn resolve_project_game_module(
    project: &ProjectRuntimeContext,
    target: GameModuleTarget,
) -> Result<Option<Arc<dyn GameModuleComposition>>, String> {
    let Some(module_id) = project
        .manifest
        .game_module
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let registration = factories()
        .read()
        .map_err(|_| "game-module factory registry poisoned".to_owned())?
        .get(module_id)
        .copied()
        .ok_or_else(|| {
            format!(
            "project requires game_module '{module_id}', but no composition factory is registered"
        )
        })?;
    let module = (registration.factory)(project, target)?;
    let descriptor = module.descriptor();
    descriptor
        .validate()
        .map_err(|errors| format!("game-module descriptor invalid: {}", errors.join("; ")))?;
    if descriptor.module_id != module_id {
        return Err(format!(
            "game-module composition identity mismatch project='{}' factory='{}'",
            module_id, descriptor.module_id
        ));
    }
    Ok(Some(module))
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
            GameModuleTarget::from(RuntimeLaunchProfile::Editor),
            GameModuleTarget::Editor
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
}
