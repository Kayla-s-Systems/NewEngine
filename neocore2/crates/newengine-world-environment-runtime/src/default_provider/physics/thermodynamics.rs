use super::state::{BoundaryState, PrognosticState, ThermodynamicState};

pub(super) const RD_AIR: f32 = 287.05;
pub(super) const EPSILON: f32 = 0.622;
pub(super) const GRAVITY: f32 = 9.80665;

#[inline]
pub(super) fn saturation_vapor_pressure_hpa(temperature_c: f32) -> f32 {
    6.112 * ((17.67 * temperature_c) / (temperature_c + 243.5)).exp()
}

#[inline]
pub(super) fn specific_humidity_to_vapor_pressure(q: f32, pressure_hpa: f32) -> f32 {
    q * pressure_hpa / (EPSILON + (1.0 - EPSILON) * q)
}

#[inline]
pub(super) fn vapor_pressure_to_specific_humidity(e_hpa: f32, pressure_hpa: f32) -> f32 {
    let e = e_hpa.clamp(0.0, pressure_hpa * 0.95);
    EPSILON * e / (pressure_hpa - (1.0 - EPSILON) * e).max(0.01)
}

#[inline]
pub(super) fn dew_point_celsius(vapor_pressure_hpa: f32) -> f32 {
    let ln_ratio = (vapor_pressure_hpa.max(0.02) / 6.112).ln();
    (243.5 * ln_ratio / (17.67 - ln_ratio)).clamp(-80.0, 55.0)
}

pub(super) fn diagnose(state: PrognosticState, boundary: BoundaryState) -> ThermodynamicState {
    let saturation = saturation_vapor_pressure_hpa(state.temperature_c);
    let q_sat = vapor_pressure_to_specific_humidity(saturation, state.pressure_hpa);
    let excess_q = (state.specific_humidity - q_sat).max(0.0);
    let q = state.specific_humidity.min(q_sat).clamp(0.000_001, 0.04);
    let vapor_pressure = specific_humidity_to_vapor_pressure(q, state.pressure_hpa);
    let relative_humidity = (vapor_pressure / saturation.max(0.05)).clamp(0.0, 1.0);
    let dew_point = dew_point_celsius(vapor_pressure);
    let lcl = (125.0 * (state.temperature_c - dew_point).max(0.0)).clamp(50.0, 5000.0);
    let virtual_temperature_k = (state.temperature_c + 273.15) * (1.0 + 0.61 * q);
    let density = (state.pressure_hpa * 100.0 / (RD_AIR * virtual_temperature_k.max(180.0)))
        .clamp(0.45, 1.65);
    let precipitable_water = (q * state.pressure_hpa * 100.0 / GRAVITY * 0.35).clamp(0.0, 90.0);
    let supersaturation_condensate =
        (excess_q * density * boundary.boundary_layer_depth_m).clamp(0.0, 4.0);
    let wet_bulb = stull_wet_bulb_celsius(state.temperature_c, relative_humidity);

    ThermodynamicState {
        pressure_hpa: state.pressure_hpa,
        temperature_c: state.temperature_c,
        specific_humidity: q,
        vapor_pressure_hpa: vapor_pressure,
        saturation_vapor_pressure_hpa: saturation,
        relative_humidity,
        dew_point_c: dew_point,
        wet_bulb_c: wet_bulb,
        air_density_kg_m3: density,
        lcl_m: lcl,
        precipitable_water_mm: precipitable_water,
        supersaturation_condensate_kg_m2: supersaturation_condensate,
    }
}

fn stull_wet_bulb_celsius(temperature_c: f32, relative_humidity: f32) -> f32 {
    let rh = (relative_humidity * 100.0).clamp(1.0, 100.0);
    temperature_c * (0.151_977 * (rh + 8.313_659).sqrt()).atan() + (temperature_c + rh).atan()
        - (rh - 1.676_331).atan()
        + 0.003_918_38 * rh.powf(1.5) * (0.023_101 * rh).atan()
        - 4.686_035
}

#[inline]
pub(super) fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < 1.0e-6 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub(super) fn exp_relax(current: f32, target: f32, dt: f32, tau: f32) -> f32 {
    let alpha = 1.0 - (-dt.max(0.0) / tau.max(0.001)).exp();
    current + (target - current) * alpha.clamp(0.0, 1.0)
}
