#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted `engine.entity` gateway runtime service.
//!
//! Entity lifecycle calls are exposed through stable DTOs from
//! `newengine-entity-api` and operate against the shared runtime scene bridge.

use std::sync::Arc;

use abi_stable::std_types::{RResult, RString};
use newengine_entity_api::{
    EntityDespawnRequest, EntityDespawnResponse, EntityDespawnResult, EntityExistsRequest,
    EntityExistsResponse, EntityHandle, EntityInvokeRequest, EntityListRequest, EntityListResponse,
    EntityRecord, EntityServiceInfo, EntitySpawnRequest, EntitySpawnResponse,
    ENGINE_ENTITY_SERVICE_ID, ENTITY_BACKEND_CAPABILITY_ID,
};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_gateway_provider_service, EngineGatewayProviderDecl, JsonServiceRouter,
};

pub const ENTITY_GATEWAY_OWNER: &str = "newengine-entity-runtime.entity-gateway";
const MAX_ENTITY_SPAWN_PER_CALL: usize = 4096;

#[derive(Clone)]
pub struct EngineEntityGatewayService {
    scene: Arc<newengine_scene_runtime::SceneBridge>,
}

impl EngineEntityGatewayService {
    #[inline]
    pub fn new(scene: Arc<newengine_scene_runtime::SceneBridge>) -> Self {
        Self { scene }
    }

    #[inline]
    fn handle(id: newengine_ecs::EntityId) -> EntityHandle {
        EntityHandle::new(id.stable_u64())
    }

    fn live_record(handle: EntityHandle) -> EntityRecord {
        EntityRecord {
            handle,
            lifecycle: "".to_string(),
            tags: vec![],
            owner: None,
            debug_identity: "".to_string(),
        }
    }

    fn find_entity_by_handle(
        world: &newengine_ecs::World,
        handle: EntityHandle,
    ) -> Option<newengine_ecs::EntityId> {
        world
            .iter_entities()
            .find(|id| id.stable_u64() == handle.stable_id)
    }

    fn list_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload)
            .and_then(|v| serde_json::from_value::<EntityListRequest>(v).map_err(|e| e.to_string()))
        {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };

        let scene_lock = self.scene.scene();
        let scene = scene_lock.read();
        let world = scene.world();
        let mut entities = Vec::new();
        let mut truncated = false;

        for id in world.iter_entities() {
            if entities.len() >= req.limit {
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

    fn exists_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload).and_then(|v| {
            serde_json::from_value::<EntityExistsRequest>(v).map_err(|e| e.to_string())
        }) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };

        let scene_lock = self.scene.scene();
        let scene = scene_lock.read();
        let world = scene.world();
        let exists = Self::find_entity_by_handle(world, req.entity).is_some();

        ok_json(&EntityExistsResponse {
            entity: req.entity,
            exists,
        })
    }

    fn spawn_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload).and_then(|v| {
            serde_json::from_value::<EntitySpawnRequest>(v).map_err(|e| e.to_string())
        }) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };

        let count = req.count.min(MAX_ENTITY_SPAWN_PER_CALL);
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

    fn despawn_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload).and_then(|v| {
            serde_json::from_value::<EntityDespawnRequest>(v).map_err(|e| e.to_string())
        }) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };

        let scene_lock = self.scene.scene();
        let mut scene = scene_lock.write();
        let world = scene.world_mut();
        let mut results = Vec::with_capacity(req.entities.len());

        for entity in req.entities {
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

    fn invoke_json(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload).and_then(|v| {
            serde_json::from_value::<EntityInvokeRequest>(v).map_err(|e| e.to_string())
        }) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
        let payload = match serde_json::to_vec(&req.payload) {
            Ok(bytes) => Blob::from(bytes),
            Err(e) => return RResult::RErr(RString::from(e.to_string())),
        };

        match req.method.as_str() {
            newengine_entity_api::ENTITY_SERVICE_METHOD_LIST_JSON_V1 => self.list_json_v1(payload),
            newengine_entity_api::ENTITY_SERVICE_METHOD_EXISTS_JSON_V1 => {
                self.exists_json_v1(payload)
            }
            newengine_entity_api::ENTITY_SERVICE_METHOD_SPAWN_JSON_V1 => {
                self.spawn_json_v1(payload)
            }
            newengine_entity_api::ENTITY_SERVICE_METHOD_DESPAWN_JSON_V1 => {
                self.despawn_json_v1(payload)
            }
            other => RResult::RErr(RString::from(format!(
                "engine.entity invoke_json unknown target method '{other}'"
            ))),
        }
    }
}

pub fn entity_gateway_service(
    scene: Arc<newengine_scene_runtime::SceneBridge>,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let service = EngineEntityGatewayService::new(scene);
    let info = EntityServiceInfo::default();
    let description = engine_gateway_provider_service_description(
        ENGINE_ENTITY_SERVICE_ID,
        ENTITY_GATEWAY_OWNER,
        ENTITY_BACKEND_CAPABILITY_ID,
        info.methods.clone(),
    )
    .protocol(info.protocol.clone())
    .features(info.features.clone());

    let invoke_service = service.clone();
    let list_service = service.clone();
    let exists_service = service.clone();
    let spawn_service = service.clone();
    let despawn_service = service;

    JsonServiceRouter::new(ENGINE_ENTITY_SERVICE_ID)
        .describe_json(&description)
        .info(EntityServiceInfo::default)
        .blob(
            newengine_entity_api::ENTITY_SERVICE_METHOD_INVOKE,
            move |_unit, payload| invoke_service.invoke_json(payload),
        )
        .blob(
            newengine_entity_api::ENTITY_SERVICE_METHOD_LIST_JSON_V1,
            move |_unit, payload| list_service.list_json_v1(payload),
        )
        .blob(
            newengine_entity_api::ENTITY_SERVICE_METHOD_EXISTS_JSON_V1,
            move |_unit, payload| exists_service.exists_json_v1(payload),
        )
        .blob(
            newengine_entity_api::ENTITY_SERVICE_METHOD_SPAWN_JSON_V1,
            move |_unit, payload| spawn_service.spawn_json_v1(payload),
        )
        .blob(
            newengine_entity_api::ENTITY_SERVICE_METHOD_DESPAWN_JSON_V1,
            move |_unit, payload| despawn_service.despawn_json_v1(payload),
        )
        .blob(
            newengine_entity_api::ENTITY_SERVICE_METHOD_SHUTDOWN_V1,
            |_unit, _payload| ok_empty_blob(),
        )
        .into_service_v1()
}

pub fn register_entity_gateway_best_effort(scene: Arc<newengine_scene_runtime::SceneBridge>) {
    if newengine_plugin_host::has_service(ENGINE_ENTITY_SERVICE_ID) {
        newengine_ulog_api::ulog::debug!(
            "engine.entity gateway registration skipped; service already available"
        );
        return;
    }

    let service = entity_gateway_service(scene);
    match register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: ENGINE_ENTITY_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Entity,
        provider_service: ENGINE_ENTITY_SERVICE_ID,
        provider_route: "engine.entity.foundation",
        capability: ENTITY_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: ENTITY_GATEWAY_OWNER,
        service,
    }) {
        Ok(()) => newengine_ulog_api::ulog::info!(
            "engine.entity gateway registered source=engine-runtime service='{}' capability='{}' owner='{}'",
            ENGINE_ENTITY_SERVICE_ID,
            ENTITY_BACKEND_CAPABILITY_ID,
            ENTITY_GATEWAY_OWNER
        ),
        Err(e) => newengine_ulog_api::ulog::error!(
            "engine.entity gateway registration failed id='{}' err='{}'",
            ENGINE_ENTITY_SERVICE_ID,
            e
        ),
    }
}
