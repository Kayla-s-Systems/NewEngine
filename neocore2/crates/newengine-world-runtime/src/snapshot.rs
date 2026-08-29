use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_scene::{SceneAsset, SceneAssetOptions};
use newengine_service_kit::ok_json;
use newengine_world_api::{
    WorldLoadSnapshotRequest, WorldLoadSnapshotResponse, WorldRestoreSnapshotRequest,
    WorldRestoreSnapshotResponse, WorldSaveSnapshotRequest, WorldSaveSnapshotResponse,
    WorldSnapshotRequest, WorldSnapshotResponse,
};

use crate::{
    payload::decode_blob,
    service::{EngineWorldGatewayService, WORLD_SNAPSHOT_SCHEMA_V1},
};

impl EngineWorldGatewayService {
    fn capture_scene_payload(&self) -> Result<serde_json::Value, RString> {
        let scene_lock = self.scene.scene();
        let mut scene = scene_lock.write();
        let asset = scene.to_asset(SceneAssetOptions {
            include_empty_entities: true,
        });
        serde_json::to_value(asset).map_err(|error| RString::from(error.to_string()))
    }

    pub(crate) fn snapshot(
        &self,
        request: WorldSnapshotRequest,
    ) -> Result<WorldSnapshotResponse, RString> {
        let scene_payload = request
            .include_scene_payload
            .then(|| self.capture_scene_payload())
            .transpose()?;

        Ok(WorldSnapshotResponse {
            schema: WORLD_SNAPSHOT_SCHEMA_V1.to_owned(),
            state: self.runtime_state(request.include_cells),
            scene_payload,
        })
    }

    pub(crate) fn snapshot_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let request = match decode_blob::<WorldSnapshotRequest>(&payload) {
            Ok(request) => request,
            Err(error) => return RResult::RErr(error),
        };
        match self.snapshot(request) {
            Ok(response) => ok_json(response),
            Err(error) => RResult::RErr(error),
        }
    }

    pub(crate) fn restore_snapshot(
        &self,
        mut request: WorldRestoreSnapshotRequest,
    ) -> Result<WorldRestoreSnapshotResponse, RString> {
        if request.replace_scene {
            if let Some(scene_payload) = request.snapshot.scene_payload.take() {
                let asset =
                    serde_json::from_value::<SceneAsset>(scene_payload).map_err(|error| {
                        RString::from(format!(
                            "world snapshot scene payload decode failed: {error}"
                        ))
                    })?;
                let scene_lock = self.scene.scene();
                let mut scene = scene_lock.write();
                scene.load_asset(&asset).map_err(|error| {
                    RString::from(format!("world snapshot scene restore failed: {error}"))
                })?;
            }
        }

        let restored = request.snapshot.state;
        let (_, desired_cells) = Self::build_partition_cache(&restored.partition);
        {
            let mut state = self.state.lock();
            state.world_instance_id = restored.world_instance_id;
            state.phase = restored.phase;
            state.deterministic = restored.deterministic;
            state.boot_sequence = restored.boot_sequence;
            state.partition = restored.partition;
            state.active_cells = restored.active_cells;
            state.desired_cells = desired_cells;
            state.notes = restored.notes;
            state.notes.push(
                "snapshot restored through engine.world; scene payload restored through engine.scene-compatible asset contract"
                    .to_owned(),
            );
        }

        Ok(WorldRestoreSnapshotResponse {
            ok: true,
            state: self.runtime_state(true),
        })
    }

    pub(crate) fn restore_snapshot_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let request = match decode_blob::<WorldRestoreSnapshotRequest>(&payload) {
            Ok(request) => request,
            Err(error) => return RResult::RErr(error),
        };
        match self.restore_snapshot(request) {
            Ok(response) => ok_json(response),
            Err(error) => RResult::RErr(error),
        }
    }

    pub(crate) fn save_snapshot(
        &self,
        request: WorldSaveSnapshotRequest,
    ) -> Result<WorldSaveSnapshotResponse, RString> {
        let snapshot = self.snapshot(WorldSnapshotRequest {
            include_scene_payload: request.include_scene_payload,
            include_cells: request.include_cells,
        })?;
        let payload_text = serde_json::to_string_pretty(&snapshot)
            .map_err(|error| RString::from(error.to_string()))?;

        Ok(WorldSaveSnapshotResponse {
            ok: true,
            storage: "caller-owned".to_owned(),
            target_ref: request.target_ref,
            snapshot,
            payload_text,
        })
    }

    pub(crate) fn save_snapshot_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let request = match decode_blob::<WorldSaveSnapshotRequest>(&payload) {
            Ok(request) => request,
            Err(error) => return RResult::RErr(error),
        };
        match self.save_snapshot(request) {
            Ok(response) => ok_json(response),
            Err(error) => RResult::RErr(error),
        }
    }

    pub(crate) fn load_snapshot(
        &self,
        request: WorldLoadSnapshotRequest,
    ) -> Result<WorldLoadSnapshotResponse, RString> {
        let snapshot = match (request.snapshot, request.payload) {
            (Some(snapshot), _) => snapshot,
            (None, Some(payload)) => serde_json::from_value::<WorldSnapshotResponse>(payload)
                .map_err(|error| {
                    RString::from(format!(
                        "world.load_snapshot_json_v1 payload decode failed: {error}"
                    ))
                })?,
            (None, None) => {
                return Err(RString::from(
                    "world.load_snapshot_json_v1 requires snapshot or payload",
                ))
            }
        };

        let restored = self.restore_snapshot(WorldRestoreSnapshotRequest {
            snapshot,
            replace_scene: request.replace_scene,
        })?;

        Ok(WorldLoadSnapshotResponse {
            ok: restored.ok,
            state: restored.state,
        })
    }

    pub(crate) fn load_snapshot_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let request = match decode_blob::<WorldLoadSnapshotRequest>(&payload) {
            Ok(request) => request,
            Err(error) => return RResult::RErr(error),
        };
        match self.load_snapshot(request) {
            Ok(response) => ok_json(response),
            Err(error) => RResult::RErr(error),
        }
    }
}
