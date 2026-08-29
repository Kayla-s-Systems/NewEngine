use newengine_world_api::WorldCellCoord;
use serde::{Deserialize, Serialize};

use crate::{AabbDto, EnvironmentObjectId, EnvironmentObjectKind, TransformDto};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentObjectDto {
    pub id: EnvironmentObjectId,
    pub kind: EnvironmentObjectKind,
    pub bounds: AabbDto,
    pub owning_cells: Vec<WorldCellCoord>,
    pub transform: TransformDto,
    pub tags: Vec<String>,
    pub state_json: serde_json::Value,
}

impl Default for EnvironmentObjectDto {
    #[inline]
    fn default() -> Self {
        Self {
            id: EnvironmentObjectId::default(),
            kind: EnvironmentObjectKind::CloudField,
            bounds: AabbDto::default(),
            owning_cells: Vec::new(),
            transform: TransformDto::default(),
            tags: Vec::new(),
            state_json: serde_json::Value::Null,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentDiagnosticsDto {
    pub provider: String,
    pub provider_route: String,
    pub degraded: bool,
    pub deterministic_key: String,
    pub active_profile: String,
    pub reasons: Vec<String>,
    pub warnings: Vec<String>,
}

impl Default for EnvironmentDiagnosticsDto {
    #[inline]
    fn default() -> Self {
        Self {
            provider: "environment.default".to_owned(),
            provider_route: "engine.world.default.environment".to_owned(),
            degraded: false,
            deterministic_key: String::new(),
            active_profile: "environment.default".to_owned(),
            reasons: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentResidencyIntentDto {
    pub object_id: EnvironmentObjectId,
    pub owning_cells: Vec<WorldCellCoord>,
    pub required_assets: Vec<String>,
    pub priority: String,
    pub reason: String,
}

impl Default for EnvironmentResidencyIntentDto {
    #[inline]
    fn default() -> Self {
        Self {
            object_id: EnvironmentObjectId::default(),
            owning_cells: Vec::new(),
            required_assets: Vec::new(),
            priority: "background".to_owned(),
            reason: "environment".to_owned(),
        }
    }
}
