#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use newengine_service_api::{
    Cardinality, RuntimeUnitRequirementDescriptor, RuntimeUnitRequirementSpec,
};

pub const GAME_MODULE_SERVICE_ID: &str = "engine.game.module";
pub const GAME_MODULE_DESCRIBE_METHOD_V1: &str = "game.describe_v1";
pub const GAME_MODULE_DESCRIBE_METHOD_V2: &str = "game.describe_v2";
pub const GAME_MODULE_CONTRACT_V1: &str = "newengine.game-module/v1";
pub const GAME_MODULE_CONTRACT_V2: &str = "newengine.game-module/v2";

/// Normative GameModule contract. V1 remains decodable as migration vocabulary, but all
/// first-party/native publication targets V2.
pub const GAME_MODULE_CONTRACT_SPEC: newengine_contract_api::ContractSpec =
    newengine_contract_api::ContractSpec::new(
        "game.module.contract",
        newengine_contract_api::ContractKind::Abi,
        newengine_contract_api::ContractVersion::major(2),
        newengine_contract_api::ContractCompatibility::Exact,
        "newengine-game-module-api",
        Some(GAME_MODULE_CONTRACT_V2),
    );

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Complete legacy V1 provider-role vocabulary. Descriptor V2 never references this enum.
/// Subsystem variants normalize into generic runtime-unit requirements; gameplay variants normalize
/// into `GameModuleGameplayProviderRole`.
pub enum GameModuleProviderRole {
    SceneBootstrap,
    WorldRuntime,
    InputProfile,
    RenderFeature,

    // Legacy gameplay-role values normalize into the dedicated V2 gameplay vocabulary.
    GameplayContent,
    GameplaySystem,
    GameplayUi,
    GameplayPhysicsQueries,
}

pub const GAME_SCENE_BOOTSTRAP_CAPABILITY: &str =
    newengine_service_api::runtime_unit_capability::GAME_SCENE_BOOTSTRAP;
pub const GAME_WORLD_RUNTIME_CAPABILITY: &str =
    newengine_service_api::runtime_unit_capability::GAME_WORLD_RUNTIME;
pub const GAME_INPUT_PROFILE_CAPABILITY: &str =
    newengine_service_api::runtime_unit_capability::GAME_INPUT_PROFILE;
pub const RENDER_FEATURE_CAPABILITY: &str =
    newengine_service_api::runtime_unit_capability::RENDER_FEATURE;

impl GameModuleProviderRole {
    /// V1 compatibility mapping only. Native V2 descriptors store the owned requirement directly.
    pub const fn runtime_unit_requirement(
        self,
        required: bool,
    ) -> Option<RuntimeUnitRequirementSpec> {
        let base = match self {
            Self::SceneBootstrap => {
                RuntimeUnitRequirementSpec::required(GAME_SCENE_BOOTSTRAP_CAPABILITY)
            }
            Self::WorldRuntime => {
                RuntimeUnitRequirementSpec::required(GAME_WORLD_RUNTIME_CAPABILITY)
            }
            Self::InputProfile => {
                RuntimeUnitRequirementSpec::required(GAME_INPUT_PROFILE_CAPABILITY)
            }
            Self::RenderFeature => RuntimeUnitRequirementSpec::required(RENDER_FEATURE_CAPABILITY)
                .with_cardinality(Cardinality::Many),
            Self::GameplayContent
            | Self::GameplaySystem
            | Self::GameplayUi
            | Self::GameplayPhysicsQueries => return None,
        };
        Some(if required {
            base
        } else {
            RuntimeUnitRequirementSpec {
                strength: newengine_service_api::RequirementStrength::Optional,
                cardinality: match base.cardinality {
                    Cardinality::One => Cardinality::ZeroOrOne,
                    other => other,
                },
                ..base
            }
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameModuleProviderRef {
    pub role: Option<GameModuleProviderRole>,
    pub provider_id: String,
    pub interface: String,
    pub required: bool,
}

/// Native V2 provider vocabulary. Runtime subsystem composition is intentionally absent:
/// scene/world/input/render belong to the generic capability graph in `requirements`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameModuleGameplayProviderRole {
    GameplayContent,
    GameplaySystem,
    GameplayUi,
    GameplayPhysicsQueries,
}

impl GameModuleGameplayProviderRole {
    #[inline]
    pub const fn from_v1(role: GameModuleProviderRole) -> Option<Self> {
        match role {
            GameModuleProviderRole::GameplayContent => Some(Self::GameplayContent),
            GameModuleProviderRole::GameplaySystem => Some(Self::GameplaySystem),
            GameModuleProviderRole::GameplayUi => Some(Self::GameplayUi),
            GameModuleProviderRole::GameplayPhysicsQueries => Some(Self::GameplayPhysicsQueries),
            GameModuleProviderRole::SceneBootstrap
            | GameModuleProviderRole::WorldRuntime
            | GameModuleProviderRole::InputProfile
            | GameModuleProviderRole::RenderFeature => None,
        }
    }

    #[inline]
    pub const fn to_v1(self) -> GameModuleProviderRole {
        match self {
            Self::GameplayContent => GameModuleProviderRole::GameplayContent,
            Self::GameplaySystem => GameModuleProviderRole::GameplaySystem,
            Self::GameplayUi => GameModuleProviderRole::GameplayUi,
            Self::GameplayPhysicsQueries => GameModuleProviderRole::GameplayPhysicsQueries,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameModuleProviderRefV2 {
    pub role: GameModuleGameplayProviderRole,
    pub provider_id: String,
    pub interface: String,
    pub required: bool,
}

impl GameModuleProviderRefV2 {
    #[inline]
    pub fn into_v1_compat(self) -> GameModuleProviderRef {
        GameModuleProviderRef {
            role: Some(self.role.to_v1()),
            provider_id: self.provider_id,
            interface: self.interface,
            required: self.required,
        }
    }
}

/// Legacy GameModule wire descriptor. Kept for decode/service compatibility only.
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
    pub fn runtime_unit_requirements(&self) -> Vec<RuntimeUnitRequirementSpec> {
        self.providers
            .iter()
            .filter_map(|provider| {
                provider
                    .role
                    .and_then(|role| role.runtime_unit_requirement(provider.required))
            })
            .collect()
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = validate_common_fields(
            &self.contract,
            GAME_MODULE_CONTRACT_V1,
            &self.module_id,
            &self.version,
            &self.required_services,
        );
        validate_v1_providers(&self.providers, &mut errors);
        finish_validation(errors)
    }
}

/// Native capability-first GameModule descriptor.
///
/// Runtime subsystem composition is expressed exclusively through `requirements`. `providers`
/// contains only in-process gameplay trait/provider bindings; subsystem roles are not representable in V2.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameModuleDescriptorV2 {
    pub contract: String,
    pub module_id: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub required_services: Vec<String>,
    pub requirements: Vec<RuntimeUnitRequirementDescriptor>,
    pub providers: Vec<GameModuleProviderRefV2>,
}

impl Default for GameModuleDescriptorV2 {
    fn default() -> Self {
        Self {
            contract: GAME_MODULE_CONTRACT_V2.to_owned(),
            module_id: String::new(),
            version: "0.0.0".to_owned(),
            capabilities: Vec::new(),
            required_services: Vec::new(),
            requirements: Vec::new(),
            providers: Vec::new(),
        }
    }
}

impl GameModuleDescriptorV2 {
    #[inline]
    pub fn runtime_unit_requirements(&self) -> Vec<RuntimeUnitRequirementDescriptor> {
        self.requirements.clone()
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = validate_common_fields(
            &self.contract,
            GAME_MODULE_CONTRACT_V2,
            &self.module_id,
            &self.version,
            &self.required_services,
        );
        validate_v2_providers(&self.providers, &mut errors);

        let mut requirements = BTreeSet::new();
        for requirement in &self.requirements {
            if let Err(error) = requirement.validate() {
                errors.push(error);
                continue;
            }
            let capability = requirement.capability.trim();
            if !requirements.insert(capability.to_owned()) {
                errors.push(format!(
                    "duplicate game-module runtime requirement capability '{capability}'"
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Normalizes a V1 descriptor into the V2 model. Legacy subsystem provider roles become
    /// capability requirements; gameplay providers remain provider references.
    pub fn from_v1(v1: GameModuleDescriptorV1) -> Result<Self, Vec<String>> {
        v1.validate()?;
        let mut requirements = BTreeMap::<String, RuntimeUnitRequirementDescriptor>::new();
        let mut providers = Vec::new();
        for provider in v1.providers {
            let role = provider
                .role
                .expect("validated V1 provider must have a role before normalization");
            if let Some(spec) = role.runtime_unit_requirement(provider.required) {
                let incoming = RuntimeUnitRequirementDescriptor::from_static(spec);
                requirements
                    .entry(incoming.capability.clone())
                    .and_modify(|existing| {
                        existing.required |= incoming.required;
                        if matches!(incoming.cardinality, Cardinality::Many) {
                            existing.cardinality = Cardinality::Many;
                        }
                    })
                    .or_insert(incoming);
            } else {
                providers.push(GameModuleProviderRefV2 {
                    role: GameModuleGameplayProviderRole::from_v1(role)
                        .expect("non-runtime V1 role must map to a V2 gameplay role"),
                    provider_id: provider.provider_id,
                    interface: provider.interface,
                    required: provider.required,
                });
            }
        }
        let descriptor = Self {
            contract: GAME_MODULE_CONTRACT_V2.to_owned(),
            module_id: v1.module_id,
            version: v1.version,
            capabilities: v1.capabilities,
            required_services: v1.required_services,
            requirements: requirements.into_values().collect(),
            providers,
        };
        descriptor.validate()?;
        Ok(descriptor)
    }
}

fn validate_common_fields(
    contract: &str,
    expected_contract: &str,
    module_id: &str,
    version: &str,
    required_services: &[String],
) -> Vec<String> {
    let mut errors = Vec::new();
    if contract != expected_contract {
        errors.push(format!(
            "game-module contract mismatch expected='{expected_contract}' got='{contract}'"
        ));
    }
    if module_id.trim().is_empty() {
        errors.push("game-module module_id must not be empty".to_owned());
    }
    if version.trim().is_empty() {
        errors.push("game-module version must not be empty".to_owned());
    }

    let mut services = BTreeSet::new();
    for service in required_services {
        let service = service.trim();
        if service.is_empty() {
            errors.push("game-module required_services contains an empty id".to_owned());
        } else if !services.insert(service.to_owned()) {
            errors.push(format!("duplicate required service '{service}'"));
        }
    }
    errors
}

fn validate_v1_providers(providers: &[GameModuleProviderRef], errors: &mut Vec<String>) {
    let mut provider_ids = BTreeSet::new();
    for provider in providers {
        if provider.role.is_none() {
            errors.push(format!(
                "game-module provider '{}' has no role",
                provider.provider_id
            ));
        }
        validate_provider_identity(
            &provider.provider_id,
            &provider.interface,
            &mut provider_ids,
            errors,
        );
    }
}

fn validate_v2_providers(providers: &[GameModuleProviderRefV2], errors: &mut Vec<String>) {
    let mut provider_ids = BTreeSet::new();
    for provider in providers {
        validate_provider_identity(
            &provider.provider_id,
            &provider.interface,
            &mut provider_ids,
            errors,
        );
    }
}

fn validate_provider_identity(
    provider_id: &str,
    interface: &str,
    provider_ids: &mut BTreeSet<String>,
    errors: &mut Vec<String>,
) {
    let provider_id = provider_id.trim();
    if provider_id.is_empty() {
        errors.push("game-module provider id must not be empty".to_owned());
    } else if !provider_ids.insert(provider_id.to_owned()) {
        errors.push(format!("duplicate game-module provider '{provider_id}'"));
    }
    if interface.trim().is_empty() {
        errors.push(format!(
            "game-module provider '{provider_id}' interface must not be empty"
        ));
    }
}

fn finish_validation(errors: Vec<String>) -> Result<(), Vec<String>> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_wire_schema_cannot_decode_legacy_subsystem_provider_roles() {
        let json = format!(
            r#"{{
                "contract": "{}",
                "module_id": "test.game",
                "providers": [{{
                    "role": "scene_bootstrap",
                    "provider_id": "legacy.scene",
                    "interface": "legacy.scene.v1",
                    "required": true
                }}]
            }}"#,
            GAME_MODULE_CONTRACT_V2
        );
        assert!(serde_json::from_str::<GameModuleDescriptorV2>(&json).is_err());
    }

    #[test]
    fn v1_normalizes_subsystem_roles_into_v2_requirements() {
        let v1 = GameModuleDescriptorV1 {
            module_id: "test.game".to_owned(),
            providers: vec![
                GameModuleProviderRef {
                    role: Some(GameModuleProviderRole::SceneBootstrap),
                    provider_id: "legacy.scene".to_owned(),
                    interface: "legacy.scene.v1".to_owned(),
                    required: true,
                },
                GameModuleProviderRef {
                    role: Some(GameModuleProviderRole::RenderFeature),
                    provider_id: "legacy.render".to_owned(),
                    interface: "legacy.render.v1".to_owned(),
                    required: true,
                },
                GameModuleProviderRef {
                    role: Some(GameModuleProviderRole::GameplaySystem),
                    provider_id: "test.system".to_owned(),
                    interface: "gameplay.system.v1".to_owned(),
                    required: true,
                },
            ],
            ..GameModuleDescriptorV1::default()
        };
        let v2 = GameModuleDescriptorV2::from_v1(v1).expect("V1 must normalize");
        assert_eq!(v2.contract, GAME_MODULE_CONTRACT_V2);
        assert!(v2.requirements.iter().any(|requirement| {
            requirement.capability == GAME_SCENE_BOOTSTRAP_CAPABILITY
                && requirement.cardinality == Cardinality::One
        }));
        assert!(v2.requirements.iter().any(|requirement| {
            requirement.capability == RENDER_FEATURE_CAPABILITY
                && requirement.cardinality == Cardinality::Many
        }));
        assert_eq!(v2.providers.len(), 1);
        assert_eq!(
            v2.providers[0].role,
            GameModuleGameplayProviderRole::GameplaySystem
        );
    }

    #[test]
    fn v2_serializes_native_owned_requirements() {
        let descriptor = GameModuleDescriptorV2 {
            module_id: "test.game".to_owned(),
            requirements: vec![RuntimeUnitRequirementDescriptor::required(
                GAME_SCENE_BOOTSTRAP_CAPABILITY,
            )],
            ..GameModuleDescriptorV2::default()
        };
        let json = serde_json::to_value(&descriptor).unwrap();
        assert_eq!(
            json["requirements"][0]["capability"],
            GAME_SCENE_BOOTSTRAP_CAPABILITY
        );
        assert_eq!(json["requirements"][0]["required"], true);
        assert_eq!(json["requirements"][0]["cardinality"], "one");
    }
}
