use crate::{math::clamp01_f32, profile_catalog::EnvironmentProfileDescriptor};
use newengine_world_environment_api::{CloudLayerDto, Vec3Dto, WindStateDto};

pub(super) fn cloud_layers(
    profile: &EnvironmentProfileDescriptor,
    cloud_coverage: f32,
    overcast: f32,
    pressure: f32,
    precipitation: f32,
    wind: &WindStateDto,
) -> Vec<CloudLayerDto> {
    let low_layer_base = if profile.biome == "desert" {
        1600.0
    } else {
        1200.0
    };
    vec![
        CloudLayerDto {
            altitude_min_meters: low_layer_base,
            altitude_max_meters: low_layer_base + 1100.0,
            coverage: clamp01_f32(cloud_coverage * 0.70 + overcast * 0.20),
            density: 0.16 + cloud_coverage * 0.30 + precipitation * 0.10,
            wind_velocity: wind.cloud_advection,
        },
        CloudLayerDto {
            altitude_min_meters: 2800.0,
            altitude_max_meters: 5200.0,
            coverage: clamp01_f32(cloud_coverage * 0.45 + pressure * 0.18),
            density: 0.10 + cloud_coverage * 0.20,
            wind_velocity: Vec3Dto::new(
                wind.cloud_advection.x * 1.35,
                0.0,
                wind.cloud_advection.z * 1.35,
            ),
        },
    ]
}
