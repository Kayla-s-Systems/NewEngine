#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted `engine.world` gateway service.
//!
//! `engine.scene` is authored structure. `engine.world` is the living runtime instance.
//! This service is a provider route behind the `engine.world` gateway and exposes
//! DTOs only: no native ECS `World`, no native `EntityId`, no renderer handles.

use std::sync::Arc;

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_scene::{SceneAsset, SceneAssetOptions, SceneEntityAsset, TransformAsset};
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_gateway_provider_service_dynamic, EngineGatewayProviderDeclDynamic,
    JsonServiceRouter,
};
use newengine_world_api::{
    WorldActiveCellsRequest, WorldActiveCellsResponse, WorldApplyStageRequest,
    WorldApplyStageResponse, WorldBootPhase, WorldBootRequest, WorldBootResponse, WorldCellCoord,
    WorldCellRecord, WorldCellResidency, WorldInvokeRequest, WorldLoadSnapshotRequest,
    WorldLoadSnapshotResponse, WorldPartitionResponse, WorldPartitionState,
    WorldRestoreSnapshotRequest, WorldRestoreSnapshotResponse, WorldRuntimeState,
    WorldSaveSnapshotRequest, WorldSaveSnapshotResponse, WorldServiceInfo, WorldSnapshotRequest,
    WorldSnapshotResponse, WorldStateRequest, WorldStateResponse, WorldStreamingCellDto,
    WorldStreamingCellsRequest, WorldStreamingCellsResponse, WorldStreamingPlanDto,
    ENGINE_WORLD_SERVICE_ID, WORLD_BACKEND_CAPABILITY_ID, WORLD_SERVICE_ID,
    WORLD_SERVICE_METHOD_ACTIVE_CELLS_JSON_V1, WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1,
    WORLD_SERVICE_METHOD_BOOT_JSON_V1, WORLD_SERVICE_METHOD_INFO, WORLD_SERVICE_METHOD_INVOKE,
    WORLD_SERVICE_METHOD_LOAD_SNAPSHOT_JSON_V1, WORLD_SERVICE_METHOD_PARTITION_JSON_V1,
    WORLD_SERVICE_METHOD_RESTORE_SNAPSHOT_JSON_V1, WORLD_SERVICE_METHOD_SAVE_SNAPSHOT_JSON_V1,
    WORLD_SERVICE_METHOD_SHUTDOWN_V1, WORLD_SERVICE_METHOD_SNAPSHOT_JSON_V1,
    WORLD_SERVICE_METHOD_STATE_JSON_V1, WORLD_SERVICE_METHOD_STREAMING_CELLS_JSON_V1,
};

pub const WORLD_GATEWAY_OWNER: &str = "newengine-world-runtime.world-gateway";
pub const WORLD_FOUNDATION_PROVIDER_ROUTE: &str = "engine.world.foundation";
const WORLD_SNAPSHOT_SCHEMA_V1: &str = "newengine.world.snapshot.v1";

mod apply_stage;
mod streaming_cells;

#[derive(Clone, Debug)]
struct WorldRuntimeBookkeeping {
    world_instance_id: String,
    phase: WorldBootPhase,
    deterministic: bool,
    boot_sequence: u64,
    seed: u64,
    partition: WorldPartitionState,
    active_cells: Vec<WorldCellRecord>,
    notes: Vec<String>,
}

impl Default for WorldRuntimeBookkeeping {
    #[inline]
    fn default() -> Self {
        Self {
            world_instance_id: "world.runtime.default".to_owned(),
            phase: WorldBootPhase::Cold,
            deterministic: true,
            boot_sequence: 0,
            seed: 0,
            partition: WorldPartitionState::default(),
            active_cells: Vec::new(),
            notes: vec![
                "Scene is authored structure; World is living runtime instance.".to_owned(),
                "ECS remains storage behind engine.ecs; native EntityId is not exposed.".to_owned(),
            ],
        }
    }
}

#[derive(Clone)]
pub struct EngineWorldGatewayService {
    scene: Arc<newengine_scene_runtime::SceneBridge>,
    state: Arc<parking_lot::Mutex<WorldRuntimeBookkeeping>>,
}

impl EngineWorldGatewayService {
    #[inline]
    pub fn new(scene: Arc<newengine_scene_runtime::SceneBridge>) -> Self {
        Self {
            scene,
            state: Arc::new(parking_lot::Mutex::new(WorldRuntimeBookkeeping::default())),
        }
    }

    fn authority_json(&self) -> serde_json::Value {
        let snap = self.scene.authority_snapshot();
        let route_json =
            |route: &newengine_runtime_host::world_authority::WorldAuthorityGatewayRoute| {
                serde_json::json!({
                    "gateway": route.gateway_id,
                    "kind": route.service_kind,
                    "provider_service": route.provider_service_id,
                    "provider_owner": route.provider_owner_id,
                    "capability": route.backend_capability_id,
                    "origin": route.origin,
                    "priority": route.backend_priority,
                    "score": route.active_score,
                })
            };
        serde_json::json!({
            "authority": snap.authority_label(),
            "split": snap.has_split_world_authority(),
            "ecs": snap.ecs.as_ref().map(|route| route_json(route)),
            "entity": snap.entity.as_ref().map(|route| route_json(route)),
            "scene": snap.scene.as_ref().map(|route| route_json(route)),
            "world": snap.world.as_ref().map(|route| route_json(route)),
            "physics": snap.physics.as_ref().map(|route| route_json(route)),
            "render": snap.render.as_ref().map(|route| route_json(route)),
            "notes": snap.notes,
        })
    }

    fn build_active_cells(partition: WorldPartitionState) -> Vec<WorldCellRecord> {
        if !partition.enabled {
            return Vec::new();
        }
        let radius = partition
            .simulation_radius
            .max(partition.render_radius)
            .clamp(0, 16);
        let mut cells = Vec::new();
        for z in (partition.center.z - radius)..=(partition.center.z + radius) {
            for x in (partition.center.x - radius)..=(partition.center.x + radius) {
                let dx = (x - partition.center.x).abs();
                let dz = (z - partition.center.z).abs();
                let dist = dx.max(dz);
                let residency = match (
                    dist <= partition.render_radius,
                    dist <= partition.simulation_radius,
                ) {
                    (true, true) => WorldCellResidency::RenderAndSimulation,
                    (true, false) => WorldCellResidency::Render,
                    (false, true) => WorldCellResidency::Simulation,
                    (false, false) => WorldCellResidency::Unloaded,
                };
                if !matches!(residency, WorldCellResidency::Unloaded) {
                    cells.push(WorldCellRecord {
                        coord: WorldCellCoord { x, z },
                        residency,
                        dirty: false,
                        reason: "world partition desired residency".to_owned(),
                    });
                }
            }
        }
        cells.sort_by_key(|cell| {
            let dx = cell.coord.x - partition.center.x;
            let dz = cell.coord.z - partition.center.z;
            (dx * dx + dz * dz, cell.coord.x, cell.coord.z)
        });
        cells
    }

    fn runtime_state(&self, include_cells: bool) -> WorldRuntimeState {
        let bookkeeping = self.state.lock().clone();
        let (tick, entity_count) = {
            let scene_lock = self.scene.scene();
            let scene = scene_lock.read();
            (scene.world().tick(), scene.world().entity_count() as u64)
        };
        let active_cells = if include_cells {
            bookkeeping.active_cells.clone()
        } else {
            Vec::new()
        };
        WorldRuntimeState {
            world_instance_id: bookkeeping.world_instance_id,
            phase: bookkeeping.phase,
            deterministic: bookkeeping.deterministic,
            boot_sequence: bookkeeping.boot_sequence,
            tick,
            entity_count,
            selected_entity: self.scene.selection_authority_handle(),
            partition: bookkeeping.partition,
            active_cells,
            authority: self.authority_json(),
            notes: bookkeeping.notes,
        }
    }

    fn info_json(&self) -> WorldServiceInfo {
        WorldServiceInfo::default()
    }

    fn boot_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload)
            .and_then(|v| serde_json::from_value::<WorldBootRequest>(v).map_err(|e| e.to_string()))
        {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };

        {
            let mut state = self.state.lock();
            state.boot_sequence = state.boot_sequence.saturating_add(1);
            state.deterministic = req.deterministic;
            state.seed = req.seed;
            let headless = newengine_plugin_host::active_engine_gateway_route("engine.platform")
                .map(|route| route.provider_route_id.as_deref() == Some("engine.platform.headless"))
                .unwrap_or(false);
            state.phase = if headless {
                WorldBootPhase::Headless
            } else if req
                .scene_ref
                .as_deref()
                .map(str::trim)
                .filter(|it| !it.is_empty())
                .is_some()
            {
                WorldBootPhase::SceneDeclared
            } else {
                WorldBootPhase::RuntimeBootstrapped
            };
            state.partition = req.partition;
            state.active_cells = Self::build_active_cells(state.partition.clone());
            state.notes.retain(|note| !note.starts_with("boot:"));
            state.notes.push(format!(
                "boot: deterministic={} seed={} scene_ref={}",
                req.deterministic,
                req.seed,
                req.scene_ref.as_deref().unwrap_or("<none>")
            ));
        }

        ok_json(&WorldBootResponse {
            ok: true,
            state: self.runtime_state(true),
        })
    }

    fn state_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload)
            .and_then(|v| serde_json::from_value::<WorldStateRequest>(v).map_err(|e| e.to_string()))
        {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
        ok_json(&WorldStateResponse {
            state: self.runtime_state(req.include_cells),
        })
    }

    fn active_cells_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload).and_then(|v| {
            serde_json::from_value::<WorldActiveCellsRequest>(v).map_err(|e| e.to_string())
        }) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
        let state = self.state.lock().clone();
        let mut cells = state.active_cells;
        if !req.include_unloaded {
            cells.retain(|cell| !matches!(cell.residency, WorldCellResidency::Unloaded));
        }
        ok_json(&WorldActiveCellsResponse {
            partition: state.partition,
            cells,
        })
    }

    fn partition_json_v1(&self) -> RResult<Blob, RString> {
        let partition = self.state.lock().partition.clone();
        ok_json(&WorldPartitionResponse { partition })
    }

    fn snapshot_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload).and_then(|v| {
            serde_json::from_value::<WorldSnapshotRequest>(v).map_err(|e| e.to_string())
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
        ok_json(&WorldSnapshotResponse {
            schema: WORLD_SNAPSHOT_SCHEMA_V1.to_owned(),
            state: self.runtime_state(req.include_cells),
            scene_payload,
        })
    }

    fn restore_snapshot_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload).and_then(|v| {
            serde_json::from_value::<WorldRestoreSnapshotRequest>(v).map_err(|e| e.to_string())
        }) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };

        let mut snapshot = req.snapshot;
        if req.replace_scene {
            if let Some(scene_payload) = snapshot.scene_payload.take() {
                let asset = match serde_json::from_value::<SceneAsset>(scene_payload) {
                    Ok(asset) => asset,
                    Err(e) => {
                        return RResult::RErr(RString::from(format!(
                            "world snapshot scene payload decode failed: {e}"
                        )))
                    }
                };
                let scene_lock = self.scene.scene();
                let mut scene = scene_lock.write();
                if let Err(e) = scene.load_asset(&asset) {
                    return RResult::RErr(RString::from(format!(
                        "world snapshot scene restore failed: {e}"
                    )));
                }
            }
        }

        {
            let mut state = self.state.lock();
            state.world_instance_id = snapshot.state.world_instance_id;
            state.phase = snapshot.state.phase;
            state.deterministic = snapshot.state.deterministic;
            state.boot_sequence = snapshot.state.boot_sequence;
            state.partition = snapshot.state.partition;
            state.active_cells = snapshot.state.active_cells;
            state.notes = snapshot.state.notes;
            state.notes.push("snapshot restored through engine.world; scene payload restored through engine.scene-compatible asset contract".to_owned());
        }

        ok_json(&WorldRestoreSnapshotResponse {
            ok: true,
            state: self.runtime_state(true),
        })
    }

    fn invoke_json(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload).and_then(|v| {
            serde_json::from_value::<WorldInvokeRequest>(v).map_err(|e| e.to_string())
        }) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
        let payload = match serde_json::to_vec(&req.payload) {
            Ok(bytes) => Blob::from(bytes),
            Err(e) => return RResult::RErr(RString::from(e.to_string())),
        };
        match req.method.as_str() {
            WORLD_SERVICE_METHOD_BOOT_JSON_V1 => self.boot_json_v1(payload),
            WORLD_SERVICE_METHOD_STATE_JSON_V1 => self.state_json_v1(payload),
            WORLD_SERVICE_METHOD_ACTIVE_CELLS_JSON_V1 => self.active_cells_json_v1(payload),
            WORLD_SERVICE_METHOD_STREAMING_CELLS_JSON_V1 => self.streaming_cells_json_v1(payload),
            WORLD_SERVICE_METHOD_PARTITION_JSON_V1 => self.partition_json_v1(),
            WORLD_SERVICE_METHOD_SNAPSHOT_JSON_V1 => self.snapshot_json_v1(payload),
            WORLD_SERVICE_METHOD_RESTORE_SNAPSHOT_JSON_V1 => self.restore_snapshot_json_v1(payload),
            WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1 => self.apply_stage_json_v1(payload),
            WORLD_SERVICE_METHOD_SAVE_SNAPSHOT_JSON_V1 => self.save_snapshot_json_v1(payload),
            WORLD_SERVICE_METHOD_LOAD_SNAPSHOT_JSON_V1 => self.load_snapshot_json_v1(payload),
            other => RResult::RErr(RString::from(format!(
                "engine.world invoke_json unknown target method '{other}'"
            ))),
        }
    }
}

pub fn world_gateway_service(
    scene: Arc<newengine_scene_runtime::SceneBridge>,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let service = EngineWorldGatewayService::new(scene);
    let info = WorldServiceInfo::default();
    let description = engine_gateway_provider_service_description(
        WORLD_SERVICE_ID,
        WORLD_GATEWAY_OWNER,
        WORLD_BACKEND_CAPABILITY_ID,
        info.methods.clone(),
    )
    .protocol(info.protocol.clone())
    .features(info.features.clone())
    .gateway(ENGINE_WORLD_SERVICE_ID)
    .notes("Scene = authored structure; World = living runtime world.");

    let info_service = service.clone();
    let invoke_service = service.clone();
    let boot_service = service.clone();
    let state_service = service.clone();
    let cells_service = service.clone();
    let partition_service = service.clone();
    let streaming_service = service.clone();
    let snapshot_service = service.clone();
    let restore_service = service.clone();
    let apply_service = service.clone();
    let save_snapshot_service = service.clone();
    let load_snapshot_service = service;

    JsonServiceRouter::new(WORLD_SERVICE_ID)
        .describe_json(&description)
        .get_json(WORLD_SERVICE_METHOD_INFO, move |_| info_service.info_json())
        .blob(WORLD_SERVICE_METHOD_INVOKE, move |_unit, payload| {
            invoke_service.invoke_json(payload)
        })
        .blob(WORLD_SERVICE_METHOD_BOOT_JSON_V1, move |_unit, payload| {
            boot_service.boot_json_v1(payload)
        })
        .blob(WORLD_SERVICE_METHOD_STATE_JSON_V1, move |_unit, payload| {
            state_service.state_json_v1(payload)
        })
        .blob(
            WORLD_SERVICE_METHOD_ACTIVE_CELLS_JSON_V1,
            move |_unit, payload| cells_service.active_cells_json_v1(payload),
        )
        .blob(
            WORLD_SERVICE_METHOD_STREAMING_CELLS_JSON_V1,
            move |_unit, payload| streaming_service.streaming_cells_json_v1(payload),
        )
        .blob(
            WORLD_SERVICE_METHOD_PARTITION_JSON_V1,
            move |_unit, _payload| partition_service.partition_json_v1(),
        )
        .blob(
            WORLD_SERVICE_METHOD_SNAPSHOT_JSON_V1,
            move |_unit, payload| snapshot_service.snapshot_json_v1(payload),
        )
        .blob(
            WORLD_SERVICE_METHOD_RESTORE_SNAPSHOT_JSON_V1,
            move |_unit, payload| restore_service.restore_snapshot_json_v1(payload),
        )
        .blob(
            WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1,
            move |_unit, payload| apply_service.apply_stage_json_v1(payload),
        )
        .blob(
            WORLD_SERVICE_METHOD_SAVE_SNAPSHOT_JSON_V1,
            move |_unit, payload| save_snapshot_service.save_snapshot_json_v1(payload),
        )
        .blob(
            WORLD_SERVICE_METHOD_LOAD_SNAPSHOT_JSON_V1,
            move |_unit, payload| load_snapshot_service.load_snapshot_json_v1(payload),
        )
        .blob(WORLD_SERVICE_METHOD_SHUTDOWN_V1, |_unit, _payload| {
            ok_empty_blob()
        })
        .into_service_v1()
}

pub fn register_world_gateway_best_effort(scene: Arc<newengine_scene_runtime::SceneBridge>) {
    if newengine_plugin_host::has_service(ENGINE_WORLD_SERVICE_ID) {
        newengine_ulog_api::ulog::debug!(
            "engine.world gateway registration skipped; service already available"
        );
        return;
    }

    let service = world_gateway_service(scene);
    match register_engine_gateway_provider_service_dynamic(EngineGatewayProviderDeclDynamic {
        gateway: ENGINE_WORLD_SERVICE_ID,
        service_kind: "world",
        provider_service: WORLD_SERVICE_ID,
        provider_route: WORLD_FOUNDATION_PROVIDER_ROUTE,
        capability: WORLD_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: WORLD_GATEWAY_OWNER,
        service,
    }) {
        Ok(()) => newengine_ulog_api::ulog::info!(
            "engine.world gateway registered source=engine-runtime service='{}' provider_service='{}' capability='{}' owner='{}' semantics='living runtime world; scene remains authored structure'",
            ENGINE_WORLD_SERVICE_ID,
            WORLD_SERVICE_ID,
            WORLD_BACKEND_CAPABILITY_ID,
            WORLD_GATEWAY_OWNER
        ),
        Err(e) => newengine_ulog_api::ulog::error!(
            "engine.world gateway registration failed id='{}' err='{}'",
            ENGINE_WORLD_SERVICE_ID,
            e
        ),
    }
}
