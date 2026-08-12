#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable service contract for the `engine.entity` gateway.
//!
//! `engine.entity` is intentionally narrower than `engine.ecs`: it exposes
//! entity identity and coarse lifecycle operations through opaque stable handles.
//! Service/runtime consumers must not depend on the concrete ECS `World` layout.

mod dto;
mod identity;
mod service;

pub use dto::{
    EntityArchetypeDescriptor, EntityArchetypeListResponse, EntityDespawnRequest,
    EntityDespawnResponse, EntityDespawnResult, EntityExistsRequest, EntityExistsResponse,
    EntityInvokeRequest, EntityListRequest, EntityListResponse, EntityRecord, EntitySpawnRequest,
    EntitySpawnResponse, EntitySpawnTransform,
};
pub use identity::{EntityHandle, EntityId};
pub use service::{
    EntityServiceInfo, ENGINE_ENTITY_SERVICE_ID, ENTITY_BACKEND_CAPABILITY_ID,
    ENTITY_BACKEND_SERVICE_SPEC, ENTITY_REQUIRED_METHODS_V1, ENTITY_RUNTIME_CONTRACT_SPEC,
    ENTITY_RUNTIME_REQUIREMENT_SPEC, ENTITY_SERVICE_ID, ENTITY_SERVICE_METHOD_ARCHETYPES_JSON_V1,
    ENTITY_SERVICE_METHOD_DESPAWN_JSON_V1, ENTITY_SERVICE_METHOD_EXISTS_JSON_V1,
    ENTITY_SERVICE_METHOD_INFO, ENTITY_SERVICE_METHOD_INVOKE, ENTITY_SERVICE_METHOD_LIST_JSON_V1,
    ENTITY_SERVICE_METHOD_SHUTDOWN_V1, ENTITY_SERVICE_METHOD_SPAWN_JSON_V1,
};
