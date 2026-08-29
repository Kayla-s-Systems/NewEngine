use crate::profile_catalog::AtmosphereProfileDescriptor;
use newengine_world_environment_api::{EnvironmentSurfaceBoundaryDto, Vec3Dto};

use super::state::{BoundaryState, ColumnMemory};
use super::thermodynamics::{
    saturation_vapor_pressure_hpa, vapor_pressure_to_specific_humidity, GRAVITY, RD_AIR,
};

const SOLAR_CONSTANT_W_M2: f32 = 1361.0;
const STEFAN_BOLTZMANN: f32 = 5.670_374_4e-8;

pub(super) fn resolve(
    profile: &AtmosphereProfileDescriptor,
    surface: Option<&EnvironmentSurfaceBoundaryDto>,
    large_scale_wind: Option<Vec3Dto>,
    sun_elevation_sine: f32,
    previous: Option<ColumnMemory>,
    world_time_seconds: f64,
) -> BoundaryState {
    let dt = previous
        .map(|state| (world_time_seconds - state.world_time_seconds) as f32)
        .filter(|dt| dt.is_finite() && *dt > 0.0 && *dt <= 600.0)
        .unwrap_or(0.0);
    let terrain_elevation_m = surface
        .map(|it| it.terrain_elevation_meters)
        .unwrap_or(profile.terrain_elevation_m)
        .clamp(-500.0, 7000.0);
    let surface_albedo = surface
        .map(|it| it.albedo)
        .unwrap_or(profile.surface_albedo)
        .clamp(0.02, 0.95);
    let surface_moisture = surface
        .map(|it| it.moisture_availability)
        .unwrap_or(profile.surface_moisture_availability)
        .clamp(0.0, 1.0);
    let surface_roughness = surface
        .map(|it| it.roughness_length_meters)
        .unwrap_or(profile.surface_roughness_m)
        .clamp(0.001, 3.0);
    let local_climate_temperature =
        profile.mean_temperature_c - profile.lapse_rate_k_per_km * terrain_elevation_m * 0.001;
    // Climate q is defined at the profile reference elevation. A world cell at a
    // different height receives the same air mass through a bounded tropospheric
    // moisture profile; the atmosphere never teleports lowland q to a mountain cell.
    let elevation_delta_m = terrain_elevation_m - profile.terrain_elevation_m;
    let equilibrium_specific_humidity =
        (profile.base_specific_humidity_g_per_kg * 0.001 * (-elevation_delta_m / 2500.0).exp())
            .clamp(0.000_2, 0.03);
    let previous_temperature = previous
        .map(|state| state.temperature_c)
        .unwrap_or(local_climate_temperature);
    let previous_q = previous
        .map(|state| state.specific_humidity)
        .unwrap_or(equilibrium_specific_humidity);
    let previous_cwp = previous
        .map(|state| state.cloud_water_path_kg_m2)
        .unwrap_or(0.0);

    let virtual_temperature_k = (previous_temperature + 273.15) * (1.0 + 0.61 * previous_q);
    let scale_height = RD_AIR * virtual_temperature_k.max(200.0) / GRAVITY;
    let pressure_target =
        profile.sea_level_pressure_hpa * (-terrain_elevation_m / scale_height.max(5000.0)).exp();

    let solar_mu = sun_elevation_sine.max(0.0);
    let clear_sky_transmission = 0.74_f32;
    let cloud_transmission = (-0.62 * previous_cwp.clamp(0.0, 4.0)).exp();
    let shortwave = SOLAR_CONSTANT_W_M2
        * solar_mu
        * clear_sky_transmission
        * cloud_transmission
        * (1.0 - surface_albedo);

    let temperature_k = (previous_temperature + 273.15).clamp(190.0, 330.0);
    let vapor_pressure = previous
        .map(|state| {
            super::thermodynamics::specific_humidity_to_vapor_pressure(
                state.specific_humidity,
                state.pressure_hpa,
            )
        })
        .unwrap_or_else(|| {
            let saturation = saturation_vapor_pressure_hpa(previous_temperature);
            let qsat = vapor_pressure_to_specific_humidity(saturation, pressure_target);
            let rh = (previous_q / qsat.max(0.000_001)).clamp(0.05, 1.0);
            saturation * rh
        });
    let sky_emissivity = (0.60 + 0.055 * vapor_pressure.max(0.0).sqrt()).clamp(0.58, 0.96);
    let longwave_up = 0.96 * STEFAN_BOLTZMANN * temperature_k.powi(4);
    let longwave_down = sky_emissivity * STEFAN_BOLTZMANN * temperature_k.powi(4);
    let net_radiative_flux = (shortwave + longwave_down - longwave_up).clamp(-260.0, 980.0);

    let profile_wind = Vec3Dto::new(
        profile.geostrophic_wind_x * profile.geostrophic_wind_mps,
        0.0,
        profile.geostrophic_wind_z * profile.geostrophic_wind_mps,
    );
    let resolved_large_scale_wind = large_scale_wind.unwrap_or(profile_wind);
    let geostrophic_wind_mps = (resolved_large_scale_wind.x * resolved_large_scale_wind.x
        + resolved_large_scale_wind.z * resolved_large_scale_wind.z)
        .sqrt()
        .clamp(0.0, 60.0);
    let geostrophic_wind = if geostrophic_wind_mps > 0.000_1 {
        Vec3Dto::new(
            resolved_large_scale_wind.x / geostrophic_wind_mps,
            0.0,
            resolved_large_scale_wind.z / geostrophic_wind_mps,
        )
    } else {
        Vec3Dto::new(1.0, 0.0, 0.0)
    };
    let saturation = saturation_vapor_pressure_hpa(previous_temperature);
    let q_sat = vapor_pressure_to_specific_humidity(saturation, pressure_target);
    let humidity_deficit = (q_sat - previous_q).max(0.0);
    let rho_air = pressure_target * 100.0 / (RD_AIR * virtual_temperature_k.max(200.0));
    let transfer_coefficient = 0.0013;
    let evaporation_flux = (rho_air
        * transfer_coefficient
        * geostrophic_wind_mps.max(0.2)
        * humidity_deficit
        * surface_moisture)
        .clamp(0.0, 0.000_35);

    // Mineral aerosol emission requires both an erodible dry surface and enough wind stress.
    let dryness = 1.0 - surface_moisture;
    let threshold_mps = 7.0 + surface_roughness.min(1.0) * 4.0;
    let wind_excess = (geostrophic_wind_mps - threshold_mps).max(0.0);
    let aerosol_emission = (dryness * wind_excess * wind_excess * 0.000_000_8).clamp(0.0, 0.000_08);

    BoundaryState {
        dt_seconds: dt,
        local_climate_temperature_c: local_climate_temperature,
        pressure_target_hpa: pressure_target,
        equilibrium_specific_humidity,
        net_radiative_flux_w_m2: net_radiative_flux,
        evaporation_flux_kg_m2_s: evaporation_flux,
        aerosol_emission_per_s: aerosol_emission,
        boundary_layer_depth_m: profile.boundary_layer_depth_m.clamp(250.0, 3500.0),
        boundary_layer_heat_capacity_j_m2_k: profile
            .boundary_layer_heat_capacity_j_m2_k
            .clamp(250_000.0, 8_000_000.0),
        geostrophic_wind,
        geostrophic_wind_mps: geostrophic_wind_mps.clamp(0.0, 60.0),
        surface_roughness_m: surface_roughness,
    }
}
