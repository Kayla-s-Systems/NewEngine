use super::*;

fn test_cycle() -> SkyCycleRuntime {
    SkyCycleRuntime {
        anchor: None,
        sun: None,
        enabled: true,
        time_of_day_hours: 12.0,
        day_length_seconds: 1200.0,
        latitude_degrees: 45.0,
        axial_tilt_degrees: 23.44,
        base_sun_color: [1.0, 0.95, 0.84],
        base_sun_intensity: 3.6,
        base_ambient_color: [0.32, 0.39, 0.52],
        base_ambient_intensity: 0.24,
        day_index: 171,
    }
}

#[test]
fn solar_altitude_is_symmetric_around_solar_noon() {
    let morning = solar_direction_from_cycle(10.0, 45.0, 23.44, 171);
    let afternoon = solar_direction_from_cycle(14.0, 45.0, 23.44, 171);
    assert!((morning.y - afternoon.y).abs() < 1.0e-5);
    assert!((morning.x + afternoon.x).abs() < 1.0e-5);
    assert!((morning.length() - 1.0).abs() < 1.0e-5);
}

#[test]
fn summer_noon_is_higher_than_winter_noon_at_mid_latitude() {
    let summer = solar_direction_from_cycle(12.0, 45.0, 23.44, 171);
    let winter = solar_direction_from_cycle(12.0, 45.0, 23.44, 354);
    assert!(
        summer.y > winter.y + 0.40,
        "summer={summer:?} winter={winter:?}"
    );
}

#[test]
fn authored_twilight_has_monotonic_sun_and_ambient_energy() {
    let cycle = test_cycle();
    let night = sample_sky_frame(&cycle, None, Vec3::new(0.0, -0.25, 0.968));
    let dawn = sample_sky_frame(&cycle, None, Vec3::new(0.0, 0.0, 1.0));
    let day = sample_sky_frame(&cycle, None, Vec3::new(0.0, 0.65, 0.760));
    assert!(night.sun_intensity < dawn.sun_intensity);
    assert!(dawn.sun_intensity < day.sun_intensity);
    assert!(night.ambient_intensity < dawn.ambient_intensity);
    assert!(dawn.ambient_intensity < day.ambient_intensity);
}

#[test]
fn overcast_day_postfx_preserves_visibility_without_flattening_color() {
    let mut env = newengine_world_environment_api::EnvironmentFrameDto::default();
    env.time_of_day_state.day_blend = 1.0;
    env.time_of_day_state.night_blend = 0.0;
    env.sky.overcast_blend = 0.82;
    env.atmosphere.haze_amount = 0.28;
    env.exposure_intent.storm_darkening = 0.18;
    env.exposure_intent.sun_glare_hint = 0.05;
    let postfx = sky_postfx_from_environment(&env);
    assert!((0.95..=1.30).contains(&postfx.exposure));
    assert!((0.88..=1.08).contains(&postfx.saturation));
    assert!((0.90..=1.08).contains(&postfx.contrast));
    assert!(postfx.black_lift < 0.01);
}

#[test]
fn night_adaptation_lifts_exposure_but_keeps_black_floor_small() {
    let mut day = newengine_world_environment_api::EnvironmentFrameDto::default();
    day.time_of_day_state.day_blend = 1.0;
    day.time_of_day_state.night_blend = 0.0;
    day.exposure_intent.night_adaptation_hint = 0.0;

    let mut night = day.clone();
    night.time_of_day_state.day_blend = 0.0;
    night.time_of_day_state.night_blend = 1.0;
    night.exposure_intent.night_adaptation_hint = 1.0;

    let day_postfx = sky_postfx_from_environment(&day);
    let night_postfx = sky_postfx_from_environment(&night);
    assert!(night_postfx.exposure > day_postfx.exposure + 0.20);
    assert!(night_postfx.black_lift > day_postfx.black_lift);
    assert!(night_postfx.black_lift <= 0.01);
}

#[test]
fn live_cloud_dynamics_integrates_wind_without_position_jump() {
    let mut world = newengine_ecs::World::new();
    let mut frame = sample_sky_frame(&test_cycle(), None, Vec3::new(0.0, 0.65, 0.760));
    frame.cloud_advection = Vec2::new(4.0, 1.0);
    frame.cloud_gust_strength = 0.35;
    let first = update_sky_dynamics(&mut world, &frame, 1.0 / 60.0);
    for _ in 0..120 {
        let _ = update_sky_dynamics(&mut world, &frame, 1.0 / 60.0);
    }
    let before_change = *world.resource::<SkyDynamicsRuntime>().unwrap();

    frame.cloud_advection = Vec2::new(-2.0, 3.0);
    let changed = update_sky_dynamics(&mut world, &frame, 1.0 / 60.0);
    let delta = (changed.cloud_offset - before_change.cloud_offset).length();

    assert!(first.cloud_offset.is_finite());
    assert!(delta < 0.01, "wind change teleported clouds delta={delta}");
    assert!(changed.cloud_offset.x >= 0.0 && changed.cloud_offset.x < 1024.0);
    assert!(changed.cloud_offset.y >= 0.0 && changed.cloud_offset.y < 1024.0);
}

#[test]
fn live_cloud_shape_and_lifecycle_advance_independently() {
    let mut world = newengine_ecs::World::new();
    let mut frame = sample_sky_frame(&test_cycle(), None, Vec3::new(0.0, 0.65, 0.760));
    frame.cloud_advection = Vec2::new(5.0, 1.5);
    frame.cloud_overcast = 0.55;
    frame.cloud_light_absorption = 0.28;
    let start = update_sky_dynamics(&mut world, &frame, 1.0 / 60.0);
    for _ in 0..1800 {
        let _ = update_sky_dynamics(&mut world, &frame, 1.0 / 60.0);
    }
    let end = update_sky_dynamics(&mut world, &frame, 1.0 / 60.0);

    assert_ne!(start.evolution_phase, end.evolution_phase);
    assert_ne!(start.lifecycle, end.lifecycle);
    assert!((0.0..=1.0).contains(&end.coverage));
    assert!((0.04..=0.98).contains(&end.softness));
}

#[test]
fn live_cloud_coverage_smoothly_tracks_weather_target() {
    let mut world = newengine_ecs::World::new();
    let mut frame = sample_sky_frame(&test_cycle(), None, Vec3::new(0.0, 0.65, 0.760));
    frame.cloud_coverage = 0.10;
    for _ in 0..60 {
        let _ = update_sky_dynamics(&mut world, &frame, 1.0 / 60.0);
    }
    let low = world
        .resource::<SkyDynamicsRuntime>()
        .unwrap()
        .smoothed_coverage;

    frame.cloud_coverage = 0.90;
    let immediate = update_sky_dynamics(&mut world, &frame, 1.0 / 60.0);
    assert!(immediate.coverage < 0.30, "coverage snapped to target");
    for _ in 0..3600 {
        let _ = update_sky_dynamics(&mut world, &frame, 1.0 / 60.0);
    }
    let high = world
        .resource::<SkyDynamicsRuntime>()
        .unwrap()
        .smoothed_coverage;
    assert!(
        high > low + 0.60,
        "coverage did not converge low={low} high={high}"
    );
}

#[test]
fn dense_cloud_optical_depth_reduces_direct_light_more_than_skylight() {
    let mut frame = sample_sky_frame(&test_cycle(), None, Vec3::new(0.15, 0.78, 0.607));
    frame.cloud_light_absorption = 0.32;
    frame.cloud_overcast = 0.48;
    frame.cloud_shadow_strength = 0.62;

    let clear = sky_cloud_occlusion_from_density(&frame, 0.05, 0.05);
    let dense = sky_cloud_occlusion_from_density(&frame, 0.92, 0.92);

    assert!(dense.optical_depth > clear.optical_depth);
    assert!(dense.transmittance < clear.transmittance * 0.35);
    assert!(dense.direct_light_scale < 0.35);
    assert!(dense.world_shadow_strength > clear.world_shadow_strength);
}

#[test]
fn sun_below_horizon_has_no_cloud_occlusion_response() {
    let frame = sample_sky_frame(&test_cycle(), None, Vec3::new(0.0, -0.20, 0.980));
    let density = sky_cloud_sun_density(&frame, 1.0, 0.50, Vec2::new(0.33, 0.71), 0.42, 0.80);
    assert_eq!(density, 0.0);
}

#[test]
fn macro_cloud_field_evolves_without_discontinuity() {
    let plane = Vec2::new(1.7, -0.8);
    let offset = Vec2::new(0.18, 0.41);
    let a = sky_macro_cloud_field(plane, offset, 0.2500, 0.55);
    let b = sky_macro_cloud_field(plane, offset, 0.2501, 0.55);
    let c = sky_macro_cloud_field(plane, offset, 0.4500, 0.55);
    assert!((a - b).abs() < 0.01, "temporal discontinuity a={a} b={b}");
    assert!((a - c).abs() > 0.005, "field did not evolve a={a} c={c}");
}

#[test]
fn sun_occlusion_attack_and_release_are_smoothed() {
    let mut world = newengine_ecs::World::new();
    let mut frame = sample_sky_frame(&test_cycle(), None, Vec3::new(0.15, 0.78, 0.607));
    frame.cloud_coverage = 0.95;
    frame.cloud_light_absorption = 0.40;
    frame.cloud_shadow_strength = 0.75;
    frame.cloud_advection = Vec2::ZERO;

    let first = update_sky_dynamics(&mut world, &frame, 1.0 / 60.0);
    assert!(
        (first.sun_occlusion.smoothed_density - first.sun_occlusion.raw_density).abs() < 1.0e-6,
        "initial cloud lighting must not flash from a clear-sky state"
    );
    for _ in 0..240 {
        let _ = update_sky_dynamics(&mut world, &frame, 1.0 / 60.0);
    }
    let settled = *world.resource::<SkyDynamicsRuntime>().unwrap();
    assert!(settled.smoothed_sun_occlusion.is_finite());
    assert!((0.0..=1.0).contains(&settled.smoothed_sun_occlusion));
}

#[test]
fn spatial_cloud_shadow_varies_across_the_same_world_frame() {
    let shadow = SpatialCloudShadowRuntime {
        map0: [0.13, 0.37, 0.21, 0.62],
        map1: [0.0048, 1800.0, 0.56, 0.64],
        map2: [0.88, 0.25, 0.82, 1.0],
        map3: [0.12, 0.35, 0.20, 0.61],
        map4: [0.74, 0.036, 0.17, 96.0],
        broad_ambient_scale: 0.94,
    };
    let sun = Vec3::new(0.30, 0.82, 0.48).normalize();
    let (samples, min_value, max_value) = spatial_cloud_shadow_probe(&shadow, sun);
    assert!(
        max_value - min_value > 0.12,
        "cloud field collapsed to a global multiplier samples={samples:?}"
    );
    assert!(samples.iter().all(|sample| (0.0..=1.0).contains(sample)));
}

#[test]
fn spatial_cloud_shadow_moves_with_integrated_wind_offset() {
    let base = SpatialCloudShadowRuntime {
        map0: [0.08, 0.16, 0.12, 0.58],
        map1: [0.0044, 1750.0, 0.52, 0.68],
        map2: [0.82, 0.22, 0.86, 1.0],
        map3: [0.07, 0.15, 0.11, 0.57],
        map4: [0.72, 0.034, 0.16, 92.0],
        broad_ambient_scale: 0.95,
    };
    let mut moved = base;
    moved.map0[0] += 0.19;
    moved.map0[1] -= 0.11;
    moved.map0[2] += 0.07;
    let sun = Vec3::new(-0.24, 0.76, 0.60).normalize();
    let point = Vec3::new(12.0, 0.0, -7.0);
    let before = sample_spatial_cloud_shadow_cpu(&base, sun, point);
    let after = sample_spatial_cloud_shadow_cpu(&moved, sun, point);
    assert!(
        (before - after).abs() > 0.02,
        "shadow did not move before={before} after={after}"
    );
}

#[test]
fn disabled_or_night_spatial_shadow_is_fully_lit() {
    let disabled = SpatialCloudShadowRuntime::default();
    let point = Vec3::new(20.0, 0.0, 10.0);
    assert_eq!(
        sample_spatial_cloud_shadow_cpu(&disabled, Vec3::new(0.2, 0.8, 0.4).normalize(), point,),
        1.0
    );
    let enabled = SpatialCloudShadowRuntime {
        map2: [1.0, 0.5, 0.8, 1.0],
        ..SpatialCloudShadowRuntime::default()
    };
    assert_eq!(
        sample_spatial_cloud_shadow_cpu(&enabled, Vec3::new(0.2, -0.8, 0.4).normalize(), point,),
        1.0
    );
}

#[test]
fn nearly_clear_sky_does_not_project_orphan_cloud_shadows() {
    let mut frame = sample_sky_frame(&test_cycle(), None, Vec3::new(0.22, 0.82, 0.53).normalize());
    frame.cloud_coverage = 0.02;
    frame.cloud_overcast = 0.0;
    frame.cloud_light_absorption = 0.05;
    frame.cloud_shadow_strength = 0.90;
    let dynamics = SkyDynamicsFrame {
        cloud_offset: Vec2::new(0.17, 0.29),
        coverage: 0.02,
        softness: 0.72,
        shadow_strength: 0.90,
        haze: 0.08,
        evolution_phase: 0.24,
        lifecycle: 0.62,
        gust_factor: 1.0,
        previous_cloud_offset: Vec2::new(0.17, 0.29),
        previous_evolution_phase: 0.24,
        previous_lifecycle: 0.62,
        temporal_history_weight: 0.0,
        sun_occlusion: CloudSunOcclusionRuntime::default(),
    };

    let shadow = spatial_cloud_shadow_from_dynamics(&frame, &dynamics);
    assert_eq!(shadow.map2[3], 0.0, "clear sky enabled local shadow field");
    assert!(
        shadow.map2[0] <= 1.0e-6,
        "clear sky retained local shadow strength={}",
        shadow.map2[0]
    );
    assert_eq!(
        sample_spatial_cloud_shadow_cpu(&shadow, frame.to_sun, Vec3::new(18.0, 0.0, -11.0),),
        1.0
    );
}

#[test]
fn coherent_fair_cumulus_keeps_spatial_cloud_shadows_enabled() {
    let mut frame = sample_sky_frame(&test_cycle(), None, Vec3::new(0.22, 0.82, 0.53).normalize());
    frame.cloud_coverage = 0.45;
    frame.cloud_overcast = 0.18;
    frame.cloud_light_absorption = 0.22;
    frame.cloud_shadow_strength = 0.62;
    let dynamics = SkyDynamicsFrame {
        cloud_offset: Vec2::new(0.17, 0.29),
        coverage: 0.45,
        softness: 0.68,
        shadow_strength: 0.62,
        haze: 0.12,
        evolution_phase: 0.24,
        lifecycle: 0.62,
        gust_factor: 1.0,
        previous_cloud_offset: Vec2::new(0.168, 0.287),
        previous_evolution_phase: 0.238,
        previous_lifecycle: 0.618,
        temporal_history_weight: 0.72,
        sun_occlusion: CloudSunOcclusionRuntime::default(),
    };

    let shadow = spatial_cloud_shadow_from_dynamics(&frame, &dynamics);
    assert_eq!(shadow.map2[3], 1.0);
    assert!(shadow.map2[0] > 0.40);
}

#[test]
fn spatial_shadow_broad_light_scale_is_not_local_camera_occlusion() {
    let mut frame = sample_sky_frame(&test_cycle(), None, Vec3::new(0.3, 0.82, 0.48).normalize());
    frame.cloud_coverage = 0.62;
    frame.cloud_overcast = 0.18;
    frame.cloud_light_absorption = 0.28;
    let dynamics = SkyDynamicsFrame {
        cloud_offset: Vec2::new(0.14, 0.31),
        coverage: 0.62,
        softness: 0.66,
        shadow_strength: 0.58,
        haze: 0.12,
        evolution_phase: 0.22,
        lifecycle: 0.61,
        gust_factor: 1.0,
        previous_cloud_offset: Vec2::new(0.138, 0.307),
        previous_evolution_phase: 0.218,
        previous_lifecycle: 0.608,
        temporal_history_weight: 0.76,
        sun_occlusion: CloudSunOcclusionRuntime {
            transmittance: 0.04,
            direct_light_scale: 0.04,
            ..CloudSunOcclusionRuntime::default()
        },
    };
    let spatial = spatial_cloud_shadow_from_dynamics(&frame, &dynamics);
    assert!(spatial.map2[2] > dynamics.sun_occlusion.transmittance + 0.45);
    assert!(spatial.map2[2] <= 1.0);
}

fn erosion_test_shadow() -> SpatialCloudShadowRuntime {
    SpatialCloudShadowRuntime {
        map0: [0.137, 0.291, 0.238, 0.64],
        map1: [0.0047, 1760.0, 0.57, 0.63],
        map2: [0.90, 0.27, 0.82, 1.0],
        map3: [0.132, 0.286, 0.234, 0.635],
        map4: [0.0, 0.041, 0.22, 104.0],
        broad_ambient_scale: 0.94,
    }
}

#[test]
fn near_erosion_adds_detail_but_fades_out_at_distance() {
    let detailed = erosion_test_shadow();
    let mut coarse = detailed;
    coarse.map4[2] = 0.0;
    let sun = Vec3::new(0.26, 0.81, 0.52).normalize();
    let points = [
        Vec3::new(-48.0, 0.0, -31.0),
        Vec3::new(-24.0, 0.0, 17.0),
        Vec3::new(4.0, 0.0, -9.0),
        Vec3::new(29.0, 0.0, 21.0),
        Vec3::new(53.0, 0.0, -18.0),
        Vec3::new(71.0, 0.0, 37.0),
    ];
    let far_camera = Vec3::new(700.0, 35.0, -620.0);
    let mut near_delta = 0.0f32;
    let mut far_delta = 0.0f32;
    for point in points {
        let detailed_near = sample_spatial_cloud_shadow_cpu_at(&detailed, sun, point, point);
        let coarse_near = sample_spatial_cloud_shadow_cpu_at(&coarse, sun, point, point);
        near_delta = near_delta.max((detailed_near - coarse_near).abs());

        let detailed_far = sample_spatial_cloud_shadow_cpu_at(&detailed, sun, point, far_camera);
        let coarse_far = sample_spatial_cloud_shadow_cpu_at(&coarse, sun, point, far_camera);
        far_delta = far_delta.max((detailed_far - coarse_far).abs());
    }
    assert!(
        near_delta > 0.025,
        "near erosion has no visible effect delta={near_delta}"
    );
    assert!(
        far_delta < 0.002,
        "erosion leaked into far field delta={far_delta}"
    );
}

#[test]
fn temporal_reprojection_stabilizes_far_shadow_without_freezing_it() {
    let mut history = erosion_test_shadow();
    history.map4[0] = 0.84;
    history.map4[2] = 0.0;
    history.map0 = [0.148, 0.303, 0.245, 0.648];
    history.map3 = [0.137, 0.291, 0.238, 0.640];
    let mut current_only = history;
    current_only.map4[0] = 0.0;
    let mut previous_only = history;
    previous_only.map0 = history.map3;
    previous_only.map4[0] = 0.0;

    let sun = Vec3::new(-0.21, 0.79, 0.57).normalize();
    let camera = Vec3::new(620.0, 20.0, -540.0);
    let mut stabilized_change = false;
    let mut reactive_reset = false;
    for x in (-96..=96).step_by(16) {
        for z in (-96..=96).step_by(16) {
            let point = Vec3::new(x as f32, 0.0, z as f32);
            let current = sample_spatial_cloud_shadow_cpu_at(&current_only, sun, point, camera);
            let previous = sample_spatial_cloud_shadow_cpu_at(&previous_only, sun, point, camera);
            let reprojected = sample_spatial_cloud_shadow_cpu_at(&history, sun, point, camera);
            let frame_delta = (current - previous).abs();
            let history_effect = (reprojected - current).abs();

            if frame_delta > 0.006 && frame_delta < 0.24 && history_effect > 1.0e-5 {
                stabilized_change = true;
                assert!(
                        (reprojected - previous).abs() < frame_delta,
                        "history did not stabilize current={current} previous={previous} reprojected={reprojected}"
                    );
                assert!(
                    reprojected >= current.min(previous) - 1.0e-5
                        && reprojected <= current.max(previous) + 1.0e-5,
                    "reprojection overshot history bounds"
                );
            }
            if frame_delta > 0.30 && history_effect < 1.0e-5 {
                reactive_reset = true;
            }
        }
    }
    assert!(
        stabilized_change,
        "no temporally stable cloud transition was sampled"
    );
    assert!(
        reactive_reset,
        "reactive mask never rejected stale history at a disocclusion"
    );
}

#[test]
fn near_detail_reduces_history_weight_to_preserve_edge_response() {
    let mut history = erosion_test_shadow();
    history.map4[0] = 0.86;
    history.map4[2] = 0.0;
    history.map0 = [0.162, 318.0 / 1000.0, 0.252, 0.655];
    history.map3 = [0.137, 0.291, 0.238, 0.640];
    let mut current_only = history;
    current_only.map4[0] = 0.0;

    let sun = Vec3::new(0.24, 0.80, 0.55).normalize();
    let point = Vec3::new(26.0, 0.0, -11.0);
    let near_camera = point;
    let far_camera = Vec3::new(680.0, 25.0, 590.0);
    let current_near = sample_spatial_cloud_shadow_cpu_at(&current_only, sun, point, near_camera);
    let history_near = sample_spatial_cloud_shadow_cpu_at(&history, sun, point, near_camera);
    let current_far = sample_spatial_cloud_shadow_cpu_at(&current_only, sun, point, far_camera);
    let history_far = sample_spatial_cloud_shadow_cpu_at(&history, sun, point, far_camera);
    let near_history_effect = (history_near - current_near).abs();
    let far_history_effect = (history_far - current_far).abs();
    assert!(
        near_history_effect <= far_history_effect + 1.0e-4,
        "near history is too strong near={near_history_effect} far={far_history_effect}"
    );
}

#[test]
fn temporal_history_resets_after_long_frame_gap() {
    let mut world = newengine_ecs::World::new();
    let mut frame = sample_sky_frame(&test_cycle(), None, Vec3::new(0.18, 0.81, 0.557));
    frame.cloud_advection = Vec2::new(4.5, 1.2);
    let first = update_sky_dynamics(&mut world, &frame, 1.0 / 60.0);
    assert_eq!(first.temporal_history_weight, 0.0);
    let stable = update_sky_dynamics(&mut world, &frame, 1.0 / 60.0);
    assert!(stable.temporal_history_weight > 0.60);
    let stalled = update_sky_dynamics(&mut world, &frame, 0.20);
    assert_eq!(stalled.temporal_history_weight, 0.0);
}

#[test]
fn spatial_runtime_carries_previous_cloud_state_into_ubo_maps() {
    let frame = sample_sky_frame(&test_cycle(), None, Vec3::new(0.18, 0.81, 0.557));
    let dynamics = SkyDynamicsFrame {
        cloud_offset: Vec2::new(0.24, 0.42),
        coverage: 0.58,
        softness: 0.65,
        shadow_strength: 0.61,
        haze: 0.16,
        evolution_phase: 0.31,
        lifecycle: 0.68,
        gust_factor: 1.02,
        previous_cloud_offset: Vec2::new(0.232, 0.416),
        previous_evolution_phase: 0.306,
        previous_lifecycle: 0.674,
        temporal_history_weight: 0.79,
        sun_occlusion: CloudSunOcclusionRuntime::default(),
    };
    let spatial = spatial_cloud_shadow_from_dynamics(&frame, &dynamics);
    assert_eq!(spatial.map3[0], dynamics.previous_cloud_offset.x);
    assert_eq!(spatial.map3[1], dynamics.previous_cloud_offset.y);
    assert_eq!(spatial.map3[2], dynamics.previous_evolution_phase);
    assert_eq!(spatial.map3[3], dynamics.previous_lifecycle);
    assert_eq!(spatial.map4[0], dynamics.temporal_history_weight);
    assert!((0.016..=0.034).contains(&spatial.map4[1]));
    assert!((0.05..=0.15).contains(&spatial.map4[2]));
    assert!((72.0..=150.0).contains(&spatial.map4[3]));
}

#[test]
fn sky_postfx_sanitizer_rejects_extreme_authoring_values() {
    let sanitized = sky_postfx_sanitize(SkyPostFxRuntime {
        exposure: f32::INFINITY,
        gamma: -10.0,
        black_lift: 4.0,
        saturation: -3.0,
        contrast: 9.0,
        temperature: 7.0,
        vignette_strength: 5.0,
        local_contrast_strength: 4.0,
        dither_strength: 12.0,
        bloom_threshold: -1.0,
        bloom_knee: 9.0,
        bloom_intensity: 7.0,
        bloom_radius: 9.0,
        sun_glare_scale: 10.0,
        sun_ray_scale: -5.0,
    });
    assert!(sanitized.exposure.is_finite());
    assert_eq!(sanitized.gamma, 1.8);
    assert_eq!(sanitized.black_lift, 0.035);
    assert_eq!(sanitized.bloom_intensity, 0.30);
    assert_eq!(sanitized.sun_ray_scale, 0.0);
}
