use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_scene::{SCENE_ASSET_SCHEMA_V1, SCENE_ASSET_STATUS_TRANSITIONAL_JSON};
use newengine_scene_io::ENGINE_SCENE_SERVICE_ID;

use crate::constants::{SCENE_FORMAT_METHODS, SCENE_GATEWAY_OWNER};
use crate::state::EngineSceneGatewayService;
use crate::transport::json_result;

impl EngineSceneGatewayService {
    pub(crate) fn formats_json(&self) -> RResult<Blob, RString> {
        json_result(Ok(serde_json::json!({
            "id": ENGINE_SCENE_SERVICE_ID,
            "origin": "engine-runtime",
            "owner": SCENE_GATEWAY_OWNER,
            "version": 1,
            "semantics": {
                "scene": "authored structure",
                "world": "living runtime world handled by engine.world",
                "ecs": "storage only behind engine.ecs coarse commands/snapshots",
            },
            "formats": [
                {
                    "id": "newengine.scene.asset.v1",
                    "schema": SCENE_ASSET_SCHEMA_V1,
                    "status": SCENE_ASSET_STATUS_TRANSITIONAL_JSON,
                    "media_type": "application/json",
                    "load": true,
                    "save": true,
                    "scene_graph": true,
                    "archetype_graph": true,
                    "placement_declarations": true,
                    "prefab_archetype_instances": true,
                    "not_world_runtime_snapshot": true,
                    "not_ytyp": true,
                    "not_definition_dictionary": true,
                    "allowed_definition_ref_field": "entities[].definition_ref",
                    "definition_resolution": [
                        newengine_assets_api::ENGINE_ASSETS_GRAPH_SERVICE_ID,
                        newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                    ],
                }
            ],
            "authority_topology": self.authority_json(),
            "methods": SCENE_FORMAT_METHODS,
        })))
    }

    pub(crate) fn graph_json_v1(&self) -> RResult<Blob, RString> {
        let asset = self.current_scene_asset(true);
        let nodes = asset
            .entities
            .iter()
            .map(|entity| {
                serde_json::json!({
                    "guid": entity.guid.to_string(),
                    "name": entity.name.clone(),
                    "parent": entity.parent.map(|value| value.to_string()),
                    "has_transform": entity.transform.is_some(),
                    "definition_ref": entity.definition_ref.clone(),
                })
            })
            .collect::<Vec<_>>();
        json_result(Ok(serde_json::json!({
            "schema": asset.schema,
            "semantics": "authored_scene_graph",
            "world_runtime": "engine.world",
            "root": asset.root.map(|value| value.to_string()),
            "active_camera": asset.active_camera.map(|value| value.to_string()),
            "nodes": nodes,
            "node_count": asset.entities.len(),
        })))
    }

    pub(crate) fn archetype_graph_json_v1(&self) -> RResult<Blob, RString> {
        let asset = self.current_scene_asset(true);
        let mut references = asset
            .entities
            .iter()
            .filter_map(|entity| {
                entity
                    .definition_ref
                    .as_ref()
                    .map(|definition_ref| (entity.guid, definition_ref.clone()))
            })
            .collect::<Vec<_>>();
        references.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        let entries = references
            .into_iter()
            .map(|(guid, definition_ref)| {
                serde_json::json!({
                    "instance_guid": guid.to_string(),
                    "definition_ref": definition_ref,
                    "definition_domain": newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                    "dependency_graph_domain": newengine_assets_api::ENGINE_ASSETS_GRAPH_SERVICE_ID,
                })
            })
            .collect::<Vec<_>>();
        json_result(Ok(serde_json::json!({
            "semantics": "authored_archetype_graph",
            "not_runtime_spawn_state": true,
            "entries": entries,
        })))
    }

    pub(crate) fn placements_json_v1(&self) -> RResult<Blob, RString> {
        let asset = self.current_scene_asset(true);
        let placements = asset
            .entities
            .iter()
            .filter_map(|entity| {
                entity.transform.map(|transform| {
                    serde_json::json!({
                        "guid": entity.guid.to_string(),
                        "name": entity.name.clone(),
                        "parent": entity.parent.map(|value| value.to_string()),
                        "definition_ref": entity.definition_ref.clone(),
                        "transform": transform,
                    })
                })
            })
            .collect::<Vec<_>>();
        json_result(Ok(serde_json::json!({
            "semantics": "authored_placement_declarations",
            "world_runtime": "engine.world",
            "placements": placements,
        })))
    }
}
