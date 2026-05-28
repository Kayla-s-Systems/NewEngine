#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable service contract for the `engine.entity` gateway.
//!
//! `engine.entity` is intentionally narrower than `engine.ecs`: it exposes
//! entity identity and coarse lifecycle operations through opaque stable handles.
//! Service/runtime consumers must not depend on `newengine_entity::EntityId` or
//! the concrete ECS `World` layout.

use newengine_math::collections::prelude::{ne_new_key_type, NeKey};
use serde::{Deserialize, Serialize};


ne_new_key_type! {
    /// Stable, deterministic identifier of an entity across the engine.
    ///
    /// This low-level identity type intentionally lives in `newengine-entity-api`
    /// so API/contract crates can name entities without depending on the concrete
    /// ECS `World` implementation or the higher-level entity runtime crate.
    pub struct EntityId;
}

impl EntityId {
    /// Returns a deterministic, totally ordered representation of the entity id.
    ///
    /// This method intentionally hides the internal generational key layout.
    #[inline]
    pub fn stable_u64(self) -> u64 {
        self.data().as_ffi()
    }
}

/// Engine-facing entity service gateway id. Consumers call this facade; the host
/// resolves it to the active provider by descriptor metadata / engine-runtime facts.
pub const ENGINE_ENTITY_SERVICE_ID: &str = "engine.entity";

/// Default/first-party provider service id for future entity backends.
pub const ENTITY_SERVICE_ID: &str = "entity.api";
pub const ENTITY_BACKEND_CAPABILITY_ID: &str = "entity.backend";

pub const ENTITY_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const ENTITY_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const ENTITY_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const ENTITY_SERVICE_METHOD_LIST_JSON_V1: &str = "list_json_v1";
pub const ENTITY_SERVICE_METHOD_EXISTS_JSON_V1: &str = "exists_json_v1";
pub const ENTITY_SERVICE_METHOD_SPAWN_JSON_V1: &str = "spawn_json_v1";
pub const ENTITY_SERVICE_METHOD_DESPAWN_JSON_V1: &str = "despawn_json_v1";

pub const ENTITY_REQUIRED_METHODS_V1: &[&str] = &[
    ENTITY_SERVICE_METHOD_INFO,
    ENTITY_SERVICE_METHOD_INVOKE,
    ENTITY_SERVICE_METHOD_SHUTDOWN_V1,
    ENTITY_SERVICE_METHOD_LIST_JSON_V1,
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
            ],
            methods: ENTITY_REQUIRED_METHODS_V1
                .iter()
                .map(|it| (*it).to_owned())
                .collect(),
        }
    }
}

/// Opaque service-safe entity handle.
///
/// The value is stable for diagnostics/tool calls but does not expose the native
/// key layout or allow consumers to manufacture a direct `EntityId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct EntityHandle {
    pub stable_id: u64,
}

impl EntityHandle {
    #[inline]
    pub const fn new(stable_id: u64) -> Self {
        Self { stable_id }
    }
}

impl From<EntityId> for EntityHandle {
    #[inline]
    fn from(value: EntityId) -> Self {
        Self::new(value.stable_u64())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityListRequest {
    #[serde(default = "default_entity_limit")]
    pub limit: usize,
}

impl Default for EntityListRequest {
    #[inline]
    fn default() -> Self {
        Self { limit: default_entity_limit() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRecord {
    pub handle: EntityHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EntityListResponse {
    #[serde(default)]
    pub entities: Vec<EntityRecord>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub total_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityExistsRequest {
    pub entity: EntityHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityExistsResponse {
    pub entity: EntityHandle,
    pub exists: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntitySpawnRequest {
    #[serde(default = "default_spawn_count")]
    pub count: usize,
}

impl Default for EntitySpawnRequest {
    #[inline]
    fn default() -> Self {
        Self { count: default_spawn_count() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EntitySpawnResponse {
    #[serde(default)]
    pub entities: Vec<EntityRecord>,
    #[serde(default)]
    pub tick: u64,
    #[serde(default)]
    pub total_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDespawnRequest {
    #[serde(default)]
    pub entities: Vec<EntityHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDespawnResult {
    pub entity: EntityHandle,
    pub ok: bool,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EntityDespawnResponse {
    pub ok: bool,
    #[serde(default)]
    pub results: Vec<EntityDespawnResult>,
    #[serde(default)]
    pub tick: u64,
    #[serde(default)]
    pub total_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityInvokeRequest {
    /// One of `list_json_v1`, `exists_json_v1`, `spawn_json_v1`, `despawn_json_v1`.
    pub method: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[inline]
fn default_entity_limit() -> usize { 4096 }
#[inline]
fn default_spawn_count() -> usize { 1 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_service_ids_are_gateway_first() {
        assert_eq!(ENGINE_ENTITY_SERVICE_ID, "engine.entity");
        assert_eq!(ENTITY_BACKEND_SERVICE_SPEC.engine_gateway_id, ENGINE_ENTITY_SERVICE_ID);
        assert_eq!(ENTITY_BACKEND_SERVICE_SPEC.provider_service_id, ENTITY_SERVICE_ID);
        assert_eq!(ENTITY_BACKEND_SERVICE_SPEC.backend_capability_id, ENTITY_BACKEND_CAPABILITY_ID);
    }

    #[test]
    fn list_request_decodes_with_default_limit() {
        let req: EntityListRequest = serde_json::from_str("{}").expect("defaults decode");
        assert!(req.limit > 0);
    }
}
