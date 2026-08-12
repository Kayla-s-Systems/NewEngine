use serde::{Deserialize, Serialize};

/// Engine-facing entity service gateway id. Consumers call this facade; the host
/// resolves it to the active provider by descriptor metadata / engine-runtime facts.
pub const ENGINE_ENTITY_SERVICE_ID: &str = "engine.entity";

/// Default/first-party provider service id for future entity backends.
pub const ENTITY_SERVICE_ID: &str = "entity.api";
pub const ENTITY_BACKEND_CAPABILITY_ID: &str = "entity.backend";

pub const ENTITY_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const ENTITY_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const ENTITY_SERVICE_METHOD_SHUTDOWN_V1: &str =
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const ENTITY_SERVICE_METHOD_LIST_JSON_V1: &str = "list_json_v1";
pub const ENTITY_SERVICE_METHOD_ARCHETYPES_JSON_V1: &str = "archetypes_json_v1";
pub const ENTITY_SERVICE_METHOD_EXISTS_JSON_V1: &str = "exists_json_v1";
pub const ENTITY_SERVICE_METHOD_SPAWN_JSON_V1: &str = "spawn_json_v1";
pub const ENTITY_SERVICE_METHOD_DESPAWN_JSON_V1: &str = "despawn_json_v1";

pub const ENTITY_REQUIRED_METHODS_V1: &[&str] = &[
    ENTITY_SERVICE_METHOD_INFO,
    ENTITY_SERVICE_METHOD_INVOKE,
    ENTITY_SERVICE_METHOD_SHUTDOWN_V1,
    ENTITY_SERVICE_METHOD_LIST_JSON_V1,
    ENTITY_SERVICE_METHOD_ARCHETYPES_JSON_V1,
    ENTITY_SERVICE_METHOD_EXISTS_JSON_V1,
    ENTITY_SERVICE_METHOD_SPAWN_JSON_V1,
    ENTITY_SERVICE_METHOD_DESPAWN_JSON_V1,
];

/// Generic backend-family declaration for entity providers.
pub const ENTITY_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "entity",
        ENGINE_ENTITY_SERVICE_ID,
        ENTITY_SERVICE_ID,
        ENTITY_BACKEND_CAPABILITY_ID,
    );

/// Startup validation contract for the engine-facing entity gateway.
pub const ENTITY_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_ENTITY_SERVICE_ID,
        "newengine.entity-api >= 0.1.x",
        ENTITY_REQUIRED_METHODS_V1,
    );

/// Missing `engine.entity` degrades by default; strict profiles can require it.
pub const ENTITY_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        ENTITY_RUNTIME_CONTRACT_SPEC,
        Some(ENTITY_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_ENTITY_BACKEND"),
    );

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl Default for EntityServiceInfo {
    #[inline]
    fn default() -> Self {
        Self {
            protocol: "newengine.entity-api/v1".to_owned(),
            features: vec![
                "opaque-stable-handles".to_owned(),
                "entity-list".to_owned(),
                "entity-exists".to_owned(),
                "entity-lifecycle".to_owned(),
                "archetype-factory-registry".to_owned(),
                "entity-tags".to_owned(),
                "ownership".to_owned(),
                "debug-identity".to_owned(),
            ],
            methods: ENTITY_REQUIRED_METHODS_V1
                .iter()
                .map(|method| (*method).to_owned())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_service_ids_are_gateway_first() {
        assert_eq!(ENGINE_ENTITY_SERVICE_ID, "engine.entity");
        assert_eq!(
            ENTITY_BACKEND_SERVICE_SPEC.engine_gateway_id,
            ENGINE_ENTITY_SERVICE_ID
        );
        assert_eq!(
            ENTITY_BACKEND_SERVICE_SPEC.provider_service_id,
            ENTITY_SERVICE_ID
        );
        assert_eq!(
            ENTITY_BACKEND_SERVICE_SPEC.backend_capability_id,
            ENTITY_BACKEND_CAPABILITY_ID
        );
    }

    #[test]
    fn service_info_advertises_required_methods() {
        let info = EntityServiceInfo::default();
        assert_eq!(info.methods.len(), ENTITY_REQUIRED_METHODS_V1.len());
        assert!(info
            .methods
            .iter()
            .any(|method| method == ENTITY_SERVICE_METHOD_SPAWN_JSON_V1));
    }
}
