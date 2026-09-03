use newengine_world_api::WorldCellCoord;
use newengine_world_environment_api::{
    EnvironmentFrameRequest, EnvironmentSampleAtPositionRequest, EnvironmentSurfaceBoundaryDto,
    EnvironmentWeatherConstraint, PrecipitationKind, Vec3Dto, WeatherKind,
};

use crate::{
    constants::{WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE, WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE},
    default_provider::{
        build_default_environment_frame, build_default_environment_frame_with_history,
    },
    provider_state::EnvironmentProviderState,
};

fn request(profile: &str, normalized_day: f64, seed: u64) -> EnvironmentFrameRequest {
    let mut req = EnvironmentFrameRequest::default();
    req.environment_profile.profile_id = profile.to_owned();
    req.seed = seed;
    req.time.game.day_index = 171;
    req.time.game.normalized_day = normalized_day;
    req.time.game.seconds_of_day = normalized_day * req.time.game.seconds_per_game_day;
    req
}

fn frame(req: EnvironmentFrameRequest) -> newengine_world_environment_api::EnvironmentFrameDto {
    build_default_environment_frame(
        "environment.default",
        WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
        req,
    )
}

#[test]
#[cfg_attr(
    miri,
    ignore = "environment f32 bit patterns are validated by native tests; Miri is used for UB semantics"
)]
fn default_provider_is_deterministic_for_same_request() {
    let req = request("environment.default", 0.5, 42);
    let a = frame(req.clone());
    let b = frame(req);
    assert_eq!(a, b);
    assert!(!a.diagnostics.degraded);
    assert_eq!(
        a.visual_assets.visual_group_id,
        "environment.visuals.game_ready_skydome.v1"
    );
    assert_eq!(
        a.visual_assets.texture_dictionary_ref,
        "textures/environment/skydome.ytd"
    );
    assert_eq!(
        a.visual_assets.sky_texture_ref,
        a.consumer_packets.render.sky_texture_ref
    );
}

#[test]
fn clear_sky_constraint_is_physical_consumer_state_not_renderer_only_mask() {
    let mut req = request("environment.game_ready_highlands", 0.469, 0);
    req.weather_constraint = EnvironmentWeatherConstraint::ClearSky;
    let frame = frame(req);

    assert_eq!(frame.weather.state, WeatherKind::Clear);
    assert_eq!(frame.weather.weather_id, "weather.clear.dry_high_pressure");
    assert_eq!(frame.clouds.coverage, 0.0);
    assert_eq!(frame.clouds.overcast, 0.0);
    assert_eq!(frame.clouds.shadow_strength, 0.0);
    assert_eq!(frame.clouds.light_absorption, 0.0);
    assert!(frame.clouds.layers.is_empty());
    assert!(frame.clouds.volumes.is_empty());
    assert!(frame.clouds.storm_cells.is_empty());
    assert_eq!(frame.sky.overcast_blend, 0.0);
    assert_eq!(frame.weather.precipitation.intensity, 0.0);
    assert_eq!(frame.weather.precipitation.rate_mm_per_hour, 0.0);
    assert_eq!(frame.weather.thunder.probability, 0.0);
    assert_eq!(frame.exposure_intent.storm_darkening, 0.0);
    assert!(frame.consumer_packets.render.sun_intensity_hint > 80_000.0);
    assert!(
        (frame.consumer_packets.render.sun_intensity_hint - frame.celestial.sun.intensity_lux_hint)
            .abs()
            < 1.0
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "environment f32 bit patterns are validated by native tests; Miri is used for UB semantics"
)]
fn weather_physics_is_seed_independent() {
    let a = frame(request("environment.game_ready_forest_road", 0.42, 1));
    let b = frame(request(
        "environment.game_ready_forest_road",
        0.42,
        9_999_991,
    ));

    // The seed may identify procedural presentation objects or the moon phase, but it is
    // explicitly not a meteorological boundary condition.
    assert_eq!(a.atmosphere, b.atmosphere);
    assert_eq!(a.wind, b.wind);
    assert_eq!(a.clouds.coverage, b.clouds.coverage);
    assert_eq!(a.clouds.overcast, b.clouds.overcast);
    assert_eq!(a.clouds.layers, b.clouds.layers);
    assert_eq!(
        a.weather.precipitation.rate_mm_per_hour,
        b.weather.precipitation.rate_mm_per_hour
    );
    assert_eq!(a.weather.thunder.probability, b.weather.thunder.probability);
    assert_eq!(a.weather.state, b.weather.state);
}

#[test]
fn environment_profile_name_cannot_force_weather_by_substring() {
    let mut req = request(
        "environment.fake_storm_name_that_is_not_registered",
        0.50,
        7,
    );
    req.active_region = None;
    let frame = frame(req);
    assert_eq!(
        frame.global.active_environment_profile,
        "environment.default"
    );
    assert!(frame
        .diagnostics
        .warnings
        .iter()
        .any(|warning| warning.contains("unknown environment profile")));
    assert!(frame
        .diagnostics
        .reasons
        .iter()
        .any(|reason| reason.contains("atmosphere_graph=boundary->prognostic")));
}

#[test]
fn yweather_is_observation_mapping_not_physical_source() {
    let frame = frame(request("environment.game_ready_forest_road", 0.36, 123));
    assert!(frame
        .diagnostics
        .reasons
        .iter()
        .any(|reason| reason.contains("weather state is diagnosed from physics")));
    assert!(frame
        .diagnostics
        .reasons
        .iter()
        .any(|reason| reason.contains("weather physics contains no seed/noise/random input")));
    assert!(!frame.global.weather_table_ref.is_empty());
    assert!(!frame.visual_assets.weather_visual_ref.is_empty());
}

#[test]
fn visual_asset_refs_use_existing_grouped_skydome_dictionary() {
    let frame = frame(EnvironmentFrameRequest::default());
    let serialized = serde_json::to_string(&frame.visual_assets).expect("visual assets serialize");
    assert!(serialized.contains("textures/environment/skydome.ytd"));
    assert!(!serialized.contains("textures/sky/highlands_sky.ytd"));
    assert!(!serialized.contains("textures/sky/default_sky.ytd"));
    assert!(!serialized.contains("textures/sky/alpine_sky.ytd"));
    assert!(!serialized.contains("textures/sky/desert_sky.ytd"));
    assert!(!serialized.contains("textures/sky/celestial.ytd"));
}

#[test]
fn null_provider_returns_visible_degraded_frame() {
    let state = EnvironmentProviderState::new(
        "environment.null",
        WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE,
        true,
    );
    assert!(state.last_frame.diagnostics.degraded);
    assert_eq!(
        state.last_frame.diagnostics.provider_route,
        WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE
    );
}

#[test]
fn thermodynamic_graph_is_finite_hydrostatic_and_physically_bounded() {
    let profiles = [
        "environment.game_ready_forest_road",
        "environment.game_ready_highlands",
        "environment.default",
        "environment.alpine_winter",
        "environment.desert_dusk",
    ];
    for profile in profiles {
        for tod in [0.08_f64, 0.25, 0.50, 0.72, 0.92] {
            let frame = frame(request(profile, tod, 0));
            let a = &frame.atmosphere;
            for value in [
                a.surface_pressure_hpa,
                a.temperature_celsius,
                a.dew_point_celsius,
                a.specific_humidity_g_per_kg,
                a.vapor_pressure_hpa,
                a.saturation_vapor_pressure_hpa,
                a.air_density_kg_m3,
                a.lifting_condensation_level_meters,
                a.precipitable_water_mm,
                a.cloud_water_path_kg_m2,
                a.condensation_potential,
                a.cape_j_per_kg,
                a.cin_j_per_kg,
                a.convective_cloud_top_meters,
            ] {
                assert!(
                    value.is_finite(),
                    "profile={profile} tod={tod} value={value}"
                );
            }
            assert!((650.0..=1065.0).contains(&a.surface_pressure_hpa));
            assert!((-75.0..=58.0).contains(&a.temperature_celsius));
            assert!(a.dew_point_celsius <= a.temperature_celsius + 0.05);
            assert!((0.0..=1.0).contains(&a.humidity));
            assert!((0.0..=90.0).contains(&a.precipitable_water_mm));
            assert!((0.0..=5.0).contains(&a.cloud_water_path_kg_m2));
            assert!((0.45..=1.65).contains(&a.air_density_kg_m3));
            assert!((50.0..=5000.0).contains(&a.lifting_condensation_level_meters));
            assert!((0.0..=7000.0).contains(&a.cape_j_per_kg));
            assert!((0.0..=1500.0).contains(&a.cin_j_per_kg));
            assert!((50.0..=12_000.0).contains(&a.convective_cloud_top_meters));
            for pair in a.vertical_layers.windows(2) {
                assert!(pair[1].altitude_agl_meters > pair[0].altitude_agl_meters);
                assert!(pair[1].pressure_hpa < pair[0].pressure_hpa);
            }
            for layer in a.vertical_layers {
                assert!(layer.pressure_hpa.is_finite());
                assert!(layer.temperature_celsius.is_finite());
                assert!((0.0..=1.0).contains(&layer.relative_humidity));
                assert!((0.0..=5.0).contains(&layer.cloud_water_content_g_m3));
                assert!((0.0..=1.0).contains(&layer.ice_fraction));
                assert!((-4.0..=22.0).contains(&layer.vertical_velocity_mps));
            }
        }
    }
}

#[test]
fn low_cloud_base_is_the_lifting_condensation_level() {
    for profile in [
        "environment.game_ready_forest_road",
        "environment.game_ready_highlands",
        "environment.alpine_winter",
        "environment.desert_dusk",
    ] {
        let frame = frame(request(profile, 0.50, 0));
        let low = frame.clouds.layers.first().expect("low cloud layer");
        assert!(
            (low.altitude_min_meters - frame.atmosphere.lifting_condensation_level_meters).abs()
                < 0.01,
            "profile={profile} layer={} LCL={}",
            low.altitude_min_meters,
            frame.atmosphere.lifting_condensation_level_meters
        );
    }
}

#[test]
fn cirrus_requires_actual_transported_upper_ice() {
    for profile in [
        "environment.game_ready_forest_road",
        "environment.game_ready_highlands",
        "environment.alpine_winter",
    ] {
        for tod in [0.20, 0.50, 0.75] {
            let frame = frame(request(profile, tod, 0));
            let high = frame.clouds.layers.get(1).expect("high cloud layer");
            if high.coverage > 0.005 {
                let transported_ice = frame.atmosphere.vertical_layers[3..]
                    .iter()
                    .map(|layer| {
                        layer.cloud_water_content_g_m3
                            * layer.ice_fraction
                            * (layer.vertical_velocity_mps / 2.5).clamp(0.0, 1.0)
                    })
                    .sum::<f32>();
                assert!(
                    transported_ice > 0.012,
                    "high={high:?} transported={transported_ice}"
                );
            }
        }
    }
}

#[test]
fn precipitation_contract_is_mass_based() {
    for profile in [
        "environment.game_ready_forest_road",
        "environment.alpine_winter",
        "environment.desert_dusk",
    ] {
        for tod in [0.15, 0.40, 0.65, 0.85] {
            let frame = frame(request(profile, tod, 0));
            let p = frame.weather.precipitation;
            assert!(p.rate_mm_per_hour.is_finite());
            assert!(p.rate_mm_per_hour >= 0.0);
            assert!((0.0..=1.0).contains(&p.intensity));
            assert!((p.intensity - (p.rate_mm_per_hour / 32.0).clamp(0.0, 1.0)).abs() < 1.0e-5);
            if p.rate_mm_per_hour < 0.05 {
                assert_eq!(p.kind, PrecipitationKind::None);
            } else {
                assert!(matches!(
                    p.kind,
                    PrecipitationKind::Rain | PrecipitationKind::Snow
                ));
            }
        }
    }
}

#[test]
fn one_second_step_is_rate_bounded_not_weather_teleportation() {
    let mut req = request("environment.game_ready_forest_road", 0.22, 77);
    let initial = frame(req.clone());

    req.frame_id = 2;
    req.time.game.normalized_day = 0.68;
    req.time.game.seconds_of_day = initial.world_time_seconds
        - req.time.game.day_index as f64 * req.time.game.seconds_per_game_day
        + 1.0;
    let live = build_default_environment_frame_with_history(
        "environment.default",
        WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
        req,
        Some(&initial),
    );

    let temp_delta =
        (live.atmosphere.temperature_celsius - initial.atmosphere.temperature_celsius).abs();
    let q_delta = (live.atmosphere.specific_humidity_g_per_kg
        - initial.atmosphere.specific_humidity_g_per_kg)
        .abs();
    let cwp_delta =
        (live.atmosphere.cloud_water_path_kg_m2 - initial.atmosphere.cloud_water_path_kg_m2).abs();
    assert!(
        temp_delta < 0.05,
        "temperature jumped {temp_delta} C in one second"
    );
    assert!(
        q_delta < 0.05,
        "specific humidity jumped {q_delta} g/kg in one second"
    );
    assert!(
        cwp_delta < 0.05,
        "CWP jumped {cwp_delta} kg/m2 in one second"
    );
}

#[test]
fn surface_water_and_snow_are_persistent_mass_reservoirs() {
    let mut req = request("environment.alpine_winter", 0.42, 5);
    let mut previous = frame(req.clone());
    previous.weather.wetness.surface_water_mm = 1.20;
    previous.weather.wetness.surface_wetness = 0.80;
    previous.weather.snow.snow_water_equivalent_mm = 20.0;
    previous.weather.snow.surface_snow = 0.50;

    req.frame_id = 2;
    req.time.game.seconds_of_day += 1.0;
    req.time.game.normalized_day =
        req.time.game.seconds_of_day / req.time.game.seconds_per_game_day;
    let next = build_default_environment_frame_with_history(
        "environment.default",
        WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
        req,
        Some(&previous),
    );
    assert!(next.weather.wetness.surface_water_mm > 1.19);
    assert!(next.weather.wetness.surface_wetness > 0.79);
    assert!(next.weather.snow.snow_water_equivalent_mm > 19.99);
    assert!(next.weather.snow.surface_snow > 0.49);
}

#[test]
fn frame_diagnostics_expose_physical_causes_not_hidden_weather_selection() {
    let frame = frame(request("environment.default", 0.50, 123));
    let reasons = frame.diagnostics.reasons.join("\n");
    assert!(reasons.contains("radiation net="));
    assert!(reasons.contains("evaporation="));
    assert!(reasons.contains("thermodynamics p="));
    assert!(reasons.contains("precip="));
    assert!(!reasons.contains("cloud_seed"));
    assert!(!reasons.contains("weather_pattern="));
}

fn assert_vec3_nearly_eq(left: Vec3Dto, right: Vec3Dto) {
    const EPSILON: f32 = 0.000_001;
    assert!((left.x - right.x).abs() <= EPSILON);
    assert!((left.y - right.y).abs() <= EPSILON);
    assert!((left.z - right.z).abs() <= EPSILON);
}

#[test]
fn physical_sun_direction_remains_deterministic() {
    let a = frame(request("environment.default", 0.5, 1));
    let b = frame(request("environment.default", 0.5, 2));
    assert_vec3_nearly_eq(
        a.celestial.sun.direction_world,
        b.celestial.sun.direction_world,
    );
}

#[test]
fn observer_cell_is_authoritative_for_global_environment_state() {
    let mut req = EnvironmentFrameRequest::default();
    req.environment_profile.profile_id = "environment.default".to_owned();
    req.spatial_cell_size_meters = 5000.0;
    req.observer_cell = Some(WorldCellCoord::new(1, 0));
    req.surface_boundaries = vec![
        EnvironmentSurfaceBoundaryDto {
            cell: WorldCellCoord::new(0, 0),
            terrain_elevation_meters: 50.0,
            moisture_availability: 0.85,
            ..EnvironmentSurfaceBoundaryDto::default()
        },
        EnvironmentSurfaceBoundaryDto {
            cell: WorldCellCoord::new(1, 0),
            terrain_elevation_meters: 1450.0,
            moisture_availability: 0.20,
            ..EnvironmentSurfaceBoundaryDto::default()
        },
    ];
    let frame = build_default_environment_frame(
        "environment.default",
        WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
        req,
    );
    assert_eq!(frame.spatial_atmosphere.len(), 2);
    let observer = frame
        .spatial_atmosphere
        .iter()
        .find(|cell| cell.cell == WorldCellCoord::new(1, 0))
        .expect("observer cell");
    assert_eq!(frame.atmosphere, observer.atmosphere);
    assert_eq!(frame.weather, observer.weather);
    assert_eq!(frame.clouds.coverage, observer.clouds.coverage);
    assert_eq!(frame.clouds.overcast, observer.clouds.overcast);
    assert_eq!(frame.clouds.layers, observer.clouds.layers);
    assert_eq!(frame.wind, observer.wind);
}

#[test]
fn sample_at_position_reads_requested_mesoscale_cell_not_global_column() {
    let mut req = EnvironmentFrameRequest::default();
    req.environment_profile.profile_id = "environment.default".to_owned();
    req.spatial_cell_size_meters = 5000.0;
    req.observer_cell = Some(WorldCellCoord::new(0, 0));
    req.surface_boundaries = vec![
        EnvironmentSurfaceBoundaryDto {
            cell: WorldCellCoord::new(0, 0),
            terrain_elevation_meters: 50.0,
            ..EnvironmentSurfaceBoundaryDto::default()
        },
        EnvironmentSurfaceBoundaryDto {
            cell: WorldCellCoord::new(1, 0),
            terrain_elevation_meters: 1800.0,
            ..EnvironmentSurfaceBoundaryDto::default()
        },
    ];
    let frame = build_default_environment_frame(
        "environment.default",
        WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
        req,
    );
    let state = EnvironmentProviderState::new(
        "environment.default",
        WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
        false,
    );
    let response = state.sample_at_position_json_v1(EnvironmentSampleAtPositionRequest {
        frame: frame.clone(),
        position: Vec3Dto::zero(),
        cell: Some(WorldCellCoord::new(1, 0)),
    });
    let high = frame
        .spatial_atmosphere
        .iter()
        .find(|cell| cell.cell == WorldCellCoord::new(1, 0))
        .expect("high terrain cell");
    assert!((response.surface_pressure_hpa - high.atmosphere.surface_pressure_hpa).abs() < 1.0e-5);
    assert!((response.temperature_celsius - high.atmosphere.temperature_celsius).abs() < 1.0e-5);
    assert!(response.surface_pressure_hpa < frame.atmosphere.surface_pressure_hpa - 100.0);
    assert!(response
        .diagnostics
        .reasons
        .iter()
        .any(|reason| reason.contains("source=mesoscale")));
}
