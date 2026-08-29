mod boundary;
pub(super) mod graph;
mod microphysics;
mod optics;
mod prognostic;
mod state;
mod thermodynamics;
mod vertical;
mod wind;

use crate::profile_catalog::AtmosphereProfileDescriptor;
use newengine_world_environment_api::{
    AtmosphereStateDto, Color3Dto, EnvironmentSurfaceBoundaryDto, PrecipitationKind, Vec3Dto,
    WindStateDto,
};
pub(super) use state::ColumnMemory;

pub(super) struct AtmosphereGraphInput<'a> {
    pub profile: &'a AtmosphereProfileDescriptor,
    pub surface: Option<&'a EnvironmentSurfaceBoundaryDto>,
    /// Free-atmosphere wind supplied by mesoscale momentum. None uses the climate boundary.
    pub large_scale_wind: Option<Vec3Dto>,
    pub sun_elevation_sine: f32,
    pub day_blend: f32,
    pub world_time_seconds: f64,
    pub previous: Option<ColumnMemory>,
}

#[derive(Clone, Debug)]
pub(super) struct AtmosphereGraphOutput {
    pub atmosphere: AtmosphereStateDto,
    pub wind: WindStateDto,
    pub cloud_coverage: f32,
    pub overcast: f32,
    pub precipitation_kind: PrecipitationKind,
    pub precipitation_rate_mm_h: f32,
    pub precipitation_intensity: f32,
    pub thunder_probability: f32,
    pub upper_ice_cloud_signal: f32,
    pub net_radiative_flux_w_m2: f32,
    pub evaporation_flux_kg_m2_s: f32,
}

/// Executes the atmospheric DAG exactly once in dependency order:
/// boundary -> prognostic -> thermodynamics -> vertical dynamics -> microphysics
/// -> wind -> aerosol/optics. No node samples RNG or reads renderer state.
pub(super) fn step(input: AtmosphereGraphInput<'_>) -> AtmosphereGraphOutput {
    let boundary = boundary::resolve(
        input.profile,
        input.surface,
        input.large_scale_wind,
        input.sun_elevation_sine,
        input.previous,
        input.world_time_seconds,
    );
    let prognostic = prognostic::integrate(input.profile, boundary, input.previous);
    let thermo = thermodynamics::diagnose(prognostic, boundary);
    let vertical = vertical::solve(input.profile, thermo);
    let micro = microphysics::solve(thermo, vertical, prognostic, input.previous);
    let wind = wind::solve(boundary, vertical, input.sun_elevation_sine);
    let optical = optics::solve(
        prognostic,
        thermo,
        micro,
        boundary,
        wind.global_speed_mps,
        input.sun_elevation_sine,
    );

    let haze = optical.haze_amount;
    let atmosphere = AtmosphereStateDto {
        fog_density: optical.fog_density,
        fog_height_falloff: (0.07 + thermo.relative_humidity * 0.20).clamp(0.05, 0.38),
        fog_color_linear: Color3Dto::new(
            0.40 + input.day_blend * 0.16 + haze * 0.08,
            0.45 + input.day_blend * 0.17 + haze * 0.06,
            0.53 + input.day_blend * 0.15 + haze * 0.04,
        ),
        haze_amount: haze,
        humidity: thermo.relative_humidity,
        aerosol_density: optical.aerosol_density,
        visibility_distance_meters: optical.visibility_m,
        surface_pressure_hpa: thermo.pressure_hpa,
        temperature_celsius: thermo.temperature_c,
        dew_point_celsius: thermo.dew_point_c,
        specific_humidity_g_per_kg: thermo.specific_humidity * 1000.0,
        vapor_pressure_hpa: thermo.vapor_pressure_hpa,
        saturation_vapor_pressure_hpa: thermo.saturation_vapor_pressure_hpa,
        air_density_kg_m3: thermo.air_density_kg_m3,
        lifting_condensation_level_meters: thermo.lcl_m,
        precipitable_water_mm: thermo.precipitable_water_mm,
        cloud_water_path_kg_m2: micro.cloud_water_path_kg_m2,
        condensation_potential: micro.cloud_coverage,
        vertical_layers: vertical.layers,
        cape_j_per_kg: vertical.cape_j_kg,
        cin_j_per_kg: vertical.cin_j_kg,
        convective_cloud_top_meters: vertical.cloud_top_m,
    };

    AtmosphereGraphOutput {
        atmosphere,
        wind,
        cloud_coverage: micro.cloud_coverage,
        overcast: micro.overcast,
        precipitation_kind: micro.precipitation_kind,
        precipitation_rate_mm_h: micro.precipitation_rate_mm_h,
        precipitation_intensity: micro.precipitation_intensity,
        thunder_probability: micro.thunder_probability,
        upper_ice_cloud_signal: vertical.upper_ice_transport,
        net_radiative_flux_w_m2: boundary.net_radiative_flux_w_m2,
        evaporation_flux_kg_m2_s: boundary.evaporation_flux_kg_m2_s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_catalog::{atmosphere_profile_by_id, profile_by_id};

    #[test]
    fn graph_is_deterministic_without_seed_or_random_forcing() {
        let (environment, _) = profile_by_id("environment.game_ready_forest_road");
        let profile = atmosphere_profile_by_id(environment.atmosphere_profile_ref);
        let a = step(AtmosphereGraphInput {
            profile,
            surface: None,
            large_scale_wind: None,
            sun_elevation_sine: 0.72,
            day_blend: 1.0,
            world_time_seconds: 1000.0,
            previous: None,
        });
        let b = step(AtmosphereGraphInput {
            profile,
            surface: None,
            large_scale_wind: None,
            sun_elevation_sine: 0.72,
            day_blend: 1.0,
            world_time_seconds: 1000.0,
            previous: None,
        });
        assert_eq!(a.atmosphere, b.atmosphere);
        assert_eq!(a.wind, b.wind);
        assert_eq!(a.precipitation_rate_mm_h, b.precipitation_rate_mm_h);
    }

    #[test]
    fn one_second_step_cannot_teleport_thermodynamic_state() {
        let (environment, _) = profile_by_id("environment.game_ready_forest_road");
        let profile = atmosphere_profile_by_id(environment.atmosphere_profile_ref);
        let first = step(AtmosphereGraphInput {
            profile,
            surface: None,
            large_scale_wind: None,
            sun_elevation_sine: 0.0,
            day_blend: 0.0,
            world_time_seconds: 1000.0,
            previous: None,
        });
        let memory = ColumnMemory {
            world_time_seconds: 1000.0,
            pressure_hpa: first.atmosphere.surface_pressure_hpa,
            temperature_c: first.atmosphere.temperature_celsius,
            specific_humidity: first.atmosphere.specific_humidity_g_per_kg * 0.001,
            aerosol_mass: first.atmosphere.aerosol_density,
            cloud_water_path_kg_m2: first.atmosphere.cloud_water_path_kg_m2,
            precipitation_rate_mm_h: first.precipitation_rate_mm_h,
        };
        let second = step(AtmosphereGraphInput {
            profile,
            surface: None,
            large_scale_wind: None,
            sun_elevation_sine: 1.0,
            day_blend: 1.0,
            world_time_seconds: 1001.0,
            previous: Some(memory),
        });
        assert!(
            (second.atmosphere.temperature_celsius - first.atmosphere.temperature_celsius).abs()
                < 0.05
        );
        assert!(
            (second.atmosphere.specific_humidity_g_per_kg
                - first.atmosphere.specific_humidity_g_per_kg)
                .abs()
                < 0.05
        );
    }
}
