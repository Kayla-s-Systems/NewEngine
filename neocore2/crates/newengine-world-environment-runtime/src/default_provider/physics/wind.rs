use newengine_world_environment_api::{Vec3Dto, WindStateDto};

use super::state::{BoundaryState, VerticalState};
use super::thermodynamics::smoothstep;

pub(super) fn solve(
    boundary: BoundaryState,
    vertical: VerticalState,
    sun_elevation_sine: f32,
) -> WindStateDto {
    let z0 = boundary.surface_roughness_m.max(0.001);
    let reference_height = boundary.boundary_layer_depth_m.max(100.0);
    let log_surface_ratio =
        ((10.0 / z0).ln() / (reference_height / z0).ln().max(0.1)).clamp(0.18, 0.82);
    let convective_mixing = smoothstep(50.0, 1400.0, vertical.cape_j_kg);
    let solar_mixing = sun_elevation_sine.max(0.0).sqrt();
    let mixing = (0.25 + convective_mixing * 0.45 + solar_mixing * 0.30).clamp(0.15, 1.0);
    let surface_speed = boundary.geostrophic_wind_mps
        * (log_surface_ratio * (0.72 + mixing * 0.28)).clamp(0.12, 0.90);
    let convective_velocity = (2.0 * vertical.cape_j_kg.max(0.0)).sqrt();
    let gust_excess =
        convective_velocity * 0.16 + surface_speed * boundary.surface_roughness_m.sqrt() * 0.22;
    let gust_strength = (gust_excess / 12.0).clamp(0.0, 1.0);
    let direction = boundary.geostrophic_wind;

    WindStateDto {
        global_direction: direction,
        global_speed_mps: surface_speed.clamp(0.0, 45.0),
        gust_strength,
        cloud_advection: Vec3Dto::new(
            direction.x * boundary.geostrophic_wind_mps * 0.86,
            0.0,
            direction.z * boundary.geostrophic_wind_mps * 0.86,
        ),
    }
}
