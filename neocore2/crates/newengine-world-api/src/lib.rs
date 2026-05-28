#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable service contract for the `engine.world` gateway.
//!
//! `engine.scene` and `engine.world` are parallel systems:
//! - `engine.scene` is authored structure: scene graph, archetype graph, prefab/archetype instances and placement declarations.
//! - `engine.world` is the living runtime world: boot state, partition, active cells, runtime snapshots and state coordination.
//!
//! ECS is storage. Entity/Scene/World are contracts. No native ECS entity id is exposed here.

use abi_stable::std_types::RString;
use newengine_entity_api::EntityHandle;
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use serde::{Deserialize, Serialize};

pub const ENGINE_WORLD_SERVICE_ID: &str = "engine.world";
pub const WORLD_SERVICE_ID: &str = "world.api";
pub const WORLD_BACKEND_CAPABILITY_ID: &str = "world.backend";

pub const WORLD_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const WORLD_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const WORLD_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const WORLD_SERVICE_METHOD_BOOT_JSON_V1: &str = "world.boot_json_v1";
pub const WORLD_SERVICE_METHOD_STATE_JSON_V1: &str = "world.state_json_v1";
pub const WORLD_SERVICE_METHOD_PARTITION_JSON_V1: &str = "world.partition_json_v1";
pub const WORLD_SERVICE_METHOD_ACTIVE_CELLS_JSON_V1: &str = "world.active_cells_json_v1";
pub const WORLD_SERVICE_METHOD_SNAPSHOT_JSON_V1: &str = "world.snapshot_json_v1";
pub const WORLD_SERVICE_METHOD_RESTORE_SNAPSHOT_JSON_V1: &str = "world.restore_snapshot_json_v1";

pub const WORLD_REQUIRED_METHODS_V1: &[&str] = &[
    WORLD_SERVICE_METHOD_INFO,
    WORLD_SERVICE_METHOD_INVOKE,
    WORLD_SERVICE_METHOD_SHUTDOWN_V1,
    WORLD_SERVICE_METHOD_BOOT_JSON_V1,
    WORLD_SERVICE_METHOD_STATE_JSON_V1,
    WORLD_SERVICE_METHOD_PARTITION_JSON_V1,
    WORLD_SERVICE_METHOD_ACTIVE_CELLS_JSON_V1,
    WORLD_SERVICE_METHOD_SNAPSHOT_JSON_V1,
    WORLD_SERVICE_METHOD_RESTORE_SNAPSHOT_JSON_V1,
];

pub const WORLD_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "world",
        ENGINE_WORLD_SERVICE_ID,
        WORLD_SERVICE_ID,
        WORLD_BACKEND_CAPABILITY_ID,
    );

pub const WORLD_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_WORLD_SERVICE_ID,
        "newengine.world-api >= 0.1.x",
        WORLD_REQUIRED_METHODS_V1,
    );

/// Missing `engine.world` degrades by default; strict profiles can require it.
pub const WORLD_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        WORLD_RUNTIME_CONTRACT_SPEC,
        Some(WORLD_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_WORLD_BACKEND"),
    );

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl Default for WorldServiceInfo {
    #[inline]
    fn default() -> Self {
        Self {
            protocol: "newengine.world-api/v1".to_owned(),
            features: vec![
                "runtime-world-instance".to_owned(),
                "deterministic-boot-sequence".to_owned(),
                "world-partition".to_owned(),
                "active-cells".to_owned(),
                "world-state-snapshot".to_owned(),
                "opaque-entity-handles".to_owned(),
                "scene-world-separation".to_owned(),
            ],
            methods: WORLD_REQUIRED_METHODS_V1.iter().map(|it| (*it).to_owned()).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorldBootPhase {
    Cold,
    SceneDeclared,
    RuntimeBootstrapped,
    LaunchGated,
    Playable,
    Headless,
    Shutdown,
}

impl Default for WorldBootPhase {
    #[inline]
    fn default() -> Self { Self::Cold }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct WorldCellCoord {
    pub x: i32,
    pub z: i32,
}

impl WorldCellCoord {
    #[inline]
    pub const fn new(x: i32, z: i32) -> Self { Self { x, z } }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorldCellResidency {
    Unloaded,
    Loading,
    Simulation,
    Render,
    RenderAndSimulation,
}

impl Default for WorldCellResidency {
    #[inline]
    fn default() -> Self { Self::Unloaded }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldCellRecord {
    pub coord: WorldCellCoord,
    pub residency: WorldCellResidency,
    #[serde(default)]
    pub dirty: bool,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldPartitionState {
    pub enabled: bool,
    pub cell_size_x: u32,
    pub cell_size_z: u32,
    pub center: WorldCellCoord,
    #[serde(default)]
    pub render_radius: i32,
    #[serde(default)]
    pub simulation_radius: i32,
}

impl Default for WorldPartitionState {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: false,
            cell_size_x: 0,
            cell_size_z: 0,
            center: WorldCellCoord::default(),
            render_radius: 0,
            simulation_radius: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldRuntimeState {
    pub world_instance_id: String,
    pub phase: WorldBootPhase,
    pub deterministic: bool,
    pub boot_sequence: u64,
    pub tick: u64,
    pub entity_count: u64,
    #[serde(default)]
    pub selected_entity: Option<EntityHandle>,
    pub partition: WorldPartitionState,
    #[serde(default)]
    pub active_cells: Vec<WorldCellRecord>,
    #[serde(default)]
    pub authority: serde_json::Value,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldBootRequest {
    #[serde(default)]
    pub deterministic: bool,
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub scene_ref: Option<String>,
    #[serde(default)]
    pub partition: WorldPartitionState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldBootResponse {
    pub ok: bool,
    pub state: WorldRuntimeState,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WorldStateRequest {
    #[serde(default)]
    pub include_cells: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldStateResponse {
    pub state: WorldRuntimeState,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WorldActiveCellsRequest {
    #[serde(default)]
    pub include_unloaded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldActiveCellsResponse {
    pub partition: WorldPartitionState,
    #[serde(default)]
    pub cells: Vec<WorldCellRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldPartitionResponse {
    pub partition: WorldPartitionState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldSnapshotRequest {
    #[serde(default)]
    pub include_scene_payload: bool,
    #[serde(default)]
    pub include_cells: bool,
}

impl Default for WorldSnapshotRequest {
    #[inline]
    fn default() -> Self { Self { include_scene_payload: true, include_cells: true } }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldSnapshotResponse {
    pub schema: String,
    pub state: WorldRuntimeState,
    #[serde(default)]
    pub scene_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldRestoreSnapshotRequest {
    pub snapshot: WorldSnapshotResponse,
    #[serde(default = "default_true")]
    pub replace_scene: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldRestoreSnapshotResponse {
    pub ok: bool,
    pub state: WorldRuntimeState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldInvokeRequest {
    pub method: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[inline]
fn default_true() -> bool { true }

/// Thin host-side client over `engine.world`.
#[derive(Clone)]
pub struct WorldClient {
    host: HostApiV1,
    service_id: RString,
}

impl WorldClient {
    #[inline]
    pub fn new(host: HostApiV1) -> Self {
        Self { host, service_id: RString::from(ENGINE_WORLD_SERVICE_ID) }
    }

    #[inline]
    fn call_json(&self, method: &str, payload: serde_json::Value) -> Result<serde_json::Value, String> {
        let payload = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
        let res = (self.host.call_service_v1)(
            self.service_id.clone(),
            MethodName::from(method),
            Blob::from(payload),
        );
        let bytes = res.into_result().map_err(|e| e.to_string())?.into_vec();
        serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|e| e.to_string())
    }

    #[inline]
    pub fn state_json_v1(&self, include_cells: bool) -> Result<serde_json::Value, String> {
        self.call_json(WORLD_SERVICE_METHOD_STATE_JSON_V1, serde_json::json!({ "include_cells": include_cells }))
    }

    #[inline]
    pub fn active_cells_json_v1(&self) -> Result<serde_json::Value, String> {
        self.call_json(WORLD_SERVICE_METHOD_ACTIVE_CELLS_JSON_V1, serde_json::json!({}))
    }

    #[inline]
    pub fn snapshot_json_v1(&self) -> Result<serde_json::Value, String> {
        self.call_json(WORLD_SERVICE_METHOD_SNAPSHOT_JSON_V1, serde_json::json!({ "include_scene_payload": true, "include_cells": true }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_service_ids_are_gateway_first() {
        assert_eq!(ENGINE_WORLD_SERVICE_ID, "engine.world");
        assert_eq!(WORLD_BACKEND_SERVICE_SPEC.engine_gateway_id, ENGINE_WORLD_SERVICE_ID);
        assert_eq!(WORLD_BACKEND_SERVICE_SPEC.provider_service_id, WORLD_SERVICE_ID);
        assert_eq!(WORLD_BACKEND_SERVICE_SPEC.backend_capability_id, WORLD_BACKEND_CAPABILITY_ID);
    }
}
