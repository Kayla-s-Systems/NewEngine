use crate::profile_catalog::AtmosphereProfileDescriptor;
use newengine_world_environment_api::AtmosphericLayerDto;

use super::state::{ThermodynamicState, VerticalState};
use super::thermodynamics::{
    saturation_vapor_pressure_hpa, smoothstep, vapor_pressure_to_specific_humidity, GRAVITY, RD_AIR,
};

const LEVELS_M: [f32; 5] = [0.0, 1000.0, 2500.0, 5000.0, 9000.0];

pub(super) fn solve(
    profile: &AtmosphereProfileDescriptor,
    thermo: ThermodynamicState,
) -> VerticalState {
    let mut layers = [AtmosphericLayerDto::default(); 5];
    let mut cape = 0.0_f32;
    let mut cin = 0.0_f32;
    let mut previous_buoyancy = 0.0_f32;
    let mut previous_z = 0.0_f32;
    let mut accumulated_positive_energy = 0.0_f32;
    let mut cloud_top = thermo.lcl_m;
    let mut upper_ice_transport = 0.0_f32;

    for (index, z) in LEVELS_M.into_iter().enumerate() {
        let z_km = z * 0.001;
        let pressure = thermo.pressure_hpa * (-z / 8434.5).exp();
        let env_temperature = thermo.temperature_c - profile.lapse_rate_k_per_km * z_km;
        let environmental_q_total =
            (thermo.specific_humidity * (-z / 2350.0).exp()).clamp(0.000_001, 0.04);
        let env_qsat = vapor_pressure_to_specific_humidity(
            saturation_vapor_pressure_hpa(env_temperature),
            pressure,
        );
        let environmental_condensed_q = (environmental_q_total - env_qsat).max(0.0);
        let env_q = environmental_q_total.min(env_qsat);
        let env_rh = (environmental_q_total / env_qsat.max(0.000_001)).clamp(0.0, 1.0);

        let lcl_km = thermo.lcl_m * 0.001;
        let parcel_temperature = if z_km <= lcl_km {
            thermo.temperature_c - 9.8 * z_km
        } else {
            thermo.temperature_c - 9.8 * lcl_km - 6.0 * (z_km - lcl_km)
        };
        let parcel_qsat = vapor_pressure_to_specific_humidity(
            saturation_vapor_pressure_hpa(parcel_temperature),
            pressure,
        );
        let parcel_q = if z <= thermo.lcl_m {
            thermo.specific_humidity
        } else {
            thermo.specific_humidity.min(parcel_qsat)
        };
        let parcel_condensed_q = if z > thermo.lcl_m {
            (thermo.specific_humidity - parcel_qsat).max(0.0)
        } else {
            0.0
        };

        let env_virtual_k = (env_temperature + 273.15) * (1.0 + 0.61 * env_q);
        let parcel_virtual_k = (parcel_temperature + 273.15) * (1.0 + 0.61 * parcel_q);
        let buoyancy = GRAVITY * (parcel_virtual_k - env_virtual_k) / env_virtual_k.max(180.0);
        if index > 0 {
            let dz = z - previous_z;
            let layer_energy = 0.5 * (previous_buoyancy + buoyancy) * dz;
            if layer_energy > 0.0 {
                cape += layer_energy;
                accumulated_positive_energy += layer_energy;
            } else if accumulated_positive_energy < 25.0 {
                cin += -layer_energy;
            }
        }

        let convective_velocity = (2.0 * accumulated_positive_energy.max(0.0)).sqrt();
        let updraft = if buoyancy > 0.0 {
            (convective_velocity * 0.35).clamp(0.0, 22.0)
        } else {
            (buoyancy * 2.5).clamp(-4.0, 0.0)
        };
        let density = pressure * 100.0 / (RD_AIR * env_virtual_k.max(180.0));
        let convective_transport = if z > thermo.lcl_m {
            smoothstep(0.15, 2.0, updraft)
        } else {
            0.0
        };
        let stratiform_condensate = environmental_condensed_q * density * 1000.0;
        let convective_condensate = parcel_condensed_q * density * 1000.0 * convective_transport;
        let cloud_water = (stratiform_condensate + convective_condensate).clamp(0.0, 5.0);
        let ice_fraction = smoothstep(0.0, -22.0, env_temperature);

        if cloud_water > 0.012 && (environmental_condensed_q > 0.0 || updraft > 0.15) {
            cloud_top = z.max(cloud_top);
        }
        if z >= 5000.0 {
            let level_weight = if z < 7000.0 { 0.68 } else { 0.32 };
            upper_ice_transport +=
                cloud_water * ice_fraction * (updraft / 2.5).clamp(0.0, 1.0) * level_weight;
        }

        layers[index] = AtmosphericLayerDto {
            altitude_agl_meters: z,
            pressure_hpa: pressure,
            temperature_celsius: env_temperature,
            relative_humidity: env_rh,
            specific_humidity_g_per_kg: env_q * 1000.0,
            cloud_water_content_g_m3: cloud_water,
            ice_fraction,
            vertical_velocity_mps: updraft,
        };
        previous_buoyancy = buoyancy;
        previous_z = z;
    }

    VerticalState {
        layers,
        cape_j_kg: cape.clamp(0.0, 7000.0),
        cin_j_kg: cin.clamp(0.0, 1500.0),
        cloud_top_m: cloud_top.clamp(thermo.lcl_m.max(50.0), 12_000.0),
        upper_ice_transport: (upper_ice_transport / 0.35).clamp(0.0, 1.0),
    }
}
