use newengine_world_environment_api::Vec3Dto;

use super::{
    state::{wind_speed, TransportCell, GRAVITY_M_S2},
    topology::GridTopology,
};

const EARTH_OMEGA_RAD_S: f32 = 7.292_115e-5;
const VON_KARMAN: f32 = 0.40;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct MomentumDiagnostics {
    pub substeps: usize,
    pub max_pressure_accel_m_s2: f32,
    pub max_wind_mps: f32,
}

pub(super) fn advance(
    cells: &mut [TransportCell],
    topology: &GridTopology,
    cell_size_m: f32,
    dt_seconds: f32,
    latitude_degrees: f32,
    background_geostrophic_wind: Vec3Dto,
    boundary_layer_depth_m: f32,
) -> MomentumDiagnostics {
    if cells.is_empty() || dt_seconds <= 0.0 || cell_size_m <= 1.0 {
        return MomentumDiagnostics::default();
    }
    let substeps = ((dt_seconds / 5.0).ceil() as usize).clamp(1, 120);
    let dt = dt_seconds / substeps as f32;
    let coriolis_f = 2.0 * EARTH_OMEGA_RAD_S * latitude_degrees.to_radians().sin();
    let background_pressure_accel = Vec3Dto::new(
        -coriolis_f * background_geostrophic_wind.z,
        0.0,
        coriolis_f * background_geostrophic_wind.x,
    );
    let mut diagnostics = MomentumDiagnostics {
        substeps,
        ..MomentumDiagnostics::default()
    };

    for _ in 0..substeps {
        let pressure = cells
            .iter()
            .map(reduced_sea_level_pressure_pa)
            .collect::<Vec<_>>();
        let old_wind = cells
            .iter()
            .map(|cell| cell.large_scale_wind)
            .collect::<Vec<_>>();
        for index in 0..cells.len() {
            let grad_x = pressure_gradient_axis(index, 1, 0, &pressure, topology, cell_size_m);
            let grad_z = pressure_gradient_axis(index, 0, 1, &pressure, topology, cell_size_m);
            let state = cells[index].memory;
            let q = state.specific_humidity.clamp(0.0, 0.04);
            let virtual_temperature_k = (state.temperature_c + 273.15) * (1.0 + 0.61 * q);
            let rho = (state.pressure_hpa * 100.0 / (287.05 * virtual_temperature_k.max(180.0)))
                .clamp(0.40, 1.70);
            let pressure_accel = Vec3Dto::new(-grad_x / rho, 0.0, -grad_z / rho);
            diagnostics.max_pressure_accel_m_s2 = diagnostics.max_pressure_accel_m_s2.max(
                (pressure_accel.x * pressure_accel.x + pressure_accel.z * pressure_accel.z).sqrt(),
            );

            let wind = old_wind[index];
            let coriolis = Vec3Dto::new(coriolis_f * wind.z, 0.0, -coriolis_f * wind.x);
            let speed = wind_speed(wind);
            let z0 = cells[index]
                .surface
                .roughness_length_meters
                .clamp(0.001, 3.0);
            let mixing_depth = boundary_layer_depth_m.clamp(100.0, 4000.0);
            let reference_height = (mixing_depth * 0.35).clamp(30.0, 500.0);
            let log_ratio = (reference_height / z0).ln().max(1.2);
            let drag_coefficient = (VON_KARMAN / log_ratio).powi(2).clamp(0.000_2, 0.025);
            let drag_scale = drag_coefficient * speed / mixing_depth;
            let drag = Vec3Dto::new(-drag_scale * wind.x, 0.0, -drag_scale * wind.z);

            let ax = pressure_accel.x + background_pressure_accel.x + coriolis.x + drag.x;
            let az = pressure_accel.z + background_pressure_accel.z + coriolis.z + drag.z;
            let next = Vec3Dto::new(wind.x + ax * dt, 0.0, wind.z + az * dt);
            let next_speed = wind_speed(next);
            cells[index].large_scale_wind = if next_speed > 80.0 {
                Vec3Dto::new(next.x * 80.0 / next_speed, 0.0, next.z * 80.0 / next_speed)
            } else {
                next
            };
            diagnostics.max_wind_mps = diagnostics
                .max_wind_mps
                .max(wind_speed(cells[index].large_scale_wind));
        }
    }
    diagnostics
}

fn reduced_sea_level_pressure_pa(cell: &TransportCell) -> f32 {
    let q = cell.memory.specific_humidity.clamp(0.0, 0.04);
    let virtual_temperature_k = (cell.memory.temperature_c + 273.15) * (1.0 + 0.61 * q);
    let scale_height = 287.05 * virtual_temperature_k.max(190.0) / GRAVITY_M_S2;
    cell.memory.pressure_hpa
        * 100.0
        * (cell.surface.terrain_elevation_meters.clamp(-500.0, 7000.0) / scale_height.max(4500.0))
            .exp()
}

fn pressure_gradient_axis(
    index: usize,
    dx: i32,
    dz: i32,
    pressure_pa: &[f32],
    topology: &GridTopology,
    cell_size_m: f32,
) -> f32 {
    let cell = topology.surfaces[index].cell;
    let minus = newengine_world_api::WorldCellCoord::new(cell.x - dx, cell.z - dz);
    let plus = newengine_world_api::WorldCellCoord::new(cell.x + dx, cell.z + dz);
    match (
        topology.index_by_cell.get(&minus),
        topology.index_by_cell.get(&plus),
    ) {
        (Some(&a), Some(&b)) => (pressure_pa[b] - pressure_pa[a]) / (2.0 * cell_size_m),
        (None, Some(&b)) => (pressure_pa[b] - pressure_pa[index]) / cell_size_m,
        (Some(&a), None) => (pressure_pa[index] - pressure_pa[a]) / cell_size_m,
        (None, None) => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::default_provider::physics::ColumnMemory;
    use newengine_world_api::WorldCellCoord;
    use newengine_world_environment_api::EnvironmentSurfaceBoundaryDto;

    fn cell(x: i32, pressure: f32) -> TransportCell {
        let surface = EnvironmentSurfaceBoundaryDto {
            cell: WorldCellCoord::new(x, 0),
            ..EnvironmentSurfaceBoundaryDto::default()
        };
        TransportCell {
            cell: surface.cell,
            surface,
            memory: ColumnMemory {
                world_time_seconds: 0.0,
                pressure_hpa: pressure,
                temperature_c: 15.0,
                specific_humidity: 0.007,
                aerosol_mass: 0.08,
                cloud_water_path_kg_m2: 0.0,
                precipitation_rate_mm_h: 0.0,
            },
            had_history: true,
            large_scale_wind: Vec3Dto::zero(),
        }
    }

    #[test]
    fn pressure_gradient_accelerates_air_from_high_toward_low_pressure() {
        let surfaces = [cell(0, 1018.0).surface, cell(1, 1008.0).surface];
        let topology = crate::default_provider::mesoscale::topology::build(&surfaces);
        let mut cells = vec![cell(0, 1018.0), cell(1, 1008.0)];
        advance(
            &mut cells,
            &topology,
            10_000.0,
            30.0,
            0.0,
            Vec3Dto::zero(),
            1000.0,
        );
        assert!(cells[0].large_scale_wind.x > 0.0);
        assert!(cells[1].large_scale_wind.x > 0.0);
    }
}
