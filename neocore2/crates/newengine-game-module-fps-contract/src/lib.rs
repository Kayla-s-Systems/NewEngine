#![forbid(unsafe_op_in_unsafe_fn)]

//! Contract-only description of the reusable FPS game module.
//! No gameplay implementation, runtime profile, host, project content, UI, world or assets live here.

use newengine_game_module_api::{
    GameModuleDescriptorV1, GameModuleDescriptorV2, GameModuleGameplayProviderRole,
    GameModuleProviderRef, GameModuleProviderRefV2, GameModuleProviderRole,
    GAME_INPUT_PROFILE_CAPABILITY, GAME_MODULE_CONTRACT_V1, GAME_MODULE_CONTRACT_V2,
    GAME_SCENE_BOOTSTRAP_CAPABILITY, GAME_WORLD_RUNTIME_CAPABILITY, RENDER_FEATURE_CAPABILITY,
};
use newengine_service_api::{Cardinality, RuntimeUnitRequirementDescriptor};

pub const FPS_GAME_MODULE_ID: &str = "newengine.game-module.fps";
pub const FPS_GAME_MODULE_VERSION: &str = "1.0.0";

pub fn descriptor_v2() -> GameModuleDescriptorV2 {
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
            RuntimeUnitRequirementDescriptor::required(GAME_SCENE_BOOTSTRAP_CAPABILITY),
            RuntimeUnitRequirementDescriptor::required(GAME_WORLD_RUNTIME_CAPABILITY),
            RuntimeUnitRequirementDescriptor::required(GAME_INPUT_PROFILE_CAPABILITY),
            RuntimeUnitRequirementDescriptor::required(RENDER_FEATURE_CAPABILITY)
                .with_cardinality(Cardinality::Many),
        ],
        providers: vec![
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
        ],
    }
}

pub fn descriptor_v1_compat() -> GameModuleDescriptorV1 {
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
        descriptor_v2()
            .providers
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_is_valid_and_contract_only() {
        let descriptor = descriptor_v2();
        descriptor.validate().unwrap();
        assert_eq!(descriptor.module_id, FPS_GAME_MODULE_ID);
        assert_eq!(descriptor.version, FPS_GAME_MODULE_VERSION);
        assert_eq!(descriptor.providers.len(), 4);
    }
}
