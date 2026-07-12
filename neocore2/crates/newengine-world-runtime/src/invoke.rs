use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_service_kit::ok_json;
use newengine_world_api::{
    WorldActiveCellsRequest, WorldApplyStageRequest, WorldBootRequest, WorldInvokeRequest,
    WorldLoadSnapshotRequest, WorldRestoreSnapshotRequest, WorldSaveSnapshotRequest,
    WorldSnapshotRequest, WorldStateRequest, WorldStreamingCellsRequest,
    WORLD_SERVICE_METHOD_ACTIVE_CELLS_JSON_V1, WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1,
    WORLD_SERVICE_METHOD_BOOT_JSON_V1, WORLD_SERVICE_METHOD_LOAD_SNAPSHOT_JSON_V1,
    WORLD_SERVICE_METHOD_PARTITION_JSON_V1, WORLD_SERVICE_METHOD_RESTORE_SNAPSHOT_JSON_V1,
    WORLD_SERVICE_METHOD_SAVE_SNAPSHOT_JSON_V1, WORLD_SERVICE_METHOD_SNAPSHOT_JSON_V1,
    WORLD_SERVICE_METHOD_STATE_JSON_V1, WORLD_SERVICE_METHOD_STREAMING_CELLS_JSON_V1,
};

use crate::{
    payload::{decode_blob, decode_value, json_result},
    service::EngineWorldGatewayService,
};

impl EngineWorldGatewayService {
    pub(crate) fn invoke_json(&self, payload: Blob) -> RResult<Blob, RString> {
        let WorldInvokeRequest { method, payload } =
            match decode_blob::<WorldInvokeRequest>(&payload) {
                Ok(request) => request,
                Err(error) => return RResult::RErr(error),
            };

        match method.as_str() {
            WORLD_SERVICE_METHOD_BOOT_JSON_V1 => {
                let request = match decode_value::<WorldBootRequest>(payload) {
                    Ok(request) => request,
                    Err(error) => return RResult::RErr(error),
                };
                ok_json(self.boot(request))
            }
            WORLD_SERVICE_METHOD_STATE_JSON_V1 => {
                let request = match decode_value::<WorldStateRequest>(payload) {
                    Ok(request) => request,
                    Err(error) => return RResult::RErr(error),
                };
                ok_json(self.state_response(request))
            }
            WORLD_SERVICE_METHOD_ACTIVE_CELLS_JSON_V1 => {
                let request = match decode_value::<WorldActiveCellsRequest>(payload) {
                    Ok(request) => request,
                    Err(error) => return RResult::RErr(error),
                };
                ok_json(self.active_cells_response(request))
            }
            WORLD_SERVICE_METHOD_STREAMING_CELLS_JSON_V1 => {
                let request = match decode_value::<WorldStreamingCellsRequest>(payload) {
                    Ok(request) => request,
                    Err(error) => return RResult::RErr(error),
                };
                ok_json(self.streaming_response(request))
            }
            WORLD_SERVICE_METHOD_PARTITION_JSON_V1 => ok_json(self.partition_response()),
            WORLD_SERVICE_METHOD_SNAPSHOT_JSON_V1 => {
                let request = match decode_value::<WorldSnapshotRequest>(payload) {
                    Ok(request) => request,
                    Err(error) => return RResult::RErr(error),
                };
                json_result(self.snapshot(request))
            }
            WORLD_SERVICE_METHOD_RESTORE_SNAPSHOT_JSON_V1 => {
                let request = match decode_value::<WorldRestoreSnapshotRequest>(payload) {
                    Ok(request) => request,
                    Err(error) => return RResult::RErr(error),
                };
                json_result(self.restore_snapshot(request))
            }
            WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1 => {
                let request = match decode_value::<WorldApplyStageRequest>(payload) {
                    Ok(request) => request,
                    Err(error) => return RResult::RErr(error),
                };
                json_result(self.apply_stage(request))
            }
            WORLD_SERVICE_METHOD_SAVE_SNAPSHOT_JSON_V1 => {
                let request = match decode_value::<WorldSaveSnapshotRequest>(payload) {
                    Ok(request) => request,
                    Err(error) => return RResult::RErr(error),
                };
                json_result(self.save_snapshot(request))
            }
            WORLD_SERVICE_METHOD_LOAD_SNAPSHOT_JSON_V1 => {
                let request = match decode_value::<WorldLoadSnapshotRequest>(payload) {
                    Ok(request) => request,
                    Err(error) => return RResult::RErr(error),
                };
                json_result(self.load_snapshot(request))
            }
            other => RResult::RErr(RString::from(format!(
                "engine.world invoke_json unknown target method '{other}'"
            ))),
        }
    }
}
