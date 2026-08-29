use serde::{Deserialize, Serialize};

use crate::{WorldPartitionState, WorldRuntimeState};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldInvokeRequest {
    pub method: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}
