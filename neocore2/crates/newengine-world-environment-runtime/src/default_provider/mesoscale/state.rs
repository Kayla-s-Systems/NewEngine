use newengine_world_api::WorldCellCoord;
use newengine_world_environment_api::{EnvironmentSurfaceBoundaryDto, Vec3Dto};

use super::super::physics::ColumnMemory;

pub(super) const GRAVITY_M_S2: f32 = 9.80665;
pub(super) const KAPPA_DRY_AIR: f32 = 0.2854;

#[derive(Clone, Copy, Debug)]
pub(super) struct TransportCell {
    pub cell: WorldCellCoord,
    pub surface: EnvironmentSurfaceBoundaryDto,
    pub memory: ColumnMemory,
    pub had_history: bool,
    /// Free-atmosphere horizontal velocity [m/s], not the roughness-reduced 10 m wind.
    pub large_scale_wind: Vec3Dto,
}

#[inline]
pub(super) fn column_mass_kg_m2(pressure_hpa: f32) -> f32 {
    (pressure_hpa.max(50.0) * 100.0 / GRAVITY_M_S2).max(1.0)
}

#[inline]
pub(super) fn pressure_from_column_mass_hpa(mass_kg_m2: f32) -> f32 {
    (mass_kg_m2.max(1.0) * GRAVITY_M_S2 / 100.0).clamp(50.0, 1100.0)
}

#[inline]
pub(super) fn potential_temperature_k(temperature_c: f32, pressure_hpa: f32) -> f32 {
    (temperature_c + 273.15).clamp(170.0, 340.0)
        * (1000.0 / pressure_hpa.max(50.0)).powf(KAPPA_DRY_AIR)
}

#[inline]
pub(super) fn temperature_from_potential_c(theta_k: f32, pressure_hpa: f32) -> f32 {
    theta_k * (pressure_hpa.max(50.0) / 1000.0).powf(KAPPA_DRY_AIR) - 273.15
}

#[inline]
pub(super) fn wind_speed(wind: Vec3Dto) -> f32 {
    (wind.x * wind.x + wind.z * wind.z).sqrt()
}
