use crate::profile_catalog::AtmosphereProfileDescriptor;

use super::state::{BoundaryState, ColumnMemory, PrognosticState};
use super::thermodynamics::{exp_relax, RD_AIR};

const LATENT_HEAT_VAPORIZATION_J_KG: f32 = 2_450_000.0;

pub(super) fn integrate(
    profile: &AtmosphereProfileDescriptor,
    boundary: BoundaryState,
    previous: Option<ColumnMemory>,
) -> PrognosticState {
    let initial_q = boundary.equilibrium_specific_humidity;
    let Some(previous) = previous.filter(|_| boundary.dt_seconds > 0.0) else {
        return PrognosticState {
            dt_seconds: 0.0,
            pressure_hpa: boundary.pressure_target_hpa,
            temperature_c: boundary.local_climate_temperature_c,
            specific_humidity: initial_q,
            aerosol_mass: profile.background_aerosol.clamp(0.0, 1.5),
        };
    };

    let dt = boundary.dt_seconds;
    let a = previous;
    let previous_q = a.specific_humidity.clamp(0.000_001, 0.04);
    let virtual_temperature_k = (a.temperature_c + 273.15) * (1.0 + 0.61 * previous_q);
    let rho_air =
        (a.pressure_hpa * 100.0 / (RD_AIR * virtual_temperature_k.max(190.0))).clamp(0.45, 1.65);
    let boundary_layer_air_mass = (rho_air * boundary.boundary_layer_depth_m).max(80.0);

    let radiative_temperature_tendency =
        boundary.net_radiative_flux_w_m2 / boundary.boundary_layer_heat_capacity_j_m2_k;
    let latent_cooling_tendency = boundary.evaporation_flux_kg_m2_s * LATENT_HEAT_VAPORIZATION_J_KG
        / boundary.boundary_layer_heat_capacity_j_m2_k;
    let climate_restoring_tendency =
        (boundary.local_climate_temperature_c - a.temperature_c) / (36.0 * 3600.0);
    let temperature = (a.temperature_c
        + dt * (radiative_temperature_tendency - latent_cooling_tendency
            + climate_restoring_tendency))
        .clamp(-75.0, 58.0);

    let evaporation_q_tendency = boundary.evaporation_flux_kg_m2_s / boundary_layer_air_mass;
    let previous_precip_flux = a.precipitation_rate_mm_h.max(0.0) / 3600.0;
    let precipitation_q_sink = previous_precip_flux / boundary_layer_air_mass * 0.35;
    let free_troposphere_exchange = (initial_q - previous_q) / (18.0 * 3600.0);
    let q = (previous_q
        + dt * (evaporation_q_tendency - precipitation_q_sink + free_troposphere_exchange))
        .clamp(0.000_001, 0.04);

    let pressure = exp_relax(
        a.pressure_hpa,
        boundary.pressure_target_hpa,
        dt,
        3.0 * 3600.0,
    )
    .clamp(650.0, 1065.0);

    let dry_deposition = a.aerosol_mass.max(0.0) / (24.0 * 3600.0);
    let rain_washout = a.aerosol_mass.max(0.0) * previous_precip_flux * 0.018;
    let background_restoring = (profile.background_aerosol - a.aerosol_mass) / (12.0 * 3600.0);
    let aerosol = (a.aerosol_mass
        + dt * (boundary.aerosol_emission_per_s + background_restoring
            - dry_deposition
            - rain_washout))
        .clamp(0.0, 1.5);

    PrognosticState {
        dt_seconds: dt,
        pressure_hpa: pressure,
        temperature_c: temperature,
        specific_humidity: q,
        aerosol_mass: aerosol,
    }
}
