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
        ok_json(Self::world_summary(scene.world()))
    }

    fn snapshot_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload).and_then(|v| {
            serde_json::from_value::<EcsSnapshotRequest>(v).map_err(|e| e.to_string())
        }) {
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
                    handle: newengine_entity_api::EntityHandle::new(id.stable_u64()),
                });
            }
        }

        ok_json(&EcsWorldSnapshot {
            summary,
            entities,
            truncated,
        })
    }

    fn resolve_entity_by_stable_id(
        world: &newengine_ecs::World,
        stable_id: u64,
    ) -> Option<newengine_ecs::EntityId> {
        world
            .iter_entities()
            .find(|entity| entity.stable_u64() == stable_id)
    }

    fn set_semantic_component(
        world: &mut newengine_ecs::World,
        entity_id: u64,
        component_type: &str,
        payload: serde_json::Value,
    ) -> (bool, String) {
        let Some(entity) = Self::resolve_entity_by_stable_id(world, entity_id) else {
            return (false, "entity not found".to_owned());
        };
        match component_type.trim() {
            newengine_audio_api::AUDIO_EMITTER_COMPONENT_TYPE => {
                let emitter =
                    match serde_json::from_value::<newengine_audio_api::AudioEmitter>(payload) {
                        Ok(emitter) => emitter,
                        Err(error) => {
                            return (
                                false,
                                format!("audio.emitter payload decode failed: {error}"),
                            );
                        }
                    };
                let _ = world.insert(entity, emitter);
                (true, "audio.emitter set".to_owned())
            }
            newengine_audio_api::AUDIO_ACOUSTIC_SURFACE_COMPONENT_TYPE => {
                let surface =
                    match serde_json::from_value::<newengine_audio_api::AcousticSurface>(payload) {
                        Ok(surface) => surface.sanitized(),
                        Err(error) => {
                            return (
                                false,
                                format!("audio.acoustic_surface payload decode failed: {}", error),
                            );
                        }
                    };
                let _ = world.insert(entity, surface);
                (true, "audio.acoustic_surface set".to_owned())
            }
            newengine_audio_api::AUDIO_ENVIRONMENT_ZONE_COMPONENT_TYPE => {
                let zone = match serde_json::from_value::<newengine_audio_api::AudioEnvironmentZone>(
                    payload,
                ) {
                    Ok(zone) => zone.sanitized(),
                    Err(error) => {
                        return (
                            false,
                            format!("audio.environment_zone payload decode failed: {error}"),
                        );
                    }
                };
                let _ = world.insert(entity, zone);
                (true, "audio.environment_zone set".to_owned())
            }
            newengine_audio_api::AUDIO_PORTAL_COMPONENT_TYPE => {
                let portal =
                    match serde_json::from_value::<newengine_audio_api::AudioPortal>(payload) {
                        Ok(portal) => portal.sanitized(),
                        Err(error) => {
                            return (
                                false,
                                format!("audio.portal payload decode failed: {error}"),
                            );
                        }
                    };
                let _ = world.insert(entity, portal);
                (true, "audio.portal set".to_owned())
            }
            newengine_audio_api::AUDIO_AMBIENCE_BED_COMPONENT_TYPE => {
                let bed = match serde_json::from_value::<newengine_audio_api::AudioAmbienceBed>(
                    payload,
                ) {
                    Ok(bed) => bed.sanitized(),
                    Err(error) => {
                        return (
                            false,
                            format!("audio.ambience_bed payload decode failed: {error}"),
                        );
                    }
                };
                let _ = world.insert(entity, bed);
                (true, "audio.ambience_bed set".to_owned())
            }
            other => (
                false,
                format!(
                    "semantic component packet '{}' requires a provider-backed component authority",
                    other
                ),
            ),
        }
    }

    fn remove_semantic_component(
        world: &mut newengine_ecs::World,
        entity_id: u64,
        component_type: &str,
    ) -> (bool, String) {
        let Some(entity) = Self::resolve_entity_by_stable_id(world, entity_id) else {
            return (false, "entity not found".to_owned());
        };
        match component_type.trim() {
            newengine_audio_api::AUDIO_EMITTER_COMPONENT_TYPE => {
                let existed = world.remove::<newengine_audio_api::AudioEmitter>(entity).is_some();
                if existed {
                    (true, "audio.emitter removed".to_owned())
                } else {
                    (false, "audio.emitter was not attached".to_owned())
                }
            }
            newengine_audio_api::AUDIO_ACOUSTIC_SURFACE_COMPONENT_TYPE => {
                let existed = world
                    .remove::<newengine_audio_api::AcousticSurface>(entity)
                    .is_some();
                if existed {
                    (true, "audio.acoustic_surface removed".to_owned())
                } else {
                    (false, "audio.acoustic_surface was not attached".to_owned())
                }
            }
            newengine_audio_api::AUDIO_ENVIRONMENT_ZONE_COMPONENT_TYPE => {
                let existed = world
                    .remove::<newengine_audio_api::AudioEnvironmentZone>(entity)
                    .is_some();
                if existed {
                    (true, "audio.environment_zone removed".to_owned())
                } else {
                    (false, "audio.environment_zone was not attached".to_owned())
                }
            }
            newengine_audio_api::AUDIO_PORTAL_COMPONENT_TYPE => {
                let existed = world
                    .remove::<newengine_audio_api::AudioPortal>(entity)
                    .is_some();
                if existed {
                    (true, "audio.portal removed".to_owned())
                } else {
                    (false, "audio.portal was not attached".to_owned())
                }
            }
            newengine_audio_api::AUDIO_AMBIENCE_BED_COMPONENT_TYPE => {
                let existed = world
                    .remove::<newengine_audio_api::AudioAmbienceBed>(entity)
                    .is_some();
                if existed {
                    (true, "audio.ambience_bed removed".to_owned())
                } else {
                    (false, "audio.ambience_bed was not attached".to_owned())
                }
            }
            other => (
                false,
                format!(
                    "semantic component packet removal '{}' requires a provider-backed component authority",
                    other
                ),
            ),
        }
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
                EcsCommand::SetComponentJson {
                    entity_id,
                    component_type,
                    payload,
                } => {
                    let (ok, message) =
                        Self::set_semantic_component(world, entity_id, &component_type, payload);
                    results.push(EcsCommandResult {
                        index,
                        ok,
                        entity_id: Some(entity_id),
                        tick: world.tick(),
                        message,
                    });
                }
                EcsCommand::RemoveComponentJson {
                    entity_id,
                    component_type,
                } => {
                    let (ok, message) =
                        Self::remove_semantic_component(world, entity_id, &component_type);
                    results.push(EcsCommandResult {
                        index,
                        ok,
                        entity_id: Some(entity_id),
                        tick: world.tick(),
                        message,
                    });
                }
            }
        }

        let summary = Self::world_summary(world);
        ok_json(&EcsCommandResponse {
            ok: true,
            summary,
            results,
        })
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
            newengine_ecs_api::ECS_SERVICE_METHOD_SNAPSHOT_JSON_V1 => {
                self.snapshot_json_v1(payload)
            }
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
        .blob(
            newengine_ecs_api::ECS_SERVICE_METHOD_INVOKE,
            move |_unit, payload| invoke_service.invoke_json(payload),
        )
        .blob(
            newengine_ecs_api::ECS_SERVICE_METHOD_SUMMARY_JSON_V1,
            move |_unit, _payload| summary_service.summary_json_v1(),
        )
        .blob(
            newengine_ecs_api::ECS_SERVICE_METHOD_SNAPSHOT_JSON_V1,
            move |_unit, payload| snapshot_service.snapshot_json_v1(payload),
        )
        .blob(
            newengine_ecs_api::ECS_SERVICE_METHOD_COMMAND_JSON_V1,
            move |_unit, payload| command_service.command_json_v1(payload),
        )
        .blob(
            newengine_ecs_api::ECS_SERVICE_METHOD_SHUTDOWN_V1,
            |_unit, _payload| ok_empty_blob(),
        )
        .into_service_v1()
}

pub fn register_ecs_gateway_best_effort(scene: Arc<newengine_scene_runtime::SceneBridge>) {
    if newengine_plugin_host::has_service(ENGINE_ECS_SERVICE_ID) {
        newengine_ulog_api::ulog::debug!(
            "engine.ecs gateway registration skipped; service already available"
        );
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
        Ok(()) => newengine_ulog_api::ulog::info!(
            "engine.ecs gateway registered source=engine-runtime service='{}' capability='{}' owner='{}'",
            ENGINE_ECS_SERVICE_ID,
            ECS_BACKEND_CAPABILITY_ID,
            ECS_GATEWAY_OWNER
        ),
        Err(e) => newengine_ulog_api::ulog::error!(
            "engine.ecs gateway registration failed id='{}' err='{}'",
            ENGINE_ECS_SERVICE_ID,
            e
        ),
    }
}

#[cfg(test)]
mod semantic_component_tests {
    use super::*;

    #[test]
    fn semantic_audio_emitter_can_be_authored_through_engine_ecs() {
        let mut world = newengine_ecs::World::new();
        let entity = world.spawn();
        let payload = serde_json::json!({
            "cue": "shared/audio/weapon/rifle/rifle.yscd@fire",
            "enabled": true,
            "autoplay": false,
            "gain": 0.8,
            "spatial": true
        });
        let (ok, message) = EngineEcsGatewayService::set_semantic_component(
            &mut world,
            entity.stable_u64(),
            newengine_audio_api::AUDIO_EMITTER_COMPONENT_TYPE,
            payload,
        );
        assert!(ok, "{message}");
        let emitter = world
            .get::<newengine_audio_api::AudioEmitter>(entity)
            .expect("audio emitter component");
        assert_eq!(emitter.cue, "shared/audio/weapon/rifle/rifle.yscd@fire");
        assert_eq!(emitter.gain, 0.8);

        let (ok, message) = EngineEcsGatewayService::remove_semantic_component(
            &mut world,
            entity.stable_u64(),
            newengine_audio_api::AUDIO_EMITTER_COMPONENT_TYPE,
        );
        assert!(ok, "{message}");
        assert!(world
            .get::<newengine_audio_api::AudioEmitter>(entity)
            .is_none());
    }

    #[test]
    fn semantic_acoustic_surface_can_be_authored_through_engine_ecs() {
        let mut world = newengine_ecs::World::new();
        let entity = world.spawn();
        let payload = serde_json::json!({
            "material_id": "material.glass.thin",
            "profile": {
                "transmission_gain": 0.52,
                "high_frequency_absorption": 0.48,
                "low_pass_hz": 6200.0
            }
        });
        let (ok, message) = EngineEcsGatewayService::set_semantic_component(
            &mut world,
            entity.stable_u64(),
            newengine_audio_api::AUDIO_ACOUSTIC_SURFACE_COMPONENT_TYPE,
            payload,
        );
        assert!(ok, "{message}");
        let surface = world
            .get::<newengine_audio_api::AcousticSurface>(entity)
            .expect("acoustic surface component");
        assert_eq!(surface.material_id, "material.glass.thin");
        assert_eq!(surface.profile.low_pass_hz, 6200.0);

        let (ok, message) = EngineEcsGatewayService::remove_semantic_component(
            &mut world,
            entity.stable_u64(),
            newengine_audio_api::AUDIO_ACOUSTIC_SURFACE_COMPONENT_TYPE,
        );
        assert!(ok, "{message}");
        assert!(world
            .get::<newengine_audio_api::AcousticSurface>(entity)
            .is_none());
    }

    #[test]
    fn semantic_environment_zone_can_be_authored_through_engine_ecs() {
        let mut world = newengine_ecs::World::new();
        let entity = world.spawn();
        let payload = serde_json::json!({
            "zone_id": "room.hall",
            "half_extents": [12.0, 4.0, 20.0],
            "priority": 3,
            "send_gain": 0.55,
            "reverb": {
                "decay_seconds": 2.8,
                "damping": 0.32,
                "diffusion": 0.82
            }
        });
        let (ok, message) = EngineEcsGatewayService::set_semantic_component(
            &mut world,
            entity.stable_u64(),
            newengine_audio_api::AUDIO_ENVIRONMENT_ZONE_COMPONENT_TYPE,
            payload,
        );
        assert!(ok, "{message}");
        let zone = world
            .get::<newengine_audio_api::AudioEnvironmentZone>(entity)
            .expect("environment zone");
        assert_eq!(zone.zone_id, "room.hall");
        assert_eq!(zone.priority, 3);

        let (ok, message) = EngineEcsGatewayService::remove_semantic_component(
            &mut world,
            entity.stable_u64(),
            newengine_audio_api::AUDIO_ENVIRONMENT_ZONE_COMPONENT_TYPE,
        );
        assert!(ok, "{message}");
        assert!(world
            .get::<newengine_audio_api::AudioEnvironmentZone>(entity)
            .is_none());
    }

    #[test]
    fn semantic_audio_portal_can_be_authored_through_engine_ecs() {
        let mut world = newengine_ecs::World::new();
        let entity = world.spawn();
        let payload = serde_json::json!({
            "portal_id": "door.main",
            "zone_a": "room.hall",
            "zone_b": "room.corridor",
            "openness": 0.4,
            "transmission_gain": 0.75,
            "send_gain": 0.8
        });
        let (ok, message) = EngineEcsGatewayService::set_semantic_component(
            &mut world,
            entity.stable_u64(),
            newengine_audio_api::AUDIO_PORTAL_COMPONENT_TYPE,
            payload,
        );
        assert!(ok, "{message}");
        let portal = world
            .get::<newengine_audio_api::AudioPortal>(entity)
            .expect("audio portal");
        assert_eq!(portal.zone_a, "room.hall");
        assert!((portal.route_gain() - 0.24).abs() < 1.0e-6);
    }

    #[test]
    fn semantic_audio_ambience_bed_can_be_authored_through_engine_ecs() {
        let mut world = newengine_ecs::World::new();
        let entity = world.spawn();
        let payload = serde_json::json!({
            "bed_id": "ambience.city.rain",
            "stream": { "uri": "shared/audio/ambience/rain.ogg" },
            "scope": "zones",
            "zones": ["room.lobby", "room.corridor"],
            "gain": 0.65,
            "fade_seconds": 2.0,
            "portal_bleed": 0.25,
            "spatial": false,
            "looping": true
        });
        let (ok, message) = EngineEcsGatewayService::set_semantic_component(
            &mut world,
            entity.stable_u64(),
            newengine_audio_api::AUDIO_AMBIENCE_BED_COMPONENT_TYPE,
            payload,
        );
        assert!(ok, "{message}");
        let bed = world
            .get::<newengine_audio_api::AudioAmbienceBed>(entity)
            .expect("ambience bed");
        assert_eq!(bed.bed_id, "ambience.city.rain");
        assert_eq!(bed.zones, vec!["room.corridor", "room.lobby"]);
        assert_eq!(bed.stream.uri, "shared/audio/ambience/rain.ogg");

        let (ok, message) = EngineEcsGatewayService::remove_semantic_component(
            &mut world,
            entity.stable_u64(),
            newengine_audio_api::AUDIO_AMBIENCE_BED_COMPONENT_TYPE,
        );
        assert!(ok, "{message}");
        assert!(world
            .get::<newengine_audio_api::AudioAmbienceBed>(entity)
            .is_none());
    }

    #[test]
    fn unknown_semantic_component_stays_rejected() {
        let mut world = newengine_ecs::World::new();
        let entity = world.spawn();
        let (ok, message) = EngineEcsGatewayService::set_semantic_component(
            &mut world,
            entity.stable_u64(),
            "unknown.component",
            serde_json::Value::Null,
        );
        assert!(!ok);
        assert!(message.contains("provider-backed component authority"));
    }
}

pub const RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.ecs",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_ecs_api::ECS_BACKEND_CAPABILITY_ID],
        &["scene.backend"],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

fn runtime_unit_factory(
    engine: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    let scene = engine
        .resources_mut()
        .get::<std::sync::Arc<newengine_scene_runtime::SceneBridge>>()
        .cloned()
        .ok_or_else(|| newengine_runtime_unit_api::EngineError::Other(
            "ECS runtime unit requires instance Arc<SceneBridge> resource before materialization".to_owned(),
        ))?;
    register_ecs_gateway_best_effort(scene);
    Ok(None)
}

pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );
