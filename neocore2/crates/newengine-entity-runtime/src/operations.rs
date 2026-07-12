use abi_stable::std_types::{RResult, RString};
use newengine_entity_api::{
    EntityDespawnRequest, EntityDespawnResponse, EntityDespawnResult, EntityExistsRequest,
    EntityExistsResponse, EntityInvokeRequest, EntityListRequest, EntityListResponse,
    EntitySpawnRequest, EntitySpawnResponse, ENTITY_SERVICE_METHOD_DESPAWN_JSON_V1,
    ENTITY_SERVICE_METHOD_EXISTS_JSON_V1, ENTITY_SERVICE_METHOD_LIST_JSON_V1,
    ENTITY_SERVICE_METHOD_SPAWN_JSON_V1,
};
use newengine_plugin_api::Blob;
use newengine_service_kit::ok_json;

use crate::{
    payload::{decode_payload, payload_from_value},
    service::{EngineEntityGatewayService, MAX_ENTITY_SPAWN_PER_CALL},
};

impl EngineEntityGatewayService {
    pub(crate) fn list_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let request = match decode_payload::<EntityListRequest>(&payload) {
            Ok(request) => request,
            Err(error) => return RResult::RErr(error),
        };

        let scene_lock = self.scene.scene();
        let scene = scene_lock.read();
        let world = scene.world();
        let mut entities = Vec::new();
        let mut truncated = false;

        for id in world.iter_entities() {
            if entities.len() >= request.limit {
                truncated = true;
                break;
            }
            entities.push(Self::live_record(Self::handle(id)));
        }

        ok_json(&EntityListResponse {
            entities,
            truncated,
            total_count: world.entity_count() as u64,
        })
    }

    pub(crate) fn exists_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let request = match decode_payload::<EntityExistsRequest>(&payload) {
            Ok(request) => request,
            Err(error) => return RResult::RErr(error),
        };

        let scene_lock = self.scene.scene();
        let scene = scene_lock.read();
        let world = scene.world();
        let exists = Self::find_entity_by_handle(world, request.entity).is_some();

        ok_json(EntityExistsResponse {
            entity: request.entity,
            exists,
        })
    }

    pub(crate) fn spawn_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let request = match decode_payload::<EntitySpawnRequest>(&payload) {
            Ok(request) => request,
            Err(error) => return RResult::RErr(error),
        };

        let count = request.count.min(MAX_ENTITY_SPAWN_PER_CALL);
        let scene_lock = self.scene.scene();
        let mut scene = scene_lock.write();
        let world = scene.world_mut();
        let mut entities = Vec::with_capacity(count);

        for _ in 0..count {
            let id = world.spawn();
            entities.push(Self::live_record(Self::handle(id)));
        }

        ok_json(&EntitySpawnResponse {
            entities,
            tick: world.tick(),
            total_count: world.entity_count() as u64,
        })
    }

    pub(crate) fn despawn_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let request = match decode_payload::<EntityDespawnRequest>(&payload) {
            Ok(request) => request,
            Err(error) => return RResult::RErr(error),
        };

        let scene_lock = self.scene.scene();
        let mut scene = scene_lock.write();
        let world = scene.world_mut();
        let mut results = Vec::with_capacity(request.entities.len());

        for entity in request.entities {
            let ok = Self::find_entity_by_handle(world, entity)
                .map(|id| world.despawn(id))
                .unwrap_or(false);
            results.push(EntityDespawnResult {
                entity,
                ok,
                message: if ok {
                    "entity despawned"
                } else {
                    "entity not found"
                }
                .to_owned(),
            });
        }

        let ok = results.iter().all(|result| result.ok);
        ok_json(&EntityDespawnResponse {
            ok,
            results,
            tick: world.tick(),
            total_count: world.entity_count() as u64,
        })
    }

    pub(crate) fn invoke_json(&self, payload: Blob) -> RResult<Blob, RString> {
        let request = match decode_payload::<EntityInvokeRequest>(&payload) {
            Ok(request) => request,
            Err(error) => return RResult::RErr(error),
        };
        let payload = match payload_from_value(&request.payload) {
            Ok(payload) => payload,
            Err(error) => return RResult::RErr(error),
        };

        match request.method.as_str() {
            ENTITY_SERVICE_METHOD_LIST_JSON_V1 => self.list_json_v1(payload),
            ENTITY_SERVICE_METHOD_EXISTS_JSON_V1 => self.exists_json_v1(payload),
            ENTITY_SERVICE_METHOD_SPAWN_JSON_V1 => self.spawn_json_v1(payload),
            ENTITY_SERVICE_METHOD_DESPAWN_JSON_V1 => self.despawn_json_v1(payload),
            other => RResult::RErr(RString::from(format!(
                "engine.entity invoke_json unknown target method '{other}'"
            ))),
        }
    }
}
