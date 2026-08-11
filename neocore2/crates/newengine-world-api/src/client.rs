use abi_stable::std_types::RString;
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    WorldActiveCellsRequest, WorldBootRequest, WorldBootResponse, WorldLoadSnapshotRequest,
    WorldLoadSnapshotResponse, WorldSaveSnapshotRequest, WorldSaveSnapshotResponse,
    WorldSnapshotRequest, WorldSnapshotResponse, WorldStateRequest, WorldStreamingCellsRequest,
    WorldStreamingCellsResponse, ENGINE_WORLD_SERVICE_ID,
    WORLD_SERVICE_METHOD_ACTIVE_CELLS_JSON_V1, WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1,
    WORLD_SERVICE_METHOD_BOOT_JSON_V1, WORLD_SERVICE_METHOD_LOAD_SNAPSHOT_JSON_V1,
    WORLD_SERVICE_METHOD_SAVE_SNAPSHOT_JSON_V1, WORLD_SERVICE_METHOD_SNAPSHOT_JSON_V1,
    WORLD_SERVICE_METHOD_STATE_JSON_V1, WORLD_SERVICE_METHOD_STREAMING_CELLS_JSON_V1,
};

/// Thin host-side client over `engine.world`.
#[derive(Clone)]
pub struct WorldClient {
    host: HostApiV1,
    service_id: RString,
}

impl WorldClient {
    #[inline]
    pub fn new(host: HostApiV1) -> Self {
        Self {
            host,
            service_id: RString::from(ENGINE_WORLD_SERVICE_ID),
        }
    }

    #[inline]
    fn call_json<Request, Response>(
        &self,
        method: &str,
        request: &Request,
    ) -> Result<Response, String>
    where
        Request: Serialize + ?Sized,
        Response: DeserializeOwned,
    {
        let payload = serde_json::to_vec(request).map_err(|error| error.to_string())?;
        let response = (self.host.call_service_v1)(
            self.service_id.clone(),
            MethodName::from(method),
            Blob::from(payload),
        );
        let bytes = response
            .into_result()
            .map_err(|error| error.to_string())?
            .into_vec();
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())
    }

    #[inline]
    pub fn boot_json_v1(&self, request: &WorldBootRequest) -> Result<WorldBootResponse, String> {
        self.call_json(WORLD_SERVICE_METHOD_BOOT_JSON_V1, request)
    }

    #[inline]
    pub fn state_json_v1(&self, include_cells: bool) -> Result<serde_json::Value, String> {
        self.call_json(
            WORLD_SERVICE_METHOD_STATE_JSON_V1,
            &WorldStateRequest { include_cells },
        )
    }

    #[inline]
    pub fn active_cells_json_v1(&self) -> Result<serde_json::Value, String> {
        self.call_json(
            WORLD_SERVICE_METHOD_ACTIVE_CELLS_JSON_V1,
            &WorldActiveCellsRequest::default(),
        )
    }

    #[inline]
    pub fn snapshot_json_v1(&self) -> Result<serde_json::Value, String> {
        self.call_json(
            WORLD_SERVICE_METHOD_SNAPSHOT_JSON_V1,
            &WorldSnapshotRequest::default(),
        )
    }

    #[inline]
    pub fn snapshot_response_json_v1(&self) -> Result<WorldSnapshotResponse, String> {
        self.call_json(
            WORLD_SERVICE_METHOD_SNAPSHOT_JSON_V1,
            &WorldSnapshotRequest::default(),
        )
    }

    #[inline]
    pub fn save_snapshot_json_v1(
        &self,
        request: &WorldSaveSnapshotRequest,
    ) -> Result<WorldSaveSnapshotResponse, String> {
        self.call_json(WORLD_SERVICE_METHOD_SAVE_SNAPSHOT_JSON_V1, request)
    }

    #[inline]
    pub fn load_snapshot_json_v1(
        &self,
        request: &WorldLoadSnapshotRequest,
    ) -> Result<WorldLoadSnapshotResponse, String> {
        self.call_json(WORLD_SERVICE_METHOD_LOAD_SNAPSHOT_JSON_V1, request)
    }

    #[inline]
    pub fn streaming_cells_json_v1(&self) -> Result<serde_json::Value, String> {
        self.call_json(
            WORLD_SERVICE_METHOD_STREAMING_CELLS_JSON_V1,
            &WorldStreamingCellsRequest {
                include_unloaded: false,
                include_reasons: true,
            },
        )
    }

    #[inline]
    pub fn streaming_cells_response_json_v1(
        &self,
        request: &WorldStreamingCellsRequest,
    ) -> Result<WorldStreamingCellsResponse, String> {
        self.call_json(WORLD_SERVICE_METHOD_STREAMING_CELLS_JSON_V1, request)
    }

    #[inline]
    pub fn apply_stage_json_v1(
        &self,
        request: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.call_json(WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1, &request)
    }
}
