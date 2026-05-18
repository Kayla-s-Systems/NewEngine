use newengine_ecs_api::{
    EcsCommandRequest, EcsCommandResponse, EcsInvokeRequest, EcsServiceInfo, EcsSnapshotRequest,
    EcsWorldSnapshot, EcsWorldSummary, ECS_SERVICE_METHOD_COMMAND_JSON_V1, ECS_SERVICE_METHOD_SNAPSHOT_JSON_V1,
    ECS_SERVICE_METHOD_SUMMARY_JSON_V1, ENGINE_ECS_SERVICE_ID,
};
use newengine_plugin_api::HostApiV1;

use crate::service_runtime::GenericJsonServiceClient;

/// Host-side JSON client for the `engine.ecs` gateway.
///
/// This keeps tools/runtime service consumers on gateway DTOs instead of importing
/// `newengine_ecs::World` directly.
#[derive(Clone)]
pub struct EcsServiceClient {
    service: GenericJsonServiceClient,
}

impl EcsServiceClient {
    #[inline]
    pub fn new(host: HostApiV1) -> Self {
        Self { service: GenericJsonServiceClient::new(host, ENGINE_ECS_SERVICE_ID) }
    }

    #[inline]
    pub fn info(&self) -> Result<EcsServiceInfo, String> {
        let bytes = self.service.info_json()?;
        decode_json(&bytes)
    }

    #[inline]
    pub fn summary(&self) -> Result<EcsWorldSummary, String> {
        let bytes = self.service.call_raw(ECS_SERVICE_METHOD_SUMMARY_JSON_V1, Vec::new())?;
        decode_json(&bytes)
    }

    #[inline]
    pub fn snapshot(&self, req: EcsSnapshotRequest) -> Result<EcsWorldSnapshot, String> {
        let payload = encode_json(&req)?;
        let bytes = self.service.call_raw(ECS_SERVICE_METHOD_SNAPSHOT_JSON_V1, payload)?;
        decode_json(&bytes)
    }

    #[inline]
    pub fn command(&self, req: EcsCommandRequest) -> Result<EcsCommandResponse, String> {
        let payload = encode_json(&req)?;
        let bytes = self.service.call_raw(ECS_SERVICE_METHOD_COMMAND_JSON_V1, payload)?;
        decode_json(&bytes)
    }

    #[inline]
    pub fn invoke(&self, req: EcsInvokeRequest) -> Result<serde_json::Value, String> {
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
