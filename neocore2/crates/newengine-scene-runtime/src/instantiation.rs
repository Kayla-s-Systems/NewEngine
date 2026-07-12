#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use serde_json::Value;

use crate::state::EngineSceneGatewayService;
use crate::transport::{json_result, parse_payload};
use crate::validation::validate_definition_ref_through_gateways;

fn deterministic_instance_guid(source: &str, instance_id: &str, salt: &str) -> u128 {
    // Stable FNV-1a style fold. This is a deterministic authoring seed, not a
    // cryptographic identifier.
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

fn transform_or_identity(value: Option<&Value>) -> Value {
    value.cloned().unwrap_or_else(|| {
        serde_json::json!({
            "position": [0.0, 0.0, 0.0],
            "rotation": [0.0, 0.0, 0.0, 1.0],
            "scale": [1.0, 1.0, 1.0],
        })
    })
}

struct InstanceArguments {
    name: String,
    guid: u128,
    transform: Value,
}

fn instance_arguments(
    request: &Value,
    source: &str,
    default_instance_id: &str,
    salt: &str,
) -> InstanceArguments {
    let instance_id = request
        .get("instance_id")
        .and_then(Value::as_str)
        .unwrap_or(default_instance_id);
    let name = request
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(instance_id)
        .to_owned();
    let guid = request
        .get("guid")
        .and_then(Value::as_u64)
        .map(u128::from)
        .unwrap_or_else(|| deterministic_instance_guid(source, instance_id, salt));
    InstanceArguments {
        name,
        guid,
        transform: transform_or_identity(request.get("transform")),
    }
}

impl EngineSceneGatewayService {
    pub(crate) fn instantiate_prefab_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        json_result(self.instantiate_prefab(&payload))
    }

    fn instantiate_prefab(&self, payload: &Blob) -> Result<Value, String> {
        let request = parse_payload(payload)?;
        let prefab_ref = request
            .get("prefab_ref")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "scene.instantiate_prefab_json_v1 requires prefab_ref".to_owned())?;
        let args = instance_arguments(&request, prefab_ref, "prefab_instance", "prefab");
        let definition_ref = request
            .get("definition_ref")
            .and_then(Value::as_str)
            .unwrap_or(prefab_ref);

        Ok(serde_json::json!({
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
                    "guid": args.guid,
                    "name": args.name,
                    "source_ref": prefab_ref,
                    "definition_ref": definition_ref,
                    "transform": args.transform,
                }
            ],
        }))
    }

    pub(crate) fn instantiate_archetype_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        json_result(self.instantiate_archetype(&payload))
    }

    fn instantiate_archetype(&self, payload: &Blob) -> Result<Value, String> {
        let request = parse_payload(payload)?;
        let definition_ref = request
            .get("definition_ref")
            .or_else(|| request.get("archetype_ref"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                "scene.instantiate_archetype_json_v1 requires definition_ref or archetype_ref"
                    .to_owned()
            })?;
        validate_definition_ref_through_gateways(definition_ref)?;
        let args = instance_arguments(&request, definition_ref, "archetype_instance", "archetype");

        Ok(serde_json::json!({
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
                    "guid": args.guid,
                    "name": args.name,
                    "source_ref": definition_ref,
                    "definition_ref": definition_ref,
                    "transform": args.transform,
                }
            ],
        }))
    }
}
