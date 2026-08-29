use crate::{
    math::clamp01_f32,
    profile_catalog::{cloud_profile_by_id, EnvironmentProfileDescriptor},
};
use newengine_world_environment_api::{AtmosphericLayerDto, CloudLayerDto, Vec3Dto, WindStateDto};

#[allow(clippy::too_many_arguments)]
pub(super) fn cloud_layers(
    profile: &EnvironmentProfileDescriptor,
    cloud_coverage: f32,
    overcast: f32,
    lifting_condensation_level_m: f32,
    cloud_water_path_kg_m2: f32,
    convective_cloud_top_m: f32,
    upper_ice_cloud_signal: f32,
    vertical_layers: &[AtmosphericLayerDto; 5],
    wind: &WindStateDto,
) -> Vec<CloudLayerDto> {
    let morphology = cloud_profile_by_id(profile.cloud_profile_ref);
    let low_base = lifting_condensation_level_m.clamp(50.0, 5000.0);
    let low_top = convective_cloud_top_m
        .max(low_base + 320.0)
        .clamp(low_base + 320.0, 12_000.0);
    let low_thickness = (low_top - low_base).max(320.0);
    let low_coverage = clamp01_f32(
        cloud_coverage
            * (morphology.low_coverage_scale + overcast * morphology.low_overcast_coverage_gain),
    );
    let optical_mass_per_km =
        cloud_water_path_kg_m2.clamp(0.0, 5.0) / (low_thickness * 0.001).max(0.25);
    let low_density =
        clamp01_f32(low_coverage * morphology.low_density_scale + optical_mass_per_km * 0.08);

    let thermodynamic_high = upper_ice_cloud_signal.clamp(0.0, 1.0);
    let ice_spread_efficiency =
        (0.58 + morphology.high_cloud_coverage_scale * 0.85).clamp(0.45, 0.92);
    let high_coverage = clamp01_f32(thermodynamic_high * ice_spread_efficiency);
    let (high_base, high_top) = physical_ice_bounds(vertical_layers).unwrap_or((5000.0, 7000.0));
    let high_density =
        clamp01_f32(high_coverage * morphology.high_density_scale + thermodynamic_high * 0.22);

    vec![
        CloudLayerDto {
            altitude_min_meters: low_base,
            altitude_max_meters: low_top,
            coverage: low_coverage,
            density: low_density,
            wind_velocity: wind.cloud_advection,
        },
        CloudLayerDto {
            altitude_min_meters: high_base,
            altitude_max_meters: high_top.max(high_base + 800.0),
            coverage: high_coverage,
            density: high_density,
            wind_velocity: Vec3Dto::new(
                wind.cloud_advection.x * 1.18,
                0.0,
                wind.cloud_advection.z * 1.18,
            ),
        },
    ]
}

fn physical_ice_bounds(layers: &[AtmosphericLayerDto; 5]) -> Option<(f32, f32)> {
    let mut base = None::<f32>;
    let mut top = None::<f32>;
    for layer in layers {
        let transported_ice = layer.cloud_water_content_g_m3
            * layer.ice_fraction
            * (layer.vertical_velocity_mps / 2.5).clamp(0.0, 1.0);
        if transported_ice > 0.012 {
            base = Some(base.map_or(layer.altitude_agl_meters, |value| {
                value.min(layer.altitude_agl_meters)
            }));
            top = Some(top.map_or(layer.altitude_agl_meters, |value| {
                value.max(layer.altitude_agl_meters)
            }));
        }
    }
    base.zip(top)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_catalog::profile_by_id;

    #[test]
    fn low_cloud_base_is_lcl_not_authored_profile_altitude() {
        let wind = WindStateDto::default();
        let (alpine, _) = profile_by_id("environment.alpine_winter");
        let (desert, _) = profile_by_id("environment.desert_dusk");
        let layers = [AtmosphericLayerDto::default(); 5];
        let alpine_clouds =
            cloud_layers(alpine, 0.55, 0.28, 930.0, 0.18, 2600.0, 0.0, &layers, &wind);
        let desert_clouds =
            cloud_layers(desert, 0.55, 0.28, 930.0, 0.18, 2600.0, 0.0, &layers, &wind);
        assert!((alpine_clouds[0].altitude_min_meters - 930.0).abs() < 0.01);
        assert!((desert_clouds[0].altitude_min_meters - 930.0).abs() < 0.01);
    }

    #[test]
    fn cirrus_requires_transported_ice() {
        let wind = WindStateDto::default();
        let (profile, _) = profile_by_id("environment.game_ready_forest_road");
        let mut layers = [AtmosphericLayerDto::default(); 5];
        layers[3].altitude_agl_meters = 5000.0;
        layers[3].cloud_water_content_g_m3 = 0.45;
        layers[3].ice_fraction = 0.9;
        layers[3].vertical_velocity_mps = 0.0;
        let no_transport =
            cloud_layers(profile, 0.2, 0.0, 1200.0, 0.05, 1800.0, 0.0, &layers, &wind);
        assert_eq!(no_transport[1].coverage, 0.0);

        layers[3].vertical_velocity_mps = 4.0;
        let transported =
            cloud_layers(profile, 0.4, 0.1, 1000.0, 0.35, 6000.0, 0.8, &layers, &wind);
        assert!(transported[1].coverage > 0.25);
        assert!(transported[1].altitude_min_meters >= 5000.0);
    }
}
