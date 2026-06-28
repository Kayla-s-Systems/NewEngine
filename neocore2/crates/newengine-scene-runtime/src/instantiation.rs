#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_service_kit::{ok_json, payload_json};

use crate::{validate_definition_ref_through_gateways, EngineSceneGatewayService};

fn deterministic_instance_guid(source: &str, instance_id: &str, salt: &str) -> u128 {
    // Stable FNV-1a style fold. This is not a cryptographic id; it is an authoring/runtime
    // deterministic GUID seed so repeated plans produce identical command DTOs.
    let mut hi: u64 = 0xcbf29ce484222325;
    let mut lo: u64 = 0x84222325cbf29ce4;
    for byte in source
        .bytes()
        .chain([0xff])
        .chain(instance_id.bytes())
        .chain([0xfe])
        .chain(salt.bytes())
    {
        hi ^= byte as u64;
        hi = hi.wrapping_mul(0x100000001b3);
        lo ^= (byte as u64).rotate_left(7);
        lo = lo.wrapping_mul(0x100000001b3);
    }
    ((hi as u128) << 64) | lo as u128
}

fn transform_or_identity(value: Option<&serde_json::Value>) -> serde_json::Value {
    value.cloned().unwrap_or_else(|| {
        serde_json::json!({
            "position": [0.0, 0.0, 0.0],
            "rotation": [0.0, 0.0, 0.0, 1.0],
            "scale": [1.0, 1.0, 1.0]
        })
    })
}

impl EngineSceneGatewayService {
    pub(crate) fn instantiate_prefab_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
        let prefab_ref = req
            .get("prefab_ref")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|it| !it.is_empty());
        let Some(prefab_ref) = prefab_ref else {
            return RResult::RErr(RString::from(
                "scene.instantiate_prefab_json_v1 requires prefab_ref",
            ));
        };
        let instance_id = req
            .get("instance_id")
            .and_then(|v| v.as_str())
            .unwrap_or("prefab_instance");
        let name = req
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(instance_id);
        let guid = req
            .get("guid")
            .and_then(|v| v.as_u64())
            .map(u128::from)
            .unwrap_or_else(|| deterministic_instance_guid(prefab_ref, instance_id, "prefab"));
        let transform = transform_or_identity(req.get("transform"));
        ok_json(serde_json::json!({
            "schema": "newengine.scene.instantiation_plan.v1",
            "mode": "prefab",
            "mutates_scene": false,
            "apply_gateway": newengine_world_api::ENGINE_WORLD_SERVICE_ID,
            "apply_method": newengine_world_api::WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1,
            "stage": "scene.instantiate",
            "prefab_ref": prefab_ref,
            "commands": [
                {
                    "command": "scene.spawn_instance",
                    "guid": guid,
                    "name": name,
                    "source_ref": prefab_ref,
                    "definition_ref": req.get("definition_ref").and_then(|v| v.as_str()).unwrap_or(prefab_ref),
                    "transform": transform
                }
            ]
        }))
    }

    pub(crate) fn instantiate_archetype_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
        let definition_ref = req
            .get("definition_ref")
            .or_else(|| req.get("archetype_ref"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|it| !it.is_empty());
        let Some(definition_ref) = definition_ref else {
            return RResult::RErr(RString::from(
                "scene.instantiate_archetype_json_v1 requires definition_ref or archetype_ref",
            ));
        };
        if let Err(e) = validate_definition_ref_through_gateways(definition_ref) {
            return RResult::RErr(RString::from(e));
        }
        let instance_id = req
            .get("instance_id")
            .and_then(|v| v.as_str())
            .unwrap_or("archetype_instance");
        let name = req
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(instance_id);
        let guid = req
            .get("guid")
            .and_then(|v| v.as_u64())
            .map(u128::from)
            .unwrap_or_else(|| {
                deterministic_instance_guid(definition_ref, instance_id, "archetype")
            });
        let transform = transform_or_identity(req.get("transform"));
        ok_json(serde_json::json!({
            "schema": "newengine.scene.instantiation_plan.v1",
            "mode": "archetype",
            "mutates_scene": false,
            "apply_gateway": newengine_world_api::ENGINE_WORLD_SERVICE_ID,
            "apply_method": newengine_world_api::WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1,
            "stage": "scene.instantiate",
            "archetype_ref": definition_ref,
            "commands": [
                {
                    "command": "scene.spawn_instance",
                    "guid": guid,
                    "name": name,
                    "source_ref": definition_ref,
                    "definition_ref": definition_ref,
                    "transform": transform
                }
            ]
        }))
    }
}
