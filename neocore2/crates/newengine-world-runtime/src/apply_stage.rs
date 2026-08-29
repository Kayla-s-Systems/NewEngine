use std::collections::BTreeSet;

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_scene::{SceneAssetOptions, SceneEntityAsset, TransformAsset};
use newengine_service_kit::ok_json;
use newengine_world_api::{
    WorldApplyStageCommand, WorldApplyStageRequest, WorldApplyStageResponse,
};

use crate::{payload::decode_blob, service::EngineWorldGatewayService};

struct PreparedSpawn {
    command: WorldApplyStageCommand,
    transform: Option<TransformAsset>,
}

impl EngineWorldGatewayService {
    pub(crate) fn apply_stage(
        &self,
        request: WorldApplyStageRequest,
    ) -> Result<WorldApplyStageResponse, RString> {
        if request.stage != "scene.instantiate" {
            return Err(RString::from(format!(
                "world.apply_stage_json_v1 unsupported stage='{}'; mutation must be routed through an explicit apply stage",
                request.stage
            )));
        }

        let mut requested_guids = BTreeSet::new();
        let mut prepared = Vec::with_capacity(request.commands.len());
        for command in request.commands {
            if command.command != "scene.spawn_instance" {
                return Err(RString::from(format!(
                    "world.apply_stage_json_v1 unsupported command='{}' for stage='{}'",
                    command.command, request.stage
                )));
            }
            let guid = command
                .guid
                .ok_or_else(|| RString::from("scene.spawn_instance command requires guid"))?;
            if !requested_guids.insert(guid) {
                return Err(RString::from(format!(
                    "scene.spawn_instance duplicate guid={guid}"
                )));
            }
            let transform = command
                .transform
                .clone()
                .map(serde_json::from_value::<TransformAsset>)
                .transpose()
                .map_err(|error| {
                    RString::from(format!(
                        "scene.spawn_instance transform decode failed: {error}"
                    ))
                })?;
            prepared.push(PreparedSpawn { command, transform });
        }

        let mut undo_commands = Vec::with_capacity(prepared.len());
        {
            let scene_lock = self.scene.scene();
            let mut scene = scene_lock.write();
            let mut asset = scene.to_asset(SceneAssetOptions {
                include_empty_entities: true,
            });
            let existing_guids = asset
                .entities
                .iter()
                .map(|entity| entity.guid)
                .collect::<BTreeSet<_>>();

            if let Some(guid) = requested_guids
                .iter()
                .find(|guid| existing_guids.contains(guid))
            {
                return Err(RString::from(format!(
                    "scene.spawn_instance duplicate guid={guid}"
                )));
            }

            asset.entities.reserve(prepared.len());
            for prepared_spawn in prepared {
                let command = prepared_spawn.command;
                let guid = command.guid.expect("validated spawn guid");
                asset.entities.push(SceneEntityAsset {
                    guid,
                    name: command.name.clone(),
                    parent: command.parent,
                    transform: prepared_spawn.transform,
                    definition_ref: command.definition_ref.clone(),
                });

                let mut undo = command;
                undo.command = "scene.despawn_instance".to_owned();
                undo_commands.push(undo);
            }

            asset.entities.sort_unstable_by_key(|entity| entity.guid);
            scene.load_asset(&asset).map_err(|error| {
                RString::from(format!(
                    "world.apply_stage_json_v1 scene apply failed: {error}"
                ))
            })?;
        }

        let applied_count = undo_commands.len();
        {
            let mut state = self.state.lock();
            state.notes.push(format!(
                "apply-stage: stage={} transaction={} applied={} owner=engine.world",
                request.stage, request.transaction_id, applied_count
            ));
        }

        Ok(WorldApplyStageResponse {
            ok: true,
            stage: request.stage,
            transaction_id: request.transaction_id,
            applied_count,
            state: self.runtime_state(true),
            undo_commands,
        })
    }

    pub(crate) fn apply_stage_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let request = match decode_blob::<WorldApplyStageRequest>(&payload) {
            Ok(request) => request,
            Err(error) => return RResult::RErr(error),
        };
        match self.apply_stage(request) {
            Ok(response) => ok_json(response),
            Err(error) => RResult::RErr(error),
        }
    }
}
