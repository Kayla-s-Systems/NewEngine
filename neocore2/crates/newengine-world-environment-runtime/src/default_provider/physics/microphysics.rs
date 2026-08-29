use newengine_world_environment_api::PrecipitationKind;

use super::state::{
    ColumnMemory, MicrophysicsState, PrognosticState, ThermodynamicState, VerticalState,
};
use super::thermodynamics::smoothstep;

pub(super) fn solve(
    thermo: ThermodynamicState,
    vertical: VerticalState,
    prognostic: PrognosticState,
    previous: Option<ColumnMemory>,
) -> MicrophysicsState {
    let target_cwp = (integrated_condensate_kg_m2(&vertical)
        + thermo.supersaturation_condensate_kg_m2)
        .clamp(0.0, 5.0);
    let previous_cwp = previous
        .map(|state| state.cloud_water_path_kg_m2)
        .unwrap_or(target_cwp);
    let dt = prognostic.dt_seconds;

    let autoconversion_flux = precipitation_flux_kg_m2_s(previous_cwp.max(target_cwp), &vertical);
    let cwp = if dt > 0.0 {
        let formation = (target_cwp - previous_cwp).max(0.0) / 120.0;
        let evaporation = (previous_cwp - target_cwp).max(0.0) / 240.0;
        (previous_cwp + dt * (formation - evaporation - autoconversion_flux)).clamp(0.0, 5.0)
    } else {
        target_cwp
    };

    let precipitation_rate_mm_h = (autoconversion_flux * 3600.0).clamp(0.0, 120.0);
    let precipitation_intensity = (precipitation_rate_mm_h / 32.0).clamp(0.0, 1.0);
    let precipitation_kind = if precipitation_rate_mm_h < 0.05 {
        PrecipitationKind::None
    } else if thermo.wet_bulb_c <= 0.8 {
        PrecipitationKind::Snow
    } else {
        PrecipitationKind::Rain
    };

    let rh_critical = 0.72;
    let humidity_cloud_fraction = if thermo.relative_humidity <= rh_critical {
        0.0
    } else {
        let ratio = ((1.0 - thermo.relative_humidity) / (1.0 - rh_critical)).clamp(0.0, 1.0);
        1.0 - ratio.sqrt()
    };
    let condensate_cloud_fraction = 1.0 - (-cwp / 0.22).exp();
    let cloud_coverage =
        (1.0 - (1.0 - humidity_cloud_fraction) * (1.0 - condensate_cloud_fraction)).clamp(0.0, 1.0);
    let overcast = smoothstep(0.72, 0.98, cloud_coverage);

    let mixed_phase_charge = vertical
        .layers
        .iter()
        .filter(|layer| layer.altitude_agl_meters >= 2500.0)
        .map(|layer| {
            let mixed_phase = 1.0 - ((layer.ice_fraction - 0.5).abs() * 2.0).clamp(0.0, 1.0);
            layer.cloud_water_content_g_m3
                * mixed_phase
                * (layer.vertical_velocity_mps / 5.0).clamp(0.0, 1.0)
        })
        .sum::<f32>();
    let instability = smoothstep(250.0, 1800.0, vertical.cape_j_kg);
    let charge = smoothstep(0.03, 0.45, mixed_phase_charge);
    let thunder_probability =
        (instability * charge * smoothstep(0.5, 8.0, precipitation_rate_mm_h)).clamp(0.0, 1.0);

    MicrophysicsState {
        cloud_water_path_kg_m2: cwp,
        cloud_coverage,
        overcast,
        precipitation_kind,
        precipitation_rate_mm_h,
        precipitation_intensity,
        thunder_probability,
    }
}

fn integrated_condensate_kg_m2(vertical: &VerticalState) -> f32 {
    vertical
        .layers
        .windows(2)
        .map(|pair| {
            let dz = pair[1].altitude_agl_meters - pair[0].altitude_agl_meters;
            let mean_lwc_g_m3 =
                0.5 * (pair[0].cloud_water_content_g_m3 + pair[1].cloud_water_content_g_m3);
            mean_lwc_g_m3.max(0.0) * dz.max(0.0) / 1000.0
        })
        .sum::<f32>()
        .clamp(0.0, 5.0)
}

fn precipitation_flux_kg_m2_s(cwp: f32, vertical: &VerticalState) -> f32 {
    let threshold = 0.16;
    let excess = (cwp - threshold).max(0.0);
    if excess <= 0.0 {
        return 0.0;
    }
    let depth = (vertical.cloud_top_m - vertical.layers[0].altitude_agl_meters).max(300.0);
    let depth_efficiency = smoothstep(800.0, 5500.0, depth);
    let convective_efficiency = 0.55 + smoothstep(200.0, 1800.0, vertical.cape_j_kg) * 0.85;
    (excess / 1100.0 * (0.45 + depth_efficiency * 0.55) * convective_efficiency).clamp(0.0, 0.035)
}
