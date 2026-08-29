use super::state::{
    BoundaryState, MicrophysicsState, OpticalState, PrognosticState, ThermodynamicState,
};
use super::thermodynamics::smoothstep;

pub(super) fn solve(
    prognostic: PrognosticState,
    thermo: ThermodynamicState,
    micro: MicrophysicsState,
    boundary: BoundaryState,
    surface_wind_mps: f32,
    sun_elevation_sine: f32,
) -> OpticalState {
    let washout = if prognostic.dt_seconds > 0.0 {
        (-micro.precipitation_rate_mm_h * prognostic.dt_seconds / 18_000.0).exp()
    } else {
        1.0
    };
    let aerosol = (prognostic.aerosol_mass * washout).clamp(0.0, 1.5);
    let hygroscopic_growth = 1.0 + smoothstep(0.70, 0.98, thermo.relative_humidity) * 0.85;
    let dewpoint_spread = (thermo.temperature_c - thermo.dew_point_c).max(0.0);
    let saturation_fog = smoothstep(2.2, 0.15, dewpoint_spread);
    let nocturnal_cooling = smoothstep(0.08, -0.04, sun_elevation_sine);
    let wind_suppression = (1.0 - smoothstep(1.5, 7.0, surface_wind_mps) * 0.82).clamp(0.12, 1.0);
    let fog =
        (saturation_fog * (0.24 + nocturnal_cooling * 0.76) * wind_suppression).clamp(0.0, 1.0);
    let haze = (aerosol * hygroscopic_growth * 0.62).clamp(0.0, 1.0);

    // Meteorological optical range (Koschmieder closure) from explicit extinction terms.
    let extinction_per_km = 0.065
        + aerosol * hygroscopic_growth * 0.55
        + fog * 5.4
        + micro.precipitation_rate_mm_h * 0.018;
    let visibility_m = (3912.0 / extinction_per_km.max(0.065)).clamp(80.0, 60_000.0);

    let _ = boundary; // boundary is intentionally explicit in this node's dependency contract.
    OpticalState {
        aerosol_density: aerosol,
        haze_amount: haze,
        fog_density: fog,
        visibility_m,
    }
}
