#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use abi_stable::std_types::{RResult, RString};
use newengine_game_module_api::{
    GameModuleDescriptorV1, GameModuleDescriptorV2, GameModuleGameplayProviderRole,
    GameModuleProviderRef, GameModuleProviderRefV2, GameModuleProviderRole,
    GAME_MODULE_CONTRACT_V1, GAME_MODULE_CONTRACT_V2, GAME_MODULE_DESCRIBE_METHOD_V1,
    GAME_MODULE_DESCRIBE_METHOD_V2, GAME_MODULE_SERVICE_ID,
};
use newengine_game_module_composition::{
    GameModuleBootstrapRegistration, GameModuleComposition, GameModuleFactoryRegistration,
    GameModuleProviderSet, GameModuleTarget,
};
use newengine_gameplay_fps::{FpsContentProvider, FpsGameplayProvider, FpsInventoryHudProvider};
use newengine_gameplay_fps_api::FpsGameplayPolicyProvider;
use newengine_gameplay_fps_lua::{LuaFpsGameplayPolicyProvider, LUA_FPS_GAMEPLAY_PROVIDER_ID};
use newengine_gameplay_script_api::ScriptedGameplayProvider;
use newengine_plugin_api::{Blob, CapabilityId, MethodName, ServiceV1, ServiceV1Dyn};
use newengine_project_runtime::RuntimeCompositionContext;
use newengine_service_api::{Cardinality, RuntimeUnitRequirementDescriptor};

pub const FPS_GAME_MODULE_ID: &str = "newengine.game-module.fps";
pub const FPS_GAME_MODULE_VERSION: &str = "1.0.0";

fn gameplay_provider_refs() -> Vec<GameModuleProviderRefV2> {
    vec![
        GameModuleProviderRefV2 {
            role: GameModuleGameplayProviderRole::GameplayContent,
            provider_id: "newengine.gameplay.fps.content.lua-policy".to_owned(),
            interface: "newengine.gameplay.IGameplayContentProvider.v1".to_owned(),
            required: true,
        },
        GameModuleProviderRefV2 {
            role: GameModuleGameplayProviderRole::GameplaySystem,
            provider_id: "newengine.gameplay.fps".to_owned(),
            interface: "newengine.gameplay.IGameplaySystemProvider.v1".to_owned(),
            required: true,
        },
        GameModuleProviderRefV2 {
            role: GameModuleGameplayProviderRole::GameplayUi,
            provider_id: "newengine.gameplay.fps.inventory-hud".to_owned(),
            interface: "newengine.gameplay.IGameplayUiProvider.v1".to_owned(),
            required: false,
        },
        GameModuleProviderRefV2 {
            role: GameModuleGameplayProviderRole::GameplayPhysicsQueries,
            provider_id: "newengine.gameplay.fps.physics-queries".to_owned(),
            interface: "newengine.gameplay.IGameplayPhysicsQueryProvider.v1".to_owned(),
            required: true,
        },
    ]
}

fn descriptor_v2() -> GameModuleDescriptorV2 {
    GameModuleDescriptorV2 {
        contract: GAME_MODULE_CONTRACT_V2.to_owned(),
        module_id: FPS_GAME_MODULE_ID.to_owned(),
        version: FPS_GAME_MODULE_VERSION.to_owned(),
        capabilities: vec![
            "gameplay.fps".to_owned(),
            "gameplay.ui".to_owned(),
            "gameplay.script-policy".to_owned(),
            "target.editor".to_owned(),
            "target.client".to_owned(),
            "target.server.shared-gameplay".to_owned(),
        ],
        required_services: Vec::new(),
        requirements: vec![
            RuntimeUnitRequirementDescriptor::required(
                newengine_game_module_api::GAME_SCENE_BOOTSTRAP_CAPABILITY,
            ),
            RuntimeUnitRequirementDescriptor::required(
                newengine_game_module_api::GAME_WORLD_RUNTIME_CAPABILITY,
            ),
            RuntimeUnitRequirementDescriptor::required(
                newengine_game_module_api::GAME_INPUT_PROFILE_CAPABILITY,
            ),
            RuntimeUnitRequirementDescriptor::required(
                newengine_game_module_api::RENDER_FEATURE_CAPABILITY,
            )
            .with_cardinality(Cardinality::Many),
        ],
        providers: gameplay_provider_refs(),
    }
}

/// Legacy wire response for V1 consumers. Native runtime composition never consumes this path.
fn descriptor_v1_compat() -> GameModuleDescriptorV1 {
    let mut providers = vec![
        GameModuleProviderRef {
            role: Some(GameModuleProviderRole::SceneBootstrap),
            provider_id: "newengine.gameready.scene-bootstrap".to_owned(),
            interface: "newengine.scene.ISceneBootstrapProvider.v1".to_owned(),
            required: true,
        },
        GameModuleProviderRef {
            role: Some(GameModuleProviderRole::WorldRuntime),
            provider_id: "newengine.gameready.world-runtime".to_owned(),
            interface: "newengine.world.IWorldRuntimeProvider.v1".to_owned(),
            required: true,
        },
        GameModuleProviderRef {
            role: Some(GameModuleProviderRole::InputProfile),
            provider_id: "newengine.input-profile.gameready".to_owned(),
            interface: "newengine.input.IInputProfile.v1".to_owned(),
            required: true,
        },
        GameModuleProviderRef {
            role: Some(GameModuleProviderRole::RenderFeature),
            provider_id: "newengine.render-feature.gameready".to_owned(),
            interface: "newengine.render.IRenderFeaturePack.v1".to_owned(),
            required: true,
        },
    ];
    providers.extend(
        gameplay_provider_refs()
            .into_iter()
            .map(GameModuleProviderRefV2::into_v1_compat),
    );
    let v2 = descriptor_v2();
    GameModuleDescriptorV1 {
        contract: GAME_MODULE_CONTRACT_V1.to_owned(),
        module_id: v2.module_id,
        version: v2.version,
        capabilities: v2.capabilities,
        required_services: v2.required_services,
        providers,
    }
}

struct FpsGameModule {
    policy: Arc<LuaFpsGameplayPolicyProvider>,
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
                "contract": GAME_MODULE_CONTRACT_V2,
                "active_module": FPS_GAME_MODULE_ID,
                "methods": [GAME_MODULE_DESCRIBE_METHOD_V2, GAME_MODULE_DESCRIBE_METHOD_V1]
            })
            .to_string(),
        )
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        if method.as_str() != GAME_MODULE_DESCRIBE_METHOD_V2
            && method.as_str() != GAME_MODULE_DESCRIBE_METHOD_V1
        {
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
        let encoded = if method.as_str() == GAME_MODULE_DESCRIBE_METHOD_V2 {
            serde_json::to_vec(&descriptor_v2())
        } else {
            serde_json::to_vec(&descriptor_v1_compat())
        };
        match encoded {
            Ok(bytes) => RResult::ROk(Blob::from(bytes)),
            Err(error) => RResult::RErr(RString::from(format!(
                "encode FPS game-module descriptor: {error}"
            ))),
        }
    }
}

pub const fn factory_registration() -> GameModuleFactoryRegistration {
    GameModuleFactoryRegistration::new(FPS_GAME_MODULE_ID, create_fps_module)
        .with_descriptor(descriptor_v2)
        .with_activation(activate)
}

pub fn activate() -> Result<(), String> {
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

[scripting.bindings."newengine.gameplay.fps.lua-policy"]
module = "fps_runtime"
operation = "gameplay_policy"
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
