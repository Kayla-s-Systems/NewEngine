use serde::{Deserialize, Serialize};

use crate::WorldRuntimeState;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldSnapshotRequest {
    #[serde(default)]
    pub include_scene_payload: bool,
    #[serde(default)]
    pub include_cells: bool,
}

impl Default for WorldSnapshotRequest {
    #[inline]
    fn default() -> Self {
        Self {
            include_scene_payload: true,
            include_cells: true,
        }
    }
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
pub struct WorldSaveSnapshotRequest {
    #[serde(default = "default_true")]
    pub include_scene_payload: bool,
    #[serde(default = "default_true")]
    pub include_cells: bool,
    #[serde(default)]
    pub target_ref: Option<String>,
}

impl Default for WorldSaveSnapshotRequest {
    #[inline]
    fn default() -> Self {
        Self {
            include_scene_payload: true,
            include_cells: true,
            target_ref: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldSaveSnapshotResponse {
    pub ok: bool,
    pub storage: String,
    #[serde(default)]
    pub target_ref: Option<String>,
    pub snapshot: WorldSnapshotResponse,
    pub payload_text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldLoadSnapshotRequest {
    #[serde(default)]
    pub snapshot: Option<WorldSnapshotResponse>,
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
    #[serde(default = "default_true")]
    pub replace_scene: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldLoadSnapshotResponse {
    pub ok: bool,
    pub state: WorldRuntimeState,
}

#[inline]
fn default_true() -> bool {
    true
}
