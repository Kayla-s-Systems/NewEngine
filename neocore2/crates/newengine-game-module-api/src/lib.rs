#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const GAME_MODULE_SERVICE_ID: &str = "engine.game.module";
pub const GAME_MODULE_DESCRIBE_METHOD_V1: &str = "game.describe_v1";
pub const GAME_MODULE_CONTRACT_V1: &str = "newengine.game-module/v1";
pub const GAME_MODULE_CONTRACT_SPEC: newengine_contract_api::ContractSpec =
    newengine_contract_api::ContractSpec::new(
        "game.module.contract",
        newengine_contract_api::ContractKind::Abi,
        newengine_contract_api::ContractVersion::major(1),
        newengine_contract_api::ContractCompatibility::Exact,
        "newengine-game-module-api",
        Some(GAME_MODULE_CONTRACT_V1),
    );

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameModuleProviderRole {
    SceneBootstrap,
    WorldRuntime,
    GameplayContent,
    GameplaySystem,
    GameplayUi,
    GameplayPhysicsQueries,
    InputProfile,
    RenderFeature,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameModuleProviderRef {
    pub role: Option<GameModuleProviderRole>,
    pub provider_id: String,
    pub interface: String,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameModuleDescriptorV1 {
    pub contract: String,
    pub module_id: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub required_services: Vec<String>,
    pub providers: Vec<GameModuleProviderRef>,
}

impl Default for GameModuleDescriptorV1 {
    fn default() -> Self {
        Self {
            contract: GAME_MODULE_CONTRACT_V1.to_owned(),
            module_id: String::new(),
            version: "0.0.0".to_owned(),
            capabilities: Vec::new(),
            required_services: Vec::new(),
            providers: Vec::new(),
        }
    }
}

impl GameModuleDescriptorV1 {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.contract != GAME_MODULE_CONTRACT_V1 {
            errors.push(format!(
                "game-module contract mismatch expected='{}' got='{}'",
                GAME_MODULE_CONTRACT_V1, self.contract
            ));
        }
        if self.module_id.trim().is_empty() {
            errors.push("game-module module_id must not be empty".to_owned());
        }
        if self.version.trim().is_empty() {
            errors.push("game-module version must not be empty".to_owned());
        }

        let mut services = BTreeSet::new();
        for service in &self.required_services {
            let service = service.trim();
            if service.is_empty() {
                errors.push("game-module required_services contains an empty id".to_owned());
            } else if !services.insert(service.to_owned()) {
                errors.push(format!("duplicate required service '{service}'"));
            }
        }

        let mut providers = BTreeSet::new();
        for provider in &self.providers {
            if provider.role.is_none() {
                errors.push(format!(
                    "game-module provider '{}' has no role",
                    provider.provider_id
                ));
            }
            let provider_id = provider.provider_id.trim();
            if provider_id.is_empty() {
                errors.push("game-module provider id must not be empty".to_owned());
            } else if !providers.insert(provider_id.to_owned()) {
                errors.push(format!("duplicate game-module provider '{provider_id}'"));
            }
            if provider.interface.trim().is_empty() {
                errors.push(format!(
                    "game-module provider '{provider_id}' interface must not be empty"
                ));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_rejects_duplicate_provider_ids() {
        let descriptor = GameModuleDescriptorV1 {
            module_id: "test.game".to_owned(),
            providers: vec![
                GameModuleProviderRef {
                    role: Some(GameModuleProviderRole::GameplaySystem),
                    provider_id: "test.provider".to_owned(),
                    interface: "newengine.gameplay.IGameplaySystemProvider.v1".to_owned(),
                    required: true,
                },
                GameModuleProviderRef {
                    role: Some(GameModuleProviderRole::GameplayUi),
                    provider_id: "test.provider".to_owned(),
                    interface: "newengine.gameplay.IGameplayUiProvider.v1".to_owned(),
                    required: false,
                },
            ],
            ..GameModuleDescriptorV1::default()
        };
        assert!(descriptor.validate().is_err());
    }
}
