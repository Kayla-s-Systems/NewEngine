#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted `engine.ecs` gateway runtime service.
//!
//! The ECS gateway lives outside game profile composition. It exposes stable DTOs
//! from `newengine-ecs-api` and operates through the shared runtime scene bridge.

use std::sync::Arc;

use abi_stable::std_types::{RResult, RString};
use newengine_ecs_api::{
    EcsCommand, EcsCommandRequest, EcsCommandResponse, EcsCommandResult, EcsInvokeRequest,
    EcsServiceInfo, EcsSnapshotRequest, EcsWorldSnapshot, EcsWorldSummary,
    ECS_BACKEND_CAPABILITY_ID, ENGINE_ECS_SERVICE_ID,
};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_gateway_provider_service, EngineGatewayProviderDecl, JsonServiceRouter,
};

pub const ECS_GATEWAY_OWNER: &str = "newengine-ecs-runtime.ecs-gateway";

#[derive(Clone)]
pub struct EngineEcsGatewayService {
    scene: Arc<newengine_scene_runtime::SceneBridge>,
}

impl EngineEcsGatewayService {
    #[inline]
    pub fn new(scene: Arc<newengine_scene_runtime::SceneBridge>) -> Self {
        Self { scene }
    }

    fn world_summary(world: &newengine_ecs::World) -> EcsWorldSummary {
        EcsWorldSummary {
            tick: world.tick(),
            entity_count: world.entity_count() as u64,
            storage_count: world.storage_count() as u64,
            resource_count: world.resource_count() as u64,
            entities_changed_tick: world.entities_changed_tick(),
        }
    }

    fn summary_json_v1(&self) -> RResult<Blob, RString> {
        let scene_lock = self.scene.scene();
        let scene = scene_lock.read();
        ok_json(&Self::world_summary(scene.world()))
    }

    fn snapshot_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload)
            .and_then(|v| serde_json::from_value::<EcsSnapshotRequest>(v).map_err(|e| e.to_string()))
        {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };

        let scene_lock = self.scene.scene();
        let scene = scene_lock.read();
        let world = scene.world();
        let summary = Self::world_summary(world);
        let mut entities = Vec::new();
        let mut truncated = false;

        if req.include_entities {
            for id in world.iter_entities() {
                if entities.len() >= req.entity_limit {
                    truncated = true;
                    break;
                }
                entities.push(newengine_ecs_api::EcsEntitySnapshot {
                    stable_id: id.stable_u64(),
                });
            }
        }

        ok_json(&EcsWorldSnapshot { summary, entities, truncated })
    }

    fn command_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload)
            .and_then(|v| serde_json::from_value::<EcsCommandRequest>(v).map_err(|e| e.to_string()))
        {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };

        let scene_lock = self.scene.scene();
        let mut scene = scene_lock.write();
        let world = scene.world_mut();
        let mut results = Vec::with_capacity(req.commands.len());

        for (index, command) in req.commands.into_iter().enumerate() {
            match command {
                EcsCommand::SetTick { tick } => {
                    world.set_tick(tick);
                    results.push(EcsCommandResult {
                        index,
                        ok: true,
                        entity_id: None,
                        tick: world.tick(),
                        message: "tick set".to_owned(),
                    });
                }
                EcsCommand::AdvanceTick => {
                    let tick = world.advance_tick();
                    results.push(EcsCommandResult {
                        index,
                        ok: true,
                        entity_id: None,
                        tick,
                        message: "tick advanced".to_owned(),
                    });
                }
                EcsCommand::SpawnEmpty => {
                    let id = world.spawn();
                    results.push(EcsCommandResult {
                        index,
                        ok: true,
                        entity_id: Some(id.stable_u64()),
                        tick: world.tick(),
                        message: "entity spawned".to_owned(),
                    });
                }
                EcsCommand::SetComponentJson { entity_id, component_type, .. } => {
                    results.push(EcsCommandResult {
                        index,
                        ok: false,
                        entity_id: Some(entity_id),
                        tick: world.tick(),
                        message: format!(
                            "semantic component packet '{}' requires a provider-backed component authority",
                            component_type
                        ),
                    });
                }
                EcsCommand::RemoveComponentJson { entity_id, component_type } => {
                    results.push(EcsCommandResult {
                        index,
                        ok: false,
                        entity_id: Some(entity_id),
                        tick: world.tick(),
                        message: format!(
                            "semantic component packet removal '{}' requires a provider-backed component authority",
                            component_type
                        ),
                    });
                }
            }
        }

        let summary = Self::world_summary(world);
        ok_json(&EcsCommandResponse { ok: true, summary, results })
    }

    fn invoke_json(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload)
            .and_then(|v| serde_json::from_value::<EcsInvokeRequest>(v).map_err(|e| e.to_string()))
        {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
        let payload = match serde_json::to_vec(&req.payload) {
            Ok(bytes) => Blob::from(bytes),
            Err(e) => return RResult::RErr(RString::from(e.to_string())),
        };

        match req.method.as_str() {
            newengine_ecs_api::ECS_SERVICE_METHOD_SUMMARY_JSON_V1 => self.summary_json_v1(),
            newengine_ecs_api::ECS_SERVICE_METHOD_SNAPSHOT_JSON_V1 => self.snapshot_json_v1(payload),
            newengine_ecs_api::ECS_SERVICE_METHOD_COMMAND_JSON_V1 => self.command_json_v1(payload),
            other => RResult::RErr(RString::from(format!(
                "engine.ecs invoke_json unknown target method '{other}'"
            ))),
        }
    }
}

pub fn ecs_gateway_service(
    scene: Arc<newengine_scene_runtime::SceneBridge>,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let service = EngineEcsGatewayService::new(scene);
    let info = EcsServiceInfo::default();
    let description = engine_gateway_provider_service_description(
        ENGINE_ECS_SERVICE_ID,
        ECS_GATEWAY_OWNER,
        ECS_BACKEND_CAPABILITY_ID,
        info.methods.clone(),
    )
    .protocol(info.protocol.clone())
    .features(info.features.clone());

    let summary_service = service.clone();
    let snapshot_service = service.clone();
    let command_service = service.clone();
    let invoke_service = service;

    JsonServiceRouter::new(ENGINE_ECS_SERVICE_ID)
        .describe_json(&description)
        .info(EcsServiceInfo::default)
        .blob(newengine_ecs_api::ECS_SERVICE_METHOD_INVOKE, move |_unit, payload| invoke_service.invoke_json(payload))
        .blob(newengine_ecs_api::ECS_SERVICE_METHOD_SUMMARY_JSON_V1, move |_unit, _payload| summary_service.summary_json_v1())
        .blob(newengine_ecs_api::ECS_SERVICE_METHOD_SNAPSHOT_JSON_V1, move |_unit, payload| snapshot_service.snapshot_json_v1(payload))
        .blob(newengine_ecs_api::ECS_SERVICE_METHOD_COMMAND_JSON_V1, move |_unit, payload| command_service.command_json_v1(payload))
        .blob(newengine_ecs_api::ECS_SERVICE_METHOD_SHUTDOWN_V1, |_unit, _payload| ok_empty_blob())
        .into_service_v1()
}

pub fn register_ecs_gateway_best_effort(scene: Arc<newengine_scene_runtime::SceneBridge>) {
    if newengine_plugin_host::has_service(ENGINE_ECS_SERVICE_ID) {
        log::debug!("engine.ecs gateway registration skipped; service already available");
        return;
    }

    let service = ecs_gateway_service(scene);
    match register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: ENGINE_ECS_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Ecs,
        provider_service: ENGINE_ECS_SERVICE_ID,
        provider_route: "engine.ecs.foundation",
        capability: ECS_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: ECS_GATEWAY_OWNER,
        service,
    }) {
        Ok(()) => log::info!(
            "engine.ecs gateway registered source=engine-runtime service='{}' capability='{}' owner='{}'",
            ENGINE_ECS_SERVICE_ID,
            ECS_BACKEND_CAPABILITY_ID,
            ECS_GATEWAY_OWNER
        ),
        Err(e) => log::error!(
            "engine.ecs gateway registration failed id='{}' err='{}'",
            ENGINE_ECS_SERVICE_ID,
            e
        ),
    }
}
