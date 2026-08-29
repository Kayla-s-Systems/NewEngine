use newengine_entity_api::EntityHandle;
use serde::{Deserialize, Serialize};

use crate::{WorldBootPhase, WorldCellRecord, WorldPartitionState};

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
