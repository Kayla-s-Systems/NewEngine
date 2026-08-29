use super::{
    state::{
        column_mass_kg_m2, potential_temperature_k, pressure_from_column_mass_hpa,
        temperature_from_potential_c, wind_speed, TransportCell,
    },
    topology::GridTopology,
};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TransportDiagnostics {
    pub substeps: usize,
    pub cfl: f32,
    pub mass_before_kg_m2_sum: f64,
    pub mass_after_kg_m2_sum: f64,
    pub vapor_before_kg_m2_sum: f64,
    pub vapor_after_kg_m2_sum: f64,
    pub cwp_before_kg_m2_sum: f64,
    pub cwp_after_kg_m2_sum: f64,
}

/// First-order upwind finite-volume transport over explicit cardinal faces.
/// All internal face fluxes are applied as equal/opposite pairs, so the closed
/// simulation domain conserves pressure-column mass and transported tracers.
pub(super) fn advect(
    cells: &mut [TransportCell],
    topology: &GridTopology,
    cell_size_m: f32,
    dt_seconds: f32,
) -> TransportDiagnostics {
    if cells.is_empty() || dt_seconds <= 0.0 || cell_size_m <= 1.0 || topology.faces.is_empty() {
        return diagnostics(cells, 0, 0.0, cells);
    }
    let max_speed = cells
        .iter()
        .map(|cell| wind_speed(cell.large_scale_wind))
        .fold(0.0_f32, f32::max);
    let cfl = max_speed * dt_seconds / cell_size_m;
    let substeps = ((cfl / 0.40).ceil() as usize).clamp(1, 512);
    let dt = dt_seconds / substeps as f32;
    let area = cell_size_m * cell_size_m;
    let edge = cell_size_m;

    let before = cells.to_vec();
    let mut mass = cells
        .iter()
        .map(|cell| column_mass_kg_m2(cell.memory.pressure_hpa))
        .collect::<Vec<_>>();
    let mut theta_mass = cells
        .iter()
        .zip(mass.iter())
        .map(|(cell, m)| {
            *m * potential_temperature_k(cell.memory.temperature_c, cell.memory.pressure_hpa)
        })
        .collect::<Vec<_>>();
    let mut vapor_mass = cells
        .iter()
        .zip(mass.iter())
        .map(|(cell, m)| *m * cell.memory.specific_humidity.clamp(0.0, 0.05))
        .collect::<Vec<_>>();
    let mut aerosol_mass = cells
        .iter()
        .zip(mass.iter())
        .map(|(cell, m)| *m * cell.memory.aerosol_mass.clamp(0.0, 2.0))
        .collect::<Vec<_>>();
    let mut cwp = cells
        .iter()
        .map(|cell| cell.memory.cloud_water_path_kg_m2.clamp(0.0, 8.0))
        .collect::<Vec<_>>();

    for _ in 0..substeps {
        let mut dm = vec![0.0_f32; cells.len()];
        let mut dtheta = vec![0.0_f32; cells.len()];
        let mut dq = vec![0.0_f32; cells.len()];
        let mut daerosol = vec![0.0_f32; cells.len()];
        let mut dcwp = vec![0.0_f32; cells.len()];
        for &(a, b, nx, nz) in &topology.faces {
            let face_velocity = 0.5
                * ((cells[a].large_scale_wind.x + cells[b].large_scale_wind.x) * nx as f32
                    + (cells[a].large_scale_wind.z + cells[b].large_scale_wind.z) * nz as f32);
            if face_velocity.abs() <= 1.0e-6 {
                continue;
            }
            let upwind = if face_velocity >= 0.0 { a } else { b };
            let signed_mass_transfer = mass[upwind] * face_velocity * edge * dt / area;
            let max_transfer = mass[upwind] * 0.45;
            let transfer = signed_mass_transfer.clamp(-max_transfer, max_transfer);

            let theta = theta_mass[upwind] / mass[upwind].max(1.0);
            let q = vapor_mass[upwind] / mass[upwind].max(1.0);
            let aerosol = aerosol_mass[upwind] / mass[upwind].max(1.0);
            dm[a] -= transfer;
            dm[b] += transfer;
            dtheta[a] -= transfer * theta;
            dtheta[b] += transfer * theta;
            dq[a] -= transfer * q;
            dq[b] += transfer * q;
            daerosol[a] -= transfer * aerosol;
            daerosol[b] += transfer * aerosol;

            let signed_cwp_transfer = cwp[upwind] * face_velocity * edge * dt / area;
            let max_cwp_transfer = cwp[upwind] * 0.45;
            let cwp_transfer = signed_cwp_transfer.clamp(-max_cwp_transfer, max_cwp_transfer);
            dcwp[a] -= cwp_transfer;
            dcwp[b] += cwp_transfer;
        }
        for i in 0..cells.len() {
            mass[i] = (mass[i] + dm[i]).max(100.0);
            theta_mass[i] = (theta_mass[i] + dtheta[i]).max(100.0 * 170.0);
            vapor_mass[i] = (vapor_mass[i] + dq[i]).max(0.0);
            aerosol_mass[i] = (aerosol_mass[i] + daerosol[i]).max(0.0);
            cwp[i] = (cwp[i] + dcwp[i]).clamp(0.0, 8.0);
        }
    }

    for i in 0..cells.len() {
        let pressure = pressure_from_column_mass_hpa(mass[i]);
        let theta = theta_mass[i] / mass[i].max(1.0);
        cells[i].memory.pressure_hpa = pressure;
        cells[i].memory.temperature_c =
            temperature_from_potential_c(theta, pressure).clamp(-90.0, 65.0);
        cells[i].memory.specific_humidity =
            (vapor_mass[i] / mass[i].max(1.0)).clamp(0.000_001, 0.05);
        cells[i].memory.aerosol_mass = (aerosol_mass[i] / mass[i].max(1.0)).clamp(0.0, 2.0);
        cells[i].memory.cloud_water_path_kg_m2 = cwp[i];
    }
    diagnostics(&before, substeps, cfl, cells)
}

fn diagnostics(
    before: &[TransportCell],
    substeps: usize,
    cfl: f32,
    after: &[TransportCell],
) -> TransportDiagnostics {
    fn totals(cells: &[TransportCell]) -> (f64, f64, f64) {
        cells.iter().fold((0.0, 0.0, 0.0), |acc, cell| {
            let mass = column_mass_kg_m2(cell.memory.pressure_hpa) as f64;
            (
                acc.0 + mass,
                acc.1 + mass * cell.memory.specific_humidity as f64,
                acc.2 + cell.memory.cloud_water_path_kg_m2 as f64,
            )
        })
    }
    let b = totals(before);
    let a = totals(after);
    TransportDiagnostics {
        substeps,
        cfl,
        mass_before_kg_m2_sum: b.0,
        mass_after_kg_m2_sum: a.0,
        vapor_before_kg_m2_sum: b.1,
        vapor_after_kg_m2_sum: a.1,
        cwp_before_kg_m2_sum: b.2,
        cwp_after_kg_m2_sum: a.2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::default_provider::physics::ColumnMemory;
    use newengine_world_api::WorldCellCoord;
    use newengine_world_environment_api::{EnvironmentSurfaceBoundaryDto, Vec3Dto};

    fn cell(x: i32, q: f32, cwp: f32) -> TransportCell {
        let surface = EnvironmentSurfaceBoundaryDto {
            cell: WorldCellCoord::new(x, 0),
            ..EnvironmentSurfaceBoundaryDto::default()
        };
        TransportCell {
            cell: surface.cell,
            surface,
            memory: ColumnMemory {
                world_time_seconds: 0.0,
                pressure_hpa: 1000.0,
                temperature_c: 15.0 + x as f32 * 2.0,
                specific_humidity: q,
                aerosol_mass: 0.1 + x as f32 * 0.05,
                cloud_water_path_kg_m2: cwp,
                precipitation_rate_mm_h: 0.0,
            },
            had_history: true,
            large_scale_wind: Vec3Dto::new(12.0, 0.0, 0.0),
        }
    }

    #[test]
    fn closed_grid_transport_conserves_column_mass_vapor_and_cloud_water() {
        let surfaces = [cell(0, 0.004, 0.1).surface, cell(1, 0.012, 0.8).surface];
        let topology = crate::default_provider::mesoscale::topology::build(&surfaces);
        let mut cells = vec![cell(0, 0.004, 0.1), cell(1, 0.012, 0.8)];
        let d = advect(&mut cells, &topology, 5000.0, 30.0);
        assert!((d.mass_after_kg_m2_sum - d.mass_before_kg_m2_sum).abs() < 0.02);
        assert!((d.vapor_after_kg_m2_sum - d.vapor_before_kg_m2_sum).abs() < 0.002);
        assert!((d.cwp_after_kg_m2_sum - d.cwp_before_kg_m2_sum).abs() < 0.000_2);
        assert!(cells[1].memory.pressure_hpa > 1000.0);
    }
}
