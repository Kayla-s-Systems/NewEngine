#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use abi_stable::std_types::{RResult, RString};
use newengine_game_module_api::{
    GameModuleDescriptorV1, GameModuleProviderRef, GameModuleProviderRole, GAME_MODULE_CONTRACT_V1,
    GAME_MODULE_DESCRIBE_METHOD_V1, GAME_MODULE_SERVICE_ID,
};
use newengine_game_module_composition::{
    register_game_module_factory, GameModuleBootstrapRegistration, GameModuleComposition,
    GameModuleFactoryRegistration, GameModuleProviderSet, GameModuleTarget,
};
use newengine_gameplay_fps::{FpsContentProvider, FpsGameplayProvider};
use newengine_gameplay_fps_api::FpsGameplayPolicyProvider;
use newengine_gameplay_fps_lua::{LuaFpsGameplayPolicyProvider, LUA_FPS_GAMEPLAY_PROVIDER_ID};
use newengine_gameplay_script_api::ScriptedGameplayProvider;
use newengine_plugin_api::{Blob, CapabilityId, MethodName, ServiceV1, ServiceV1Dyn};
use newengine_project_runtime::RuntimeCompositionContext;

pub const FPS_GAME_MODULE_ID: &str = "newengine.game-module.fps";
pub const FPS_GAME_MODULE_VERSION: &str = "1.0.0";

fn descriptor_v1() -> GameModuleDescriptorV1 {
    GameModuleDescriptorV1 {
        contract: GAME_MODULE_CONTRACT_V1.to_owned(),
        module_id: FPS_GAME_MODULE_ID.to_owned(),
        version: FPS_GAME_MODULE_VERSION.to_owned(),
        capabilities: vec![
            "gameplay.fps".to_owned(),
            "gameplay.script-policy".to_owned(),
            "target.editor".to_owned(),
            "target.client".to_owned(),
            "target.server.shared-gameplay".to_owned(),
        ],
        required_services: Vec::new(),
        providers: vec![
            GameModuleProviderRef {
                role: Some(GameModuleProviderRole::GameplayContent),
                provider_id: "newengine.gameplay.fps.content.lua-policy".to_owned(),
                interface: "newengine.gameplay.IGameplayContentProvider.v1".to_owned(),
                required: true,
            },
            GameModuleProviderRef {
                role: Some(GameModuleProviderRole::GameplaySystem),
                provider_id: "newengine.gameplay.fps".to_owned(),
                interface: "newengine.gameplay.IGameplaySystemProvider.v1".to_owned(),
                required: true,
            },
            GameModuleProviderRef {
                role: Some(GameModuleProviderRole::GameplayPhysicsQueries),
                provider_id: "newengine.gameplay.fps.physics-queries".to_owned(),
                interface: "newengine.gameplay.IGameplayPhysicsQueryProvider.v1".to_owned(),
                required: true,
            },
        ],
    }
}

struct FpsGameModule {
    policy: Arc<LuaFpsGameplayPolicyProvider>,
}

impl GameModuleComposition for FpsGameModule {
    fn descriptor(&self) -> GameModuleDescriptorV1 {
        descriptor_v1()
    }

    fn providers(&self, _target: GameModuleTarget) -> Result<GameModuleProviderSet, String> {
        let policy_for_content: Arc<dyn FpsGameplayPolicyProvider> = self.policy.clone();
        let policy_for_system: Arc<dyn FpsGameplayPolicyProvider> = self.policy.clone();
        let policy_for_queries: Arc<dyn FpsGameplayPolicyProvider> = self.policy.clone();
        let scripts_for_system: Arc<dyn ScriptedGameplayProvider> = self.policy.clone();
        let scripts_for_queries: Arc<dyn ScriptedGameplayProvider> = self.policy.clone();

        let mut providers = GameModuleProviderSet::default();
        providers
            .gameplay_content
            .push(FpsContentProvider::shared(policy_for_content));
        providers.gameplay_systems.push(FpsGameplayProvider::shared(
            policy_for_system,
            scripts_for_system,
        ));
        providers
            .gameplay_physics_queries
            .push(FpsGameplayProvider::shared(
                policy_for_queries,
                scripts_for_queries,
            ));

        // FPS gameplay is intentionally HUD-free for every target. Client/editor
        // presentation is limited to the provider-neutral runtime technical overlay.
        Ok(providers)
    }
}

fn create_fps_module(
    runtime: &RuntimeCompositionContext,
    _target: GameModuleTarget,
) -> Result<Arc<dyn GameModuleComposition>, String> {
    let binding = runtime
        .scripts
        .binding(LUA_FPS_GAMEPLAY_PROVIDER_ID)
        .ok_or_else(|| {
            format!(
                "FPS game module requires runtime scripting binding for consumer '{}'",
                LUA_FPS_GAMEPLAY_PROVIDER_ID
            )
        })?;
    let operation = binding.operation.ok_or_else(|| {
        format!(
            "runtime scripting binding '{}' must declare an operation",
            LUA_FPS_GAMEPLAY_PROVIDER_ID
        )
    })?;
    Ok(Arc::new(FpsGameModule {
        policy: Arc::new(
            LuaFpsGameplayPolicyProvider::new(binding.script_ref).with_policy_operation(operation),
        ),
    }))
}

struct FpsGameModuleDescriptorService;

impl ServiceV1 for FpsGameModuleDescriptorService {
    fn id(&self) -> CapabilityId {
        RString::from(GAME_MODULE_SERVICE_ID)
    }

    fn describe(&self) -> RString {
        RString::from(
            serde_json::json!({
                "id": GAME_MODULE_SERVICE_ID,
                "contract": GAME_MODULE_CONTRACT_V1,
                "active_module": FPS_GAME_MODULE_ID,
                "methods": [GAME_MODULE_DESCRIBE_METHOD_V1]
            })
            .to_string(),
        )
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        if method.as_str() != GAME_MODULE_DESCRIBE_METHOD_V1 {
            return RResult::RErr(RString::from("unknown game-module method"));
        }
        if !payload.is_empty() {
            if let Ok(request) = serde_json::from_slice::<serde_json::Value>(payload.as_slice()) {
                if let Some(requested) = request
                    .get("requested_module_id")
                    .and_then(serde_json::Value::as_str)
                {
                    if requested != FPS_GAME_MODULE_ID {
                        return RResult::RErr(RString::from(format!(
                            "active game module '{}' does not satisfy requested '{}'",
                            FPS_GAME_MODULE_ID, requested
                        )));
                    }
                }
            }
        }
        match serde_json::to_vec(&descriptor_v1()) {
            Ok(bytes) => RResult::ROk(Blob::from(bytes)),
            Err(error) => RResult::RErr(RString::from(format!(
                "encode FPS game-module descriptor: {error}"
            ))),
        }
    }
}

pub fn activate() -> Result<(), String> {
    register_game_module_factory(GameModuleFactoryRegistration::new(
        FPS_GAME_MODULE_ID,
        create_fps_module,
    ))?;
    let service = ServiceV1Dyn::from_value(
        FpsGameModuleDescriptorService,
        abi_stable::sabi_trait::TD_Opaque,
    );
    newengine_plugin_host::host_register_service_impl(service)
        .into_result()
        .map_err(|error| format!("register FPS game-module descriptor service: {error}"))?;
    Ok(())
}

pub const fn bootstrap_registration() -> GameModuleBootstrapRegistration {
    GameModuleBootstrapRegistration::new(FPS_GAME_MODULE_ID, activate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fps_descriptor_is_valid_and_declares_shared_gameplay_providers() {
        let descriptor = descriptor_v1();
        descriptor.validate().unwrap();
        assert_eq!(descriptor.module_id, FPS_GAME_MODULE_ID);
        assert!(descriptor
            .providers
            .iter()
            .any(|provider| provider.role == Some(GameModuleProviderRole::GameplaySystem)));
        assert!(descriptor
            .providers
            .iter()
            .all(|provider| provider.role != Some(GameModuleProviderRole::GameplayUi)));
    }
}
