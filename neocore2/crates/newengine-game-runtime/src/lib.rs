#![forbid(unsafe_op_in_unsafe_fn)]

//! Standalone game/runtime composition profile.
//!
//! This crate is the app-facing runtime boundary for playable builds. Game code
//! registers a profile and scene bootstrap; rendering remains owned by the
//! engine render controller and the selected render plugin. No authoring UI, panels,
//! docking, hierarchy, property grid, or Vulkan-specific resource work is pulled
//! into the game binary.

pub mod game_ready_fps;

use newengine_assets::{AssetAccess, AssetServiceClient};
use newengine_core::{
    Engine, EngineLifecycleEvent, EngineReadinessKey, EngineReadinessSnapshot, EngineResult, Module, ModuleCtx,
    StartupConfig,
};
use newengine_render_feature_gameready::GameReadyRenderFeaturePack;
use newengine_runtime_host::physics_runtime::PhysicsBackendRuntimeModule;
use newengine_runtime_host::render_runtime::RenderBackendRuntimeModule;
use newengine_ui::{UiBuildFn, UiProviderKind};
use std::any::Any;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    engine_owned_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_owned_gateway_service, EngineOwnedGatewayDecl, JsonServiceRouter,
};
use newengine_scene::{SceneAsset, SceneAssetOptions};
use newengine_scene_io::{method as scene_method, ENGINE_SCENE_SERVICE_ID, SCENE_BACKEND_CAPABILITY_ID};
use newengine_ecs_api::{
    EcsCommand, EcsCommandRequest, EcsCommandResponse, EcsCommandResult, EcsInvokeRequest,
    EcsServiceInfo, EcsSnapshotRequest, EcsWorldSnapshot, EcsWorldSummary,
    ECS_BACKEND_CAPABILITY_ID, ENGINE_ECS_SERVICE_ID,
};
use newengine_entity_api::{
    EntityDespawnRequest, EntityDespawnResponse, EntityDespawnResult, EntityExistsRequest,
    EntityExistsResponse, EntityHandle, EntityInvokeRequest, EntityListRequest, EntityListResponse,
    EntityRecord, EntityServiceInfo, EntitySpawnRequest, EntitySpawnResponse, ENTITY_BACKEND_CAPABILITY_ID,
    ENGINE_ENTITY_SERVICE_ID,
};

use newengine_runtime_host::asset_bootstrap::{
    collect_app_asset_roots, mount_asset_roots_best_effort,
};

pub use newengine_engine_runtime::{PhysicsBodyDesc, CollisionShapeDesc, GameRunMode, GameplayActor, PlayerActor};

pub use game_ready_fps::{run_game_ready_fps_process, GameReadyFpsApp};

pub const GAME_FIXED_DT_MS: u32 = 16;
pub const GAME_APP_ASSETS_DIR_ENV: &str = "NEWENGINE_GAME_ASSETS_DIR";
pub const GAME_READY_APP_DIR_NAME: &str = "game-ready-fps";

const GAME_READY_SCENE_BOOTSTRAP_REQUIRES: &[EngineReadinessKey] = &[
    EngineReadinessKey::EnginePluginsReady,
];

struct GameReadySceneBootstrapModule {
    scene: Arc<newengine_engine_runtime::SceneBridge>,
    bootstrapped: bool,
    waiting_logged: bool,
}

impl GameReadySceneBootstrapModule {
    #[inline]
    fn new(scene: Arc<newengine_engine_runtime::SceneBridge>) -> Self {
        Self {
            scene,
            bootstrapped: false,
            waiting_logged: false,
        }
    }

    #[inline]
    fn log_waiting_once(&mut self, origin: &'static str) {
        if self.waiting_logged {
            return;
        }
        self.waiting_logged = true;
        log::info!(
            "game-ready runtime: waiting for AssetManager/geometryImporter readiness before scene bootstrap origin='{}'",
            origin
        );
    }

    #[inline]
    fn try_bootstrap(&mut self, origin: &'static str) -> EngineResult<()> {
        if self.bootstrapped {
            return Ok(());
        }

        if !newengine_plugin_host::has_service(newengine_assets::consts::ASSET_SERVICE_ID) {
            self.log_waiting_once(origin);
            return Ok(());
        }

        let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
        let asset_roots = collect_app_asset_roots(GAME_READY_APP_DIR_NAME, GAME_APP_ASSETS_DIR_ENV);
        mount_asset_roots_best_effort(&assets, &asset_roots);

        match self.scene.bootstrap_game_ready_scene_now() {
            Some(player) => {
                self.bootstrapped = true;
                log::info!(
                    "game-ready runtime: CPU scene bootstrapped via lifecycle dispatch origin='{}' selected_player={:?}; waiting for launch gate before public Play",
                    origin,
                    player
                );
            }
            None => {
                log::warn!(
                    "game-ready runtime: scene bootstrap failed after readiness dispatch origin='{}'",
                    origin
                );
            }
        }

        Ok(())
    }
}

impl<E: Send + 'static> Module<E> for GameReadySceneBootstrapModule {
    #[inline]
    fn id(&self) -> &'static str {
        "app.game_ready_scene_bootstrap"
    }

    #[inline]
    fn startup_requires(&self) -> &'static [EngineReadinessKey] {
        GAME_READY_SCENE_BOOTSTRAP_REQUIRES
    }

    #[inline]
    fn start(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let origin = if ctx
            .resources()
            .get::<EngineReadinessSnapshot>()
            .map(|s| s.engine_plugins_ready)
            .unwrap_or(false)
        {
            "startup-graph-engine-plugins-ready"
        } else {
            "startup-graph-unexpected-early-start"
        };
        self.try_bootstrap(origin)
    }

    #[inline]
    fn on_event(&mut self, _ctx: &mut ModuleCtx<'_, E>, event: &dyn Any) -> EngineResult<()> {
        let Some(event) = event.downcast_ref::<EngineLifecycleEvent>() else {
            return Ok(());
        };

        match event {
            EngineLifecycleEvent::EnginePluginsReady { origin, .. } => {
                self.try_bootstrap(origin)
            }
            EngineLifecycleEvent::EngineStartCompleted { .. } => {
                if self.bootstrapped {
                    Ok(())
                } else {
                    self.log_waiting_once("engine-start-completed");
                    Ok(())
                }
            }
        }
    }

    #[inline]
    fn update(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        if !self.bootstrapped
            && newengine_plugin_host::has_service(newengine_assets::consts::ASSET_SERVICE_ID)
        {
            self.try_bootstrap("update-readiness-fallback")?;
        }
        Ok(())
    }
}

#[derive(Clone)]
struct EngineSceneGatewayService {
    scene: Arc<newengine_engine_runtime::SceneBridge>,
}

impl EngineSceneGatewayService {
    #[inline]
    fn new(scene: Arc<newengine_engine_runtime::SceneBridge>) -> Self {
        Self { scene }
    }


    fn formats_json(&self) -> RResult<Blob, RString> {
        ok_json(serde_json::json!({
            "id": ENGINE_SCENE_SERVICE_ID,
            "origin": "engine-owned",
            "owner": "newengine-game-runtime.scene-bridge",
            "version": 1,
            "formats": [
                {
                    "id": "newengine.scene.asset.v1",
                    "schema": "kalitech.scene.asset.v1",
                    "media_type": "application/json",
                    "load": true,
                    "save": true
                }
            ],
            "methods": [
                scene_method::FORMATS_JSON,
                scene_method::LOAD_JSON_V1,
                scene_method::SAVE_JSON_V1
            ]
        }))
    }

    fn load_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };

        let path = req
            .get("path")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let Some(path) = path else {
            return RResult::RErr(RString::from("engine.scene load_json_v1 requires non-empty path"));
        };

        let replace = req.get("replace").and_then(|v| v.as_bool()).unwrap_or(true);
        if !replace {
            return RResult::RErr(RString::from(
                "engine.scene load_json_v1 currently supports replace=true only",
            ));
        }

        if !newengine_plugin_host::has_service(newengine_assets::consts::ASSET_SERVICE_ID) {
            return RResult::RErr(RString::from(format!(
                "engine.scene cannot load '{path}': asset gateway '{}' is unavailable",
                newengine_assets::consts::ASSET_SERVICE_ID
            )));
        }

        let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
        let asset_roots = collect_app_asset_roots(GAME_READY_APP_DIR_NAME, GAME_APP_ASSETS_DIR_ENV);
        mount_asset_roots_best_effort(&assets, &asset_roots);

        let bytes = match assets.text_v1(path) {
            Ok(bytes) => bytes,
            Err(e) => {
                return RResult::RErr(RString::from(format!(
                    "engine.scene load_json_v1 asset read failed path='{path}' err='{e}'"
                )));
            }
        };

        let asset = match serde_json::from_slice::<SceneAsset>(&bytes) {
            Ok(asset) => asset,
            Err(e) => {
                return RResult::RErr(RString::from(format!(
                    "engine.scene load_json_v1 scene json parse failed path='{path}' err='{e}'"
                )));
            }
        };

        {
            let scene_lock = self.scene.scene();
            let mut scene = scene_lock.write();
            if let Err(e) = scene.load_asset(&asset) {
                return RResult::RErr(RString::from(format!(
                    "engine.scene load_json_v1 scene apply failed path='{path}' err='{e}'"
                )));
            }
        }

        ok_json(serde_json::json!({
            "ok": true,
            "path": path,
            "replace": true,
            "entities": asset.entities.len(),
            "schema": asset.schema,
            "version": asset.version
        }))
    }

    fn save_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
        let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let pretty = req.get("pretty").and_then(|v| v.as_bool()).unwrap_or(true);
        let include_empty_entities = req
            .get("options")
            .and_then(|v| v.get("include_empty_entities"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let asset = {
            let scene_lock = self.scene.scene();
            let mut scene = scene_lock.write();
            scene.to_asset(SceneAssetOptions { include_empty_entities })
        };

        let payload = match serde_json::to_value(&asset) {
            Ok(value) => value,
            Err(e) => return RResult::RErr(RString::from(e.to_string())),
        };
        let payload_text = match if pretty {
            serde_json::to_string_pretty(&asset)
        } else {
            serde_json::to_string(&asset)
        } {
            Ok(value) => value,
            Err(e) => return RResult::RErr(RString::from(e.to_string())),
        };

        ok_json(serde_json::json!({
            "ok": true,
            "path": path,
            "stored": false,
            "storage": "caller-owned",
            "pretty": pretty,
            "payload": payload,
            "payload_text": payload_text
        }))
    }
}




#[derive(Clone)]
struct EngineEcsGatewayService {
    scene: Arc<newengine_engine_runtime::SceneBridge>,
}

impl EngineEcsGatewayService {
    #[inline]
    fn new(scene: Arc<newengine_engine_runtime::SceneBridge>) -> Self {
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





const MAX_ENTITY_SPAWN_PER_CALL: usize = 4096;

#[derive(Clone)]
struct EngineEntityGatewayService {
    scene: Arc<newengine_engine_runtime::SceneBridge>,
}

impl EngineEntityGatewayService {
    #[inline]
    fn new(scene: Arc<newengine_engine_runtime::SceneBridge>) -> Self {
        Self { scene }
    }


    #[inline]
    fn handle(id: newengine_ecs::EntityId) -> EntityHandle {
        EntityHandle::new(id.stable_u64())
    }

    fn find_entity_by_handle(world: &newengine_ecs::World, handle: EntityHandle) -> Option<newengine_ecs::EntityId> {
        world.iter_entities().find(|id| id.stable_u64() == handle.stable_id)
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
            entities.push(EntityRecord { handle: Self::handle(id) });
        }

        ok_json(&EntityListResponse {
            entities,
            truncated,
            total_count: world.entity_count() as u64,
        })
    }

    fn exists_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload)
            .and_then(|v| serde_json::from_value::<EntityExistsRequest>(v).map_err(|e| e.to_string()))
        {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };

        let scene_lock = self.scene.scene();
        let scene = scene_lock.read();
        let world = scene.world();
        let exists = Self::find_entity_by_handle(world, req.entity).is_some();

        ok_json(&EntityExistsResponse { entity: req.entity, exists })
    }

    fn spawn_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload)
            .and_then(|v| serde_json::from_value::<EntitySpawnRequest>(v).map_err(|e| e.to_string()))
        {
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
            entities.push(EntityRecord { handle: Self::handle(id) });
        }

        ok_json(&EntitySpawnResponse {
            entities,
            tick: world.tick(),
            total_count: world.entity_count() as u64,
        })
    }

    fn despawn_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload)
            .and_then(|v| serde_json::from_value::<EntityDespawnRequest>(v).map_err(|e| e.to_string()))
        {
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
                message: if ok { "entity despawned" } else { "entity not found" }.to_owned(),
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
        let req = match payload_json(&payload)
            .and_then(|v| serde_json::from_value::<EntityInvokeRequest>(v).map_err(|e| e.to_string()))
        {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
        let payload = match serde_json::to_vec(&req.payload) {
            Ok(bytes) => Blob::from(bytes),
            Err(e) => return RResult::RErr(RString::from(e.to_string())),
        };

        match req.method.as_str() {
            newengine_entity_api::ENTITY_SERVICE_METHOD_LIST_JSON_V1 => self.list_json_v1(payload),
            newengine_entity_api::ENTITY_SERVICE_METHOD_EXISTS_JSON_V1 => self.exists_json_v1(payload),
            newengine_entity_api::ENTITY_SERVICE_METHOD_SPAWN_JSON_V1 => self.spawn_json_v1(payload),
            newengine_entity_api::ENTITY_SERVICE_METHOD_DESPAWN_JSON_V1 => self.despawn_json_v1(payload),
            other => RResult::RErr(RString::from(format!(
                "engine.entity invoke_json unknown target method '{other}'"
            ))),
        }
    }
}




const SCENE_GATEWAY_OWNER: &str = "newengine-game-runtime.scene-bridge";
const ECS_GATEWAY_OWNER: &str = "newengine-game-runtime.ecs-gateway";
const ENTITY_GATEWAY_OWNER: &str = "newengine-game-runtime.entity-gateway";

fn scene_gateway_service(scene: Arc<newengine_engine_runtime::SceneBridge>) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let service = EngineSceneGatewayService::new(scene);
    let description = serde_json::json!({
        "id": ENGINE_SCENE_SERVICE_ID,
        "version": 1,
        "contract": "newengine.scene gateway >= 0.1.x",
        "origin": "engine-owned",
        "owner": SCENE_GATEWAY_OWNER,
        "capability": SCENE_BACKEND_CAPABILITY_ID,
        "methods": [
            scene_method::FORMATS_JSON,
            scene_method::LOAD_JSON_V1,
            scene_method::SAVE_JSON_V1
        ]
    });

    let formats_service = service.clone();
    let load_service = service.clone();
    let save_service = service;

    JsonServiceRouter::new(ENGINE_SCENE_SERVICE_ID)
        .describe_json(&description)
        .blob(scene_method::FORMATS_JSON, move |_unit, _payload| formats_service.formats_json())
        .blob(scene_method::LOAD_JSON_V1, move |_unit, payload| load_service.load_json_v1(payload))
        .blob(scene_method::SAVE_JSON_V1, move |_unit, payload| save_service.save_json_v1(payload))
        .into_service_v1()
}

fn ecs_gateway_service(scene: Arc<newengine_engine_runtime::SceneBridge>) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let service = EngineEcsGatewayService::new(scene);
    let info = EcsServiceInfo::default();
    let description = engine_owned_service_description(
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

fn entity_gateway_service(scene: Arc<newengine_engine_runtime::SceneBridge>) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let service = EngineEntityGatewayService::new(scene);
    let info = EntityServiceInfo::default();
    let description = engine_owned_service_description(
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
        .blob(newengine_entity_api::ENTITY_SERVICE_METHOD_INVOKE, move |_unit, payload| invoke_service.invoke_json(payload))
        .blob(newengine_entity_api::ENTITY_SERVICE_METHOD_LIST_JSON_V1, move |_unit, payload| list_service.list_json_v1(payload))
        .blob(newengine_entity_api::ENTITY_SERVICE_METHOD_EXISTS_JSON_V1, move |_unit, payload| exists_service.exists_json_v1(payload))
        .blob(newengine_entity_api::ENTITY_SERVICE_METHOD_SPAWN_JSON_V1, move |_unit, payload| spawn_service.spawn_json_v1(payload))
        .blob(newengine_entity_api::ENTITY_SERVICE_METHOD_DESPAWN_JSON_V1, move |_unit, payload| despawn_service.despawn_json_v1(payload))
        .blob(newengine_entity_api::ENTITY_SERVICE_METHOD_SHUTDOWN_V1, |_unit, _payload| ok_empty_blob())
        .into_service_v1()
}

#[derive(Clone)]
pub struct StandaloneGameRuntimeProfile {
    viewport: Arc<newengine_engine_runtime::ViewportBridge>,
    plugins: Arc<newengine_engine_runtime::PluginManagerBridge>,
    scene: Arc<newengine_engine_runtime::SceneBridge>,
}

impl Default for StandaloneGameRuntimeProfile {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl StandaloneGameRuntimeProfile {
    #[inline]
    pub fn new() -> Self {
        Self {
            viewport: Arc::new(newengine_engine_runtime::ViewportBridge::new()),
            plugins: Arc::new(newengine_engine_runtime::PluginManagerBridge::new()),
            scene: Arc::new(newengine_engine_runtime::SceneBridge::new(newengine_scene::Scene::new())),
        }
    }

    #[inline]
    pub fn register_modules(
        &self,
        engine: &mut Engine<()>,
        startup: &StartupConfig,
    ) -> EngineResult<()> {
        engine.register_module(Box::new(PhysicsBackendRuntimeModule::new(
            startup.modules_dir.clone(),
        )))?;

        engine.register_module(Box::new(RenderBackendRuntimeModule::new(
            startup.modules_dir.clone(),
        )))?;

        let render_controller = GameReadyRenderFeaturePack::new().install(
            newengine_engine_runtime::RuntimeRenderController::new(
                Arc::clone(&self.viewport),
                Arc::clone(&self.plugins),
                Arc::clone(&self.scene),
            ),
        );

        engine.register_module(Box::new(render_controller))?;

        engine.register_module(Box::new(GameReadySceneBootstrapModule::new(Arc::clone(
            &self.scene,
        ))))?;
        Ok(())
    }

    #[inline]
    pub fn register_scene_io_best_effort(&self) {
        if newengine_plugin_host::has_service(ENGINE_SCENE_SERVICE_ID) {
            log::debug!(
                "engine.scene gateway registration skipped; service already available"
            );
            return;
        }

        let service = scene_gateway_service(Arc::clone(&self.scene));
        match register_engine_owned_gateway_service(EngineOwnedGatewayDecl {
            gateway: ENGINE_SCENE_SERVICE_ID,
            service_kind: newengine_service_api::EngineServiceKind::Scene,
            provider_service: ENGINE_SCENE_SERVICE_ID,
            capability: SCENE_BACKEND_CAPABILITY_ID,
            priority: 0,
            owner: SCENE_GATEWAY_OWNER,
            service,
        }) {
            Ok(()) => log::info!(
                "engine.scene gateway registered source=engine-owned service='{}' capability='{}'",
                ENGINE_SCENE_SERVICE_ID,
                SCENE_BACKEND_CAPABILITY_ID
            ),
            Err(e) => log::error!(
                "engine.scene gateway registration failed id='{}' err='{}'",
                ENGINE_SCENE_SERVICE_ID,
                e
            ),
        }
    }


    #[inline]
    pub fn register_ecs_gateway_best_effort(&self) {
        if newengine_plugin_host::has_service(ENGINE_ECS_SERVICE_ID) {
            log::debug!(
                "engine.ecs gateway registration skipped; service already available"
            );
            return;
        }

        let service = ecs_gateway_service(Arc::clone(&self.scene));
        match register_engine_owned_gateway_service(EngineOwnedGatewayDecl {
            gateway: ENGINE_ECS_SERVICE_ID,
            service_kind: newengine_service_api::EngineServiceKind::Ecs,
            provider_service: ENGINE_ECS_SERVICE_ID,
            capability: ECS_BACKEND_CAPABILITY_ID,
            priority: 0,
            owner: ECS_GATEWAY_OWNER,
            service,
        }) {
            Ok(()) => log::info!(
                "engine.ecs gateway registered source=engine-owned service='{}' capability='{}'",
                ENGINE_ECS_SERVICE_ID,
                ECS_BACKEND_CAPABILITY_ID
            ),
            Err(e) => log::error!(
                "engine.ecs gateway registration failed id='{}' err='{}'",
                ENGINE_ECS_SERVICE_ID,
                e
            ),
        }
    }

    #[inline]
    pub fn register_entity_gateway_best_effort(&self) {
        if newengine_plugin_host::has_service(ENGINE_ENTITY_SERVICE_ID) {
            log::debug!(
                "engine.entity gateway registration skipped; service already available"
            );
            return;
        }

        let service = entity_gateway_service(Arc::clone(&self.scene));
        match register_engine_owned_gateway_service(EngineOwnedGatewayDecl {
            gateway: ENGINE_ENTITY_SERVICE_ID,
            service_kind: newengine_service_api::EngineServiceKind::Entity,
            provider_service: ENGINE_ENTITY_SERVICE_ID,
            capability: ENTITY_BACKEND_CAPABILITY_ID,
            priority: 0,
            owner: ENTITY_GATEWAY_OWNER,
            service,
        }) {
            Ok(()) => log::info!(
                "engine.entity gateway registered source=engine-owned service='{}' capability='{}'",
                ENGINE_ENTITY_SERVICE_ID,
                ENTITY_BACKEND_CAPABILITY_ID
            ),
            Err(e) => log::error!(
                "engine.entity gateway registration failed id='{}' err='{}'",
                ENGINE_ENTITY_SERVICE_ID,
                e
            ),
        }
    }

    #[inline]
    pub fn bootstrap_game_ready_scene_best_effort(&self) {
        // Game scenes are assembled by GameReadySceneBootstrapModule during engine.start(),
        // after engine plugins are loaded. This keeps geometry imports on the required
        // AssetManager/geometryImporter path and prevents bootstrap-time filesystem fallbacks.
    }

    /// Standalone game builds render directly into the platform surface.
    /// No authoring panels, docking, hierarchy, property grid, or markup loading.
    #[inline]
    pub fn ui_build_from_startup(
        &self,
        _startup: &StartupConfig,
    ) -> Option<Box<dyn UiBuildFn>> {
        None
    }

    #[inline]
    pub fn ui_provider_kind_from_startup(&self, _startup: &StartupConfig) -> UiProviderKind {
        // UI provider selection is discovery-driven at runtime-host level.
        UiProviderKind::Null
    }

    #[inline]
    pub fn ui_provider_kind(&self) -> UiProviderKind {
        UiProviderKind::Null
    }

    /// Compatibility no-op retained so launchers can share bootstrap shape.
    #[inline]
    pub fn load_markup_best_effort(
        &self,
        _assets: Option<&dyn AssetAccess>,
        _roots: &[PathBuf],
        _path: &str,
        _timeout: Duration,
    ) {
    }
}
