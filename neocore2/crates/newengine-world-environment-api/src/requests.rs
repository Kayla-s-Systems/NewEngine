use newengine_world_api::WorldCellCoord;
use serde::{Deserialize, Serialize};

use crate::{EnvironmentDiagnosticsDto, EnvironmentFrameDto, EnvironmentFrameRequest, Vec3Dto};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentSampleAtPositionRequest {
    pub frame: EnvironmentFrameDto,
    pub position: Vec3Dto,
    pub cell: Option<WorldCellCoord>,
}

impl Default for EnvironmentSampleAtPositionRequest {
    #[inline]
    fn default() -> Self {
        Self {
            frame: EnvironmentFrameDto::neutral_degraded(
                0,
                "world.runtime.default",
                "environment.sample.default",
            ),
            position: Vec3Dto::zero(),
            cell: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentSampleAtPositionResponse {
    pub position: Vec3Dto,
    pub cell: Option<WorldCellCoord>,
    pub visibility_multiplier: f32,
    pub wind_velocity: Vec3Dto,
    pub weather_tags: Vec<String>,
    pub diagnostics: EnvironmentDiagnosticsDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentSnapshotRequest {
    pub include_objects: bool,
}

impl Default for EnvironmentSnapshotRequest {
    #[inline]
    fn default() -> Self {
        Self {
            include_objects: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentSnapshotResponse {
    pub schema: String,
    pub frame: EnvironmentFrameDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentRestoreRequest {
    pub snapshot: EnvironmentSnapshotResponse,
}

impl Default for EnvironmentRestoreRequest {
    #[inline]
    fn default() -> Self {
        Self {
            snapshot: EnvironmentSnapshotResponse::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentRestoreResponse {
    pub ok: bool,
    pub frame: EnvironmentFrameDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentPreviewTimeRequest {
    pub base_request: EnvironmentFrameRequest,
    pub normalized_time_of_day: f32,
}

impl Default for EnvironmentPreviewTimeRequest {
    #[inline]
    fn default() -> Self {
        Self {
            base_request: EnvironmentFrameRequest::default(),
            normalized_time_of_day: 0.5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentInvokeRequest {
    pub method: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}
