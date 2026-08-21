use newengine_entity_api::{
    EntityDespawnRequest, EntityDespawnResponse, EntityExistsRequest, EntityExistsResponse,
    EntityInvokeRequest, EntityListRequest, EntityListResponse, EntityServiceInfo,
    EntitySpawnRequest, EntitySpawnResponse, ENGINE_ENTITY_SERVICE_ID,
    ENTITY_SERVICE_METHOD_DESPAWN_JSON_V1, ENTITY_SERVICE_METHOD_EXISTS_JSON_V1,
    ENTITY_SERVICE_METHOD_LIST_JSON_V1, ENTITY_SERVICE_METHOD_SPAWN_JSON_V1,
};
use newengine_plugin_api::HostApiV1;

use newengine_runtime_adapter_core::GenericJsonServiceClient;

/// Host-side JSON client for the `engine.entity` gateway.
///
/// This keeps tools/runtime service consumers on opaque entity DTOs instead of
/// importing `newengine_entity::EntityId` directly.
#[derive(Clone)]
pub struct EntityServiceClient {
    service: GenericJsonServiceClient,
}

impl EntityServiceClient {
    #[inline]
    pub fn new(host: HostApiV1) -> Self {
        Self {
            service: GenericJsonServiceClient::new(host, ENGINE_ENTITY_SERVICE_ID),
        }
    }

    #[inline]
    pub fn info(&self) -> Result<EntityServiceInfo, String> {
        let bytes = self.service.info_json()?;
        decode_json(&bytes)
    }

    #[inline]
    pub fn list(&self, req: EntityListRequest) -> Result<EntityListResponse, String> {
        let payload = encode_json(&req)?;
        let bytes = self
            .service
            .call_raw(ENTITY_SERVICE_METHOD_LIST_JSON_V1, payload)?;
        decode_json(&bytes)
    }

    #[inline]
    pub fn exists(&self, req: EntityExistsRequest) -> Result<EntityExistsResponse, String> {
        let payload = encode_json(&req)?;
        let bytes = self
            .service
            .call_raw(ENTITY_SERVICE_METHOD_EXISTS_JSON_V1, payload)?;
        decode_json(&bytes)
    }

    #[inline]
    pub fn spawn(&self, req: EntitySpawnRequest) -> Result<EntitySpawnResponse, String> {
        let payload = encode_json(&req)?;
        let bytes = self
            .service
            .call_raw(ENTITY_SERVICE_METHOD_SPAWN_JSON_V1, payload)?;
        decode_json(&bytes)
    }

    #[inline]
    pub fn despawn(&self, req: EntityDespawnRequest) -> Result<EntityDespawnResponse, String> {
        let payload = encode_json(&req)?;
        let bytes = self
            .service
            .call_raw(ENTITY_SERVICE_METHOD_DESPAWN_JSON_V1, payload)?;
        decode_json(&bytes)
    }

    #[inline]
    pub fn invoke(&self, req: EntityInvokeRequest) -> Result<serde_json::Value, String> {
        let payload = encode_json(&req)?;
        let bytes = self.service.invoke_json(payload)?;
        decode_json(&bytes)
    }
}

#[inline]
fn encode_json<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|e| e.to_string())
}

#[inline]
fn decode_json<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}
