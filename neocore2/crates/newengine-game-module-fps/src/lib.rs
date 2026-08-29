#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_fps_content_runtime::FpsContentProvider;
use newengine_fps_inventory_ui_runtime::{
    FpsInventoryHudProvider, ScriptFpsCharacterMenuPolicyProvider,
    SCRIPT_FPS_CHARACTER_MENU_PROVIDER_ID,
};
use newengine_game_module_api::GameModuleDescriptorV2;
use newengine_game_module_composition::{
    GameModuleBootstrapRegistration, GameModuleComposition, GameModuleFactoryRegistration,
    GameModuleProviderSet, GameModuleTarget,
};
use newengine_game_module_fps_contract::descriptor_v2;
pub use newengine_game_module_fps_contract::{FPS_GAME_MODULE_ID, FPS_GAME_MODULE_VERSION};
use newengine_gameplay_fps::FpsGameplayProvider;
use newengine_gameplay_fps_api::{FpsCharacterMenuPolicyProvider, FpsGameplayPolicyProvider};
use newengine_gameplay_fps_lua::{
    LuaFpsGameplayPolicyProvider, LUA_FPS_GAMEPLAY_PROVIDER_ID, SCRIPT_FPS_GAMEPLAY_PROVIDER_ID,
};
use newengine_gameplay_script_api::ScriptedGameplayProvider;
use newengine_project_runtime::RuntimeCompositionContext;

struct FpsGameModule {
    policy: Arc<LuaFpsGameplayPolicyProvider>,
    character_menu_policy: Option<Arc<ScriptFpsCharacterMenuPolicyProvider>>,
}

impl GameModuleComposition for FpsGameModule {
    fn descriptor(&self) -> GameModuleDescriptorV2 {
        descriptor_v2()
    }

    fn providers(&self, target: GameModuleTarget) -> Result<GameModuleProviderSet, String> {
        let policy_for_content: Arc<dyn FpsGameplayPolicyProvider> = self.policy.clone();
        let policy_for_system: Arc<dyn FpsGameplayPolicyProvider> = self.policy.clone();
        let policy_for_queries: Arc<dyn FpsGameplayPolicyProvider> = self.policy.clone();
        let scripts_for_system: Arc<dyn ScriptedGameplayProvider> = self.policy.clone();
        let scripts_for_queries: Arc<dyn ScriptedGameplayProvider> = self.policy.clone();

        let mut providers = GameModuleProviderSet::default();
        providers
            .gameplay_content
            .push(FpsContentProvider::shared(policy_for_content));
        if let Some(character_menu) = self.character_menu_policy.as_ref() {
            let character_menu: Arc<dyn FpsCharacterMenuPolicyProvider> = character_menu.clone();
            providers
                .gameplay_systems
                .push(FpsGameplayProvider::shared_with_character_menu(
                    policy_for_system,
                    scripts_for_system,
                    character_menu,
                ));
        } else {
            providers.gameplay_systems.push(FpsGameplayProvider::shared(
                policy_for_system,
                scripts_for_system,
            ));
        }
        providers
            .gameplay_physics_queries
            .push(FpsGameplayProvider::shared(
                policy_for_queries,
                scripts_for_queries,
            ));
        if !matches!(target, GameModuleTarget::Server) {
            providers
                .gameplay_ui
                .push(FpsInventoryHudProvider::shared());
        }

        Ok(providers)
    }
}

fn create_fps_module(
    runtime: &RuntimeCompositionContext,
    _target: GameModuleTarget,
) -> Result<Arc<dyn GameModuleComposition>, String> {
    let (binding_id, binding) = runtime
        .scripts
        .binding(SCRIPT_FPS_GAMEPLAY_PROVIDER_ID)
        .map(|binding| (SCRIPT_FPS_GAMEPLAY_PROVIDER_ID, binding))
        .or_else(|| {
            runtime
                .scripts
                .binding(LUA_FPS_GAMEPLAY_PROVIDER_ID)
                .map(|binding| (LUA_FPS_GAMEPLAY_PROVIDER_ID, binding))
        })
        .ok_or_else(|| {
            format!(
                "FPS game module requires runtime scripting binding for consumer '{}'",
                SCRIPT_FPS_GAMEPLAY_PROVIDER_ID
            )
        })?;
    let operation = binding.operation.ok_or_else(|| {
        format!(
            "runtime scripting binding '{}' must declare an operation",
            binding_id
        )
    })?;
    let character_menu_policy = if matches!(_target, GameModuleTarget::Server) {
        None
    } else {
        let binding = runtime
            .scripts
            .binding(SCRIPT_FPS_CHARACTER_MENU_PROVIDER_ID)
            .ok_or_else(|| {
                format!(
                    "FPS client module requires runtime scripting binding for consumer '{}'",
                    SCRIPT_FPS_CHARACTER_MENU_PROVIDER_ID
                )
            })?;
        let operation = binding.operation.ok_or_else(|| {
            format!(
                "runtime scripting binding '{}' must declare an operation",
                SCRIPT_FPS_CHARACTER_MENU_PROVIDER_ID
            )
        })?;
        Some(Arc::new(ScriptFpsCharacterMenuPolicyProvider::new(
            binding.script_ref,
            operation,
        )))
    };
    Ok(Arc::new(FpsGameModule {
        policy: Arc::new(
            LuaFpsGameplayPolicyProvider::new(binding.script_ref).with_policy_operation(operation),
        ),
        character_menu_policy,
    }))
}

pub const fn factory_registration() -> GameModuleFactoryRegistration {
    GameModuleFactoryRegistration::new(FPS_GAME_MODULE_ID, create_fps_module)
        .with_descriptor(descriptor_v2)
        .with_activation(activate)
}

pub fn activate() -> Result<(), String> {
    // Descriptor publication is owned by the dynamically loaded fps-game plugin.
    // This distribution-side implementation activation deliberately has no host/service side effects.
    Ok(())
}

pub const fn bootstrap_registration() -> GameModuleBootstrapRegistration {
    GameModuleBootstrapRegistration::new(FPS_GAME_MODULE_ID, activate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_game_module_api::{GameModuleDescriptorV2, GameModuleGameplayProviderRole};
    use newengine_game_module_fps_contract::descriptor_v1_compat;
    use newengine_service_api::{Cardinality, RuntimeUnitRequirementDescriptor};

    #[test]
    fn fps_descriptor_declares_gameplay_ui_provider() {
        let descriptor = descriptor_v2();
        descriptor.validate().unwrap();
        assert_eq!(descriptor.module_id, FPS_GAME_MODULE_ID);
        assert!(descriptor
            .providers
            .iter()
            .any(|provider| provider.role == GameModuleGameplayProviderRole::GameplaySystem));
        assert!(descriptor.providers.iter().any(|provider| provider.role
            == GameModuleGameplayProviderRole::GameplayUi
            && provider.provider_id == "newengine.gameplay.fps.inventory-hud"));
        let requirements = descriptor.runtime_unit_requirements();
        for capability in [
            newengine_game_module_api::GAME_SCENE_BOOTSTRAP_CAPABILITY,
            newengine_game_module_api::GAME_WORLD_RUNTIME_CAPABILITY,
            newengine_game_module_api::GAME_INPUT_PROFILE_CAPABILITY,
            newengine_game_module_api::RENDER_FEATURE_CAPABILITY,
        ] {
            assert!(requirements
                .iter()
                .any(|requirement| requirement.capability == capability && requirement.required));
        }
        assert_eq!(
            requirements
                .iter()
                .find(|requirement| requirement.capability
                    == newengine_game_module_api::RENDER_FEATURE_CAPABILITY)
                .unwrap()
                .cardinality,
            Cardinality::Many
        );
    }

    #[test]
    fn v1_compatibility_response_normalizes_to_native_v2_requirements() {
        let native = descriptor_v2();
        let normalized = GameModuleDescriptorV2::from_v1(descriptor_v1_compat())
            .expect("FPS V1 compatibility descriptor must normalize to V2");
        let requirement_map = |requirements: &[RuntimeUnitRequirementDescriptor]| {
            requirements
                .iter()
                .map(|requirement| (requirement.capability.clone(), requirement.clone()))
                .collect::<std::collections::BTreeMap<_, _>>()
        };
        assert_eq!(
            requirement_map(&normalized.requirements),
            requirement_map(&native.requirements)
        );
        assert_eq!(normalized.providers, native.providers);
        assert_eq!(normalized.module_id, native.module_id);
    }

    #[test]
    fn fps_factory_resolves_authored_runtime_scripting_binding() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "newengine-fps-binding-regression-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create temporary project root");
        let manifest_path = root.join("game.toml");
        std::fs::write(
            &manifest_path,
            r#"
format_version = 1
id = "fps-binding-regression"
name = "FPS Binding Regression"
launch_profile = "game"
runtime_profile = "newengine.runtime-profile.game-ready"
game_module = "newengine.game-module.fps"

[scripting]
runtime = "lua"

[scripting.modules]
fps_runtime = "scripts:/fps_gameplay.ysc"
character_menu = "scripts:/character_menu.ysc"

[scripting.bindings."newengine.gameplay.fps.lua-policy"]
module = "fps_runtime"
operation = "gameplay_policy"

[scripting.bindings."newengine.gameplay.fps.character-menu.script-policy"]
module = "character_menu"
operation = "character_menu_policy"
"#,
        )
        .expect("write temporary game manifest");

        let project = newengine_project_runtime::load_project_from_request(&manifest_path)
            .expect("load project with FPS scripting binding");
        let runtime = RuntimeCompositionContext::from_project(&project);
        let module = create_fps_module(&runtime, GameModuleTarget::Client)
            .expect("FPS module should resolve scripting binding from runtime context");

        assert_eq!(module.descriptor().module_id, FPS_GAME_MODULE_ID);
        let _ = std::fs::remove_dir_all(root);
    }
}
