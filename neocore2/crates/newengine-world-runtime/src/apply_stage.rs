#![forbid(unsafe_op_in_unsafe_fn)]

use super::*;

impl EngineWorldGatewayService {
    pub(crate) fn apply_stage_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload).and_then(|v| {
            serde_json::from_value::<WorldApplyStageRequest>(v).map_err(|e| e.to_string())
        }) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
        if req.stage != "scene.instantiate" {
            return RResult::RErr(RString::from(format!(
                "world.apply_stage_json_v1 unsupported stage='{}'; mutation must be routed through an explicit apply stage",
                req.stage
            )));
        }

        let mut applied = 0usize;
        let mut undo_commands = Vec::new();
        {
            let scene_lock = self.scene.scene();
            let mut scene = scene_lock.write();
            let mut asset = scene.to_asset(SceneAssetOptions {
                include_empty_entities: true,
            });
            let existing = asset
                .entities
                .iter()
                .map(|e| e.guid)
                .collect::<std::collections::BTreeSet<_>>();
            for command in req.commands.iter() {
                if command.command != "scene.spawn_instance" {
                    return RResult::RErr(RString::from(format!(
                        "world.apply_stage_json_v1 unsupported command='{}' for stage='{}'",
                        command.command, req.stage
                    )));
                }
                let Some(guid) = command.guid else {
                    return RResult::RErr(RString::from(
                        "scene.spawn_instance command requires guid",
                    ));
                };
                if existing.contains(&guid) || asset.entities.iter().any(|e| e.guid == guid) {
                    return RResult::RErr(RString::from(format!(
                        "scene.spawn_instance duplicate guid={guid}"
                    )));
                }
                let transform = match command.transform.clone() {
                    Some(value) => match serde_json::from_value::<TransformAsset>(value) {
                        Ok(v) => Some(v),
                        Err(e) => {
                            return RResult::RErr(RString::from(format!(
                                "scene.spawn_instance transform decode failed: {e}"
                            )))
                        }
                    },
                    None => None,
                };
                asset.entities.push(SceneEntityAsset {
                    guid,
                    name: command.name.clone(),
                    parent: command.parent,
                    transform,
                    definition_ref: command.definition_ref.clone(),
                });
                let mut undo = command.clone();
                undo.command = "scene.despawn_instance".to_owned();
                undo_commands.push(undo);
                applied = applied.saturating_add(1);
            }
            asset.entities.sort_by(|a, b| a.guid.cmp(&b.guid));
            if let Err(e) = scene.load_asset(&asset) {
                return RResult::RErr(RString::from(format!(
                    "world.apply_stage_json_v1 scene apply failed: {e}"
                )));
            }
        }
        {
            let mut state = self.state.lock();
            state.notes.push(format!(
                "apply-stage: stage={} transaction={} applied={} owner=engine.world",
                req.stage, req.transaction_id, applied
            ));
        }
        ok_json(&WorldApplyStageResponse {
            ok: true,
            stage: req.stage,
            transaction_id: req.transaction_id,
            applied_count: applied,
            state: self.runtime_state(true),
            undo_commands,
        })
    }

    pub(crate) fn save_snapshot_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload).and_then(|v| {
            serde_json::from_value::<WorldSaveSnapshotRequest>(v).map_err(|e| e.to_string())
        }) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
        let scene_payload = if req.include_scene_payload {
            let scene_lock = self.scene.scene();
            let mut scene = scene_lock.write();
            let asset = scene.to_asset(SceneAssetOptions {
                include_empty_entities: true,
            });
            match serde_json::to_value(&asset) {
                Ok(value) => Some(value),
                Err(e) => return RResult::RErr(RString::from(e.to_string())),
            }
        } else {
            None
        };
        let snapshot = WorldSnapshotResponse {
            schema: WORLD_SNAPSHOT_SCHEMA_V1.to_owned(),
            state: self.runtime_state(req.include_cells),
            scene_payload,
        };
        let payload_text = match serde_json::to_string_pretty(&snapshot) {
            Ok(value) => value,
            Err(e) => return RResult::RErr(RString::from(e.to_string())),
        };
        ok_json(&WorldSaveSnapshotResponse {
            ok: true,
            storage: "caller-owned".to_owned(),
            target_ref: req.target_ref,
            snapshot,
            payload_text,
        })
    }

    pub(crate) fn load_snapshot_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload).and_then(|v| {
            serde_json::from_value::<WorldLoadSnapshotRequest>(v).map_err(|e| e.to_string())
        }) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
        let snapshot = match req.snapshot {
            Some(snapshot) => snapshot,
            None => match req.payload {
                Some(value) => match serde_json::from_value::<WorldSnapshotResponse>(value) {
                    Ok(snapshot) => snapshot,
                    Err(e) => {
                        return RResult::RErr(RString::from(format!(
                            "world.load_snapshot_json_v1 payload decode failed: {e}"
                        )))
                    }
                },
                None => {
                    return RResult::RErr(RString::from(
                        "world.load_snapshot_json_v1 requires snapshot or payload",
                    ))
                }
            },
        };
        let restore = WorldRestoreSnapshotRequest {
            snapshot,
            replace_scene: req.replace_scene,
        };
        let restore_payload = match serde_json::to_vec(&restore) {
            Ok(bytes) => Blob::from(bytes),
            Err(e) => return RResult::RErr(RString::from(e.to_string())),
        };
        match self.restore_snapshot_json_v1(restore_payload).into_result() {
            Ok(bytes) => {
                let decoded =
                    serde_json::from_slice::<WorldRestoreSnapshotResponse>(&bytes.into_vec())
                        .map_err(|e| RString::from(e.to_string()));
                match decoded {
                    Ok(response) => ok_json(&WorldLoadSnapshotResponse {
                        ok: response.ok,
                        state: response.state,
                    }),
                    Err(e) => RResult::RErr(e),
                }
            }
            Err(e) => RResult::RErr(e),
        }
    }
}
