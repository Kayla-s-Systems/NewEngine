use newengine_world_api::WorldCellCoord;
use serde::{Deserialize, Serialize};

use crate::{AtmosphereStateDto, CloudStateDto, WeatherStateDto, WindStateDto};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentSurfaceBoundaryDto {
    pub cell: WorldCellCoord,
    /// Mean physical terrain height of the atmospheric cell above sea level [m].
    pub terrain_elevation_meters: f32,
    /// Broadband short-wave surface albedo [0,1].
    pub albedo: f32,
    /// Fraction of physically available surface water for evaporation [0,1].
    pub moisture_availability: f32,
    /// Aerodynamic roughness length [m].
    pub roughness_length_meters: f32,
}

impl Default for EnvironmentSurfaceBoundaryDto {
    fn default() -> Self {
        Self {
            cell: WorldCellCoord::default(),
            terrain_elevation_meters: 0.0,
            albedo: 0.18,
            moisture_availability: 0.0,
            roughness_length_meters: 0.1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EnvironmentAtmosphereCellDto {
    pub cell: WorldCellCoord,
    pub surface: EnvironmentSurfaceBoundaryDto,
    pub atmosphere: AtmosphereStateDto,
    pub weather: WeatherStateDto,
    pub clouds: CloudStateDto,
    pub wind: WindStateDto,
}
