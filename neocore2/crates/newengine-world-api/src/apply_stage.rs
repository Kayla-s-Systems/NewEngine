use serde::{Deserialize, Serialize};

use crate::WorldRuntimeState;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldApplyStageCommand {
    pub command: String,
    #[serde(default)]
    pub guid: Option<u128>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub parent: Option<u128>,
    #[serde(default)]
    pub transform: Option<serde_json::Value>,
    #[serde(default)]
    pub definition_ref: Option<String>,
    #[serde(default)]
    pub source_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldApplyStageRequest {
    pub stage: String,
    #[serde(default)]
    pub transaction_id: String,
    #[serde(default)]
    pub commands: Vec<WorldApplyStageCommand>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldApplyStageResponse {
    pub ok: bool,
    pub stage: String,
    #[serde(default)]
    pub transaction_id: String,
    pub applied_count: usize,
    pub state: WorldRuntimeState,
    #[serde(default)]
    pub undo_commands: Vec<WorldApplyStageCommand>,
}
