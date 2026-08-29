mod graph;
mod momentum;
mod phenomena;
mod state;
pub(super) mod topology;
mod transport;

use std::collections::BTreeMap;

use newengine_world_environment_api::{
    CloudStateDto, EnvironmentAtmosphereCellDto, EnvironmentFrameDto, EnvironmentFrameRequest,
    Vec3Dto,
};

use super::{
    clouds::cloud_layers,
    observation::{enrich_tags, observe},
    physics::{step as step_column, AtmosphereGraphInput, ColumnMemory},
    surface::{self, SurfaceMemory},
};
use crate::{
    math::clamp01_f32,
    profile_catalog::{AtmosphereProfileDescriptor, EnvironmentProfileDescriptor},
};
use state::TransportCell;

#[derive(Clone, Debug, Default)]
pub(super) struct MesoscaleDiagnostics {
    pub enabled: bool,
    pub cell_count: usize,
    pub duplicate_boundaries: usize,
    pub dt_seconds: f32,
    pub momentum_substeps: usize,
    pub transport_substeps: usize,
    pub transport_cfl: f32,
    pub column_mass_error_kg_m2_sum: f64,
    pub vapor_mass_error_kg_m2_sum: f64,
    pub cwp_error_kg_m2_sum: f64,
    pub max_pressure_accel_m_s2: f32,
    pub max_large_scale_wind_mps: f32,
    pub graph_path: String,
}

#[derive(Clone, Debug, Default)]
pub(super) struct MesoscaleOutput {
    pub cells: Vec<EnvironmentAtmosphereCellDto>,
    pub objects: Vec<newengine_world_environment_api::EnvironmentObjectDto>,
    pub diagnostics: MesoscaleDiagnostics,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn step(
    req: &EnvironmentFrameRequest,
    environment_profile: &EnvironmentProfileDescriptor,
    atmosphere_profile: &AtmosphereProfileDescriptor,
    previous: Option<&EnvironmentFrameDto>,
    world_time_seconds: f64,
    sun_elevation_sine: f32,
    day_blend: f32,
    phase: newengine_world_environment_api::TimeOfDayPhase,
) -> MesoscaleOutput {
    let cell_size_m = req.spatial_cell_size_meters;
    if cell_size_m <= 1.0 || req.surface_boundaries.is_empty() {
        return MesoscaleOutput::default();
    }
    let topology = topology::build(&req.surface_boundaries);
    if topology.surfaces.is_empty() {
        return MesoscaleOutput::default();
    }

    let previous_cells = previous
        .map(|frame| {
            frame
                .spatial_atmosphere
                .iter()
                .map(|cell| (cell.cell, cell))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let frame_dt = previous
        .map(|frame| (world_time_seconds - frame.world_time_seconds) as f32)
        .filter(|dt| dt.is_finite() && *dt > 0.0 && *dt <= 600.0)
        .unwrap_or(0.0);
    let all_cells_have_history = topology
        .surfaces
        .iter()
        .all(|surface| previous_cells.contains_key(&surface.cell));
    let transport_dt = if all_cells_have_history {
        frame_dt
    } else {
        0.0
    };

    let background_wind = Vec3Dto::new(
        atmosphere_profile.geostrophic_wind_x * atmosphere_profile.geostrophic_wind_mps,
        0.0,
        atmosphere_profile.geostrophic_wind_z * atmosphere_profile.geostrophic_wind_mps,
    );
    let mut transport_cells = topology
        .surfaces
        .iter()
        .copied()
        .map(|surface| {
            let previous_cell = previous_cells.get(&surface.cell).copied();
            let memory = previous_cell
                .map(|cell| {
                    ColumnMemory::from_cell(
                        previous
                            .expect("history map requires frame")
                            .world_time_seconds,
                        cell,
                    )
                })
                .unwrap_or_else(|| {
                    baseline_memory(atmosphere_profile, surface, world_time_seconds)
                });
            let large_scale_wind = previous_cell
                .map(free_atmosphere_wind_from_cell)
                .unwrap_or(background_wind);
            TransportCell {
                cell: surface.cell,
                surface,
                memory,
                had_history: previous_cell.is_some(),
                large_scale_wind,
            }
        })
        .collect::<Vec<_>>();

    let momentum = momentum::advance(
        &mut transport_cells,
        &topology,
        cell_size_m,
        transport_dt,
        environment_profile.latitude_degrees,
        background_wind,
        atmosphere_profile.boundary_layer_depth_m,
    );
    let transport = transport::advect(&mut transport_cells, &topology, cell_size_m, transport_dt);

    let cells = transport_cells
        .iter()
        .map(|transport_cell| {
            let previous_cell = previous_cells.get(&transport_cell.cell).copied();
            let graph = step_column(AtmosphereGraphInput {
                profile: atmosphere_profile,
                surface: Some(&transport_cell.surface),
                large_scale_wind: Some(transport_cell.large_scale_wind),
                sun_elevation_sine,
                day_blend,
                world_time_seconds,
                previous: transport_cell.had_history.then_some(transport_cell.memory),
            });
            let atmosphere = graph.atmosphere;
            let observed = observe(
                environment_profile,
                &atmosphere,
                graph.cloud_coverage,
                graph.overcast,
                graph.precipitation_kind,
                graph.precipitation_rate_mm_h,
                graph.precipitation_intensity,
                graph.thunder_probability,
            );
            let mut weather = observed.weather;
            surface::integrate(
                &mut weather,
                &atmosphere,
                &graph.wind,
                sun_elevation_sine,
                graph.evaporation_flux_kg_m2_s,
                previous_cell.map(|cell| {
                    SurfaceMemory::from_cell(
                        previous
                            .expect("history map requires frame")
                            .world_time_seconds,
                        cell,
                    )
                }),
                world_time_seconds,
            );
            enrich_tags(
                &mut weather,
                phase,
                atmosphere.visibility_distance_meters,
                graph.cloud_coverage,
            );
            let clouds = CloudStateDto {
                coverage: graph.cloud_coverage,
                overcast: graph.overcast,
                shadow_strength: clamp01_f32(
                    graph.cloud_coverage * 0.36
                        + graph.overcast * 0.18
                        + atmosphere.cloud_water_path_kg_m2 * 0.16,
                ),
                light_absorption: clamp01_f32(
                    graph.cloud_coverage * 0.20
                        + graph.overcast * 0.18
                        + atmosphere.cloud_water_path_kg_m2 * 0.20
                        + weather.precipitation.intensity * 0.10,
                ),
                layers: cloud_layers(
                    environment_profile,
                    graph.cloud_coverage,
                    graph.overcast,
                    atmosphere.lifting_condensation_level_meters,
                    atmosphere.cloud_water_path_kg_m2,
                    atmosphere.convective_cloud_top_meters,
                    graph.upper_ice_cloud_signal,
                    &atmosphere.vertical_layers,
                    &graph.wind,
                ),
                volumes: Vec::new(),
                storm_cells: Vec::new(),
            };
            EnvironmentAtmosphereCellDto {
                cell: transport_cell.cell,
                surface: transport_cell.surface,
                atmosphere,
                weather,
                clouds,
                wind: graph.wind,
            }
        })
        .collect::<Vec<_>>();

    let objects = phenomena::extract(&cells, &topology, cell_size_m, atmosphere_profile);
    MesoscaleOutput {
        objects,
        diagnostics: MesoscaleDiagnostics {
            enabled: true,
            cell_count: cells.len(),
            duplicate_boundaries: topology.duplicate_boundaries,
            dt_seconds: transport_dt,
            momentum_substeps: momentum.substeps,
            transport_substeps: transport.substeps,
            transport_cfl: transport.cfl,
            column_mass_error_kg_m2_sum: transport.mass_after_kg_m2_sum
                - transport.mass_before_kg_m2_sum,
            vapor_mass_error_kg_m2_sum: transport.vapor_after_kg_m2_sum
                - transport.vapor_before_kg_m2_sum,
            cwp_error_kg_m2_sum: transport.cwp_after_kg_m2_sum - transport.cwp_before_kg_m2_sum,
            max_pressure_accel_m_s2: momentum.max_pressure_accel_m_s2,
            max_large_scale_wind_mps: momentum.max_wind_mps,
            graph_path: graph::diagnostic_path(),
        },
        cells,
    }
}

fn baseline_memory(
    profile: &AtmosphereProfileDescriptor,
    surface: newengine_world_environment_api::EnvironmentSurfaceBoundaryDto,
    world_time_seconds: f64,
) -> ColumnMemory {
    let temperature = profile.mean_temperature_c
        - profile.lapse_rate_k_per_km * surface.terrain_elevation_meters * 0.001;
    let pressure = profile.sea_level_pressure_hpa
        * (-surface.terrain_elevation_meters.clamp(-500.0, 7000.0) / 8434.5).exp();
    ColumnMemory {
        world_time_seconds,
        pressure_hpa: pressure.clamp(500.0, 1080.0),
        temperature_c: temperature.clamp(-80.0, 60.0),
        specific_humidity: (profile.base_specific_humidity_g_per_kg
            * 0.001
            * (-(surface.terrain_elevation_meters - profile.terrain_elevation_m) / 2500.0).exp())
        .clamp(0.000_2, 0.03),
        aerosol_mass: profile.background_aerosol.clamp(0.0, 1.5),
        cloud_water_path_kg_m2: 0.0,
        precipitation_rate_mm_h: 0.0,
    }
}

fn free_atmosphere_wind_from_cell(cell: &EnvironmentAtmosphereCellDto) -> Vec3Dto {
    let cloud = cell.wind.cloud_advection;
    Vec3Dto::new(cloud.x / 0.86, 0.0, cloud.z / 0.86)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_catalog::{atmosphere_profile_by_id, profile_by_id};
    use newengine_world_api::WorldCellCoord;
    use newengine_world_environment_api::EnvironmentSurfaceBoundaryDto;

    #[test]
    fn mesoscale_requires_explicit_world_surface_boundaries() {
        let (environment, _) = profile_by_id("environment.default");
        let atmosphere = atmosphere_profile_by_id(environment.atmosphere_profile_ref);
        let req = EnvironmentFrameRequest {
            spatial_cell_size_meters: 5000.0,
            ..EnvironmentFrameRequest::default()
        };
        let out = step(
            &req,
            environment,
            atmosphere,
            None,
            0.0,
            0.5,
            1.0,
            newengine_world_environment_api::TimeOfDayPhase::Day,
        );
        assert!(out.cells.is_empty());
        assert!(!out.diagnostics.enabled);
    }

    #[test]
    fn terrain_lapse_alone_does_not_create_a_weather_front() {
        let (environment, _) = profile_by_id("environment.default");
        let atmosphere = atmosphere_profile_by_id(environment.atmosphere_profile_ref);
        let req = EnvironmentFrameRequest {
            spatial_cell_size_meters: 5000.0,
            surface_boundaries: vec![
                EnvironmentSurfaceBoundaryDto {
                    cell: WorldCellCoord::new(0, 0),
                    terrain_elevation_meters: 100.0,
                    moisture_availability: atmosphere.surface_moisture_availability,
                    ..EnvironmentSurfaceBoundaryDto::default()
                },
                EnvironmentSurfaceBoundaryDto {
                    cell: WorldCellCoord::new(1, 0),
                    terrain_elevation_meters: 1500.0,
                    moisture_availability: atmosphere.surface_moisture_availability,
                    ..EnvironmentSurfaceBoundaryDto::default()
                },
            ],
            ..EnvironmentFrameRequest::default()
        };
        let out = step(
            &req,
            environment,
            atmosphere,
            None,
            0.0,
            0.5,
            1.0,
            newengine_world_environment_api::TimeOfDayPhase::Day,
        );
        assert!(out.objects.iter().all(|object| object.kind
            != newengine_world_environment_api::EnvironmentObjectKind::WeatherFront));
    }

    #[test]
    fn explicit_surface_cells_produce_one_physical_column_each_without_noise() {
        let (environment, _) = profile_by_id("environment.default");
        let atmosphere = atmosphere_profile_by_id(environment.atmosphere_profile_ref);
        let req = EnvironmentFrameRequest {
            spatial_cell_size_meters: 5000.0,
            surface_boundaries: vec![
                EnvironmentSurfaceBoundaryDto {
                    cell: WorldCellCoord::new(0, 0),
                    terrain_elevation_meters: 100.0,
                    moisture_availability: 0.8,
                    ..EnvironmentSurfaceBoundaryDto::default()
                },
                EnvironmentSurfaceBoundaryDto {
                    cell: WorldCellCoord::new(1, 0),
                    terrain_elevation_meters: 1200.0,
                    moisture_availability: 0.2,
                    ..EnvironmentSurfaceBoundaryDto::default()
                },
            ],
            ..EnvironmentFrameRequest::default()
        };
        let out = step(
            &req,
            environment,
            atmosphere,
            None,
            0.0,
            0.5,
            1.0,
            newengine_world_environment_api::TimeOfDayPhase::Day,
        );
        assert_eq!(out.cells.len(), 2);
        assert!(
            out.cells[1].atmosphere.surface_pressure_hpa
                < out.cells[0].atmosphere.surface_pressure_hpa
        );
    }
}
