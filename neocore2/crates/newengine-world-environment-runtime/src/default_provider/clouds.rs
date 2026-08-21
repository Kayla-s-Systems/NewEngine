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
    let low_coverage = clamp01_f32(cloud_coverage * (0.66 + overcast * 0.22));
    let high_coverage =
        clamp01_f32(cloud_coverage * (0.24 + pressure.clamp(0.0, 1.0) * 0.24) + overcast * 0.08);
    vec![
        CloudLayerDto {
            altitude_min_meters: low_layer_base,
            altitude_max_meters: low_layer_base + 1100.0,
            coverage: low_coverage,
            // Density must disappear with coverage. The old constant 0.16 made
            // a nominally clear layer remain optically present forever.
            density: clamp01_f32(low_coverage * 0.44 + precipitation * 0.14),
            wind_velocity: wind.cloud_advection,
        },
        CloudLayerDto {
            altitude_min_meters: 2800.0,
            altitude_max_meters: 5200.0,
            coverage: high_coverage,
            density: clamp01_f32(high_coverage * 0.34 + overcast * 0.06),
            wind_velocity: Vec3Dto::new(
                wind.cloud_advection.x * 1.35,
                0.0,
                wind.cloud_advection.z * 1.35,
            ),
        },
    ]
}
