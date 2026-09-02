pub(super) fn update_sky_dynamics(
    world: &mut newengine_ecs::World,
    frame: &SkyFrameSample,
    dt: f32,
) -> SkyDynamicsFrame {
    if world.resource::<SkyDynamicsRuntime>().is_none() {
        world.insert_resource(SkyDynamicsRuntime::default());
    }
    let raw_dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
    let dt = raw_dt.clamp(0.0, 0.25);
    let dynamics = world
        .resource_mut::<SkyDynamicsRuntime>()
        .expect("SkyDynamicsRuntime inserted immediately above");

    let previous_cloud_offset = dynamics.cloud_offset;
    let previous_evolution_phase = dynamics.evolution_phase;
    let previous_lifecycle = sky_lifecycle_value(dynamics.lifecycle_phase);

    let target_wind = if frame.cloud_advection.is_finite() {
        frame.cloud_advection
    } else {
        Vec2::ZERO
    };
    let first_update = !dynamics.initialized;
    if first_update {
        dynamics.initialized = true;
        dynamics.smoothed_wind = target_wind;
        dynamics.smoothed_coverage = frame.cloud_coverage.clamp(0.0, 1.0);
        dynamics.smoothed_softness = frame.cloud_softness.clamp(0.04, 0.98);
        dynamics.smoothed_shadow = frame.cloud_shadow_strength.clamp(0.0, 1.0);
        dynamics.smoothed_haze = frame.haze_amount.clamp(0.0, 1.0);
        dynamics.smoothed_cloud_base_altitude_m = frame.cloud_base_altitude_m.clamp(400.0, 4500.0);
        dynamics.smoothed_cloud_thickness_m = frame.cloud_thickness_m.clamp(300.0, 7600.0);
        dynamics.smoothed_cloud_layer_density = frame.cloud_layer_density.clamp(0.0, 1.0);
        dynamics.smoothed_high_cloud_coverage = frame.high_cloud_coverage.clamp(0.0, 1.0);
        dynamics.smoothed_high_cloud_density = frame.high_cloud_density.clamp(0.0, 1.0);
        dynamics.smoothed_humidity = frame.humidity.clamp(0.0, 1.0);
        dynamics.smoothed_aerosol_density = frame.aerosol_density.clamp(0.0, 2.0);
        dynamics.smoothed_precipitation_intensity = frame.precipitation_intensity.clamp(0.0, 1.0);
        dynamics.cloud_offset = sky_cloud_seeded_offset(frame, target_wind);

        let initial_wind_speed = target_wind.length().clamp(0.0, 24.0);
        let initial_overcast = frame.cloud_overcast.clamp(0.0, 1.0);
        let initial_absorption = frame.cloud_light_absorption.clamp(0.0, 1.0);
        let evolution_rate = 0.0022 + initial_wind_speed * 0.00018 + initial_overcast * 0.0011;
        let lifecycle_rate = 0.00085 + initial_overcast * 0.00075 + initial_absorption * 0.00045;
        dynamics.evolution_phase = sky_cloud_seeded_phase(
            frame.cloud_field_seed,
            2,
            frame.cloud_world_time_seconds,
            evolution_rate,
        );
        dynamics.lifecycle_phase = sky_cloud_seeded_phase(
            frame.cloud_field_seed,
            3,
            frame.cloud_world_time_seconds,
            lifecycle_rate,
        );
        dynamics.gust_phase = sky_cloud_seed_unit(frame.cloud_field_seed, 4) * TAU;
    }

    let wind_alpha = sky_exp_alpha(dt, 7.5);
    let weather_alpha = sky_exp_alpha(dt, 34.0);
    let optical_alpha = sky_exp_alpha(dt, 12.0);
    dynamics.smoothed_wind += (target_wind - dynamics.smoothed_wind) * wind_alpha;

    // Cloud fraction is a transported meteorological quantity, not a UI slider.
    // Let a new deck arrive as a front over ~1-3 minutes instead of allowing a
    // discontinuous provider target to fill the sky in a few seconds. Stronger
    // wind/overcast may move the front faster, but it is still rate limited.
    let target_coverage = frame.cloud_coverage.clamp(0.0, 1.0);
    let pre_update_wind_speed = dynamics.smoothed_wind.length().clamp(0.0, 24.0);
    let coverage_is_growing = target_coverage >= dynamics.smoothed_coverage;
    let coverage_response = if coverage_is_growing {
        (70.0 - pre_update_wind_speed * 1.10 - frame.cloud_overcast.clamp(0.0, 1.0) * 18.0)
            .clamp(38.0, 70.0)
    } else {
        (48.0 - pre_update_wind_speed * 0.70).clamp(28.0, 48.0)
    };
    let coverage_max_rate = if coverage_is_growing {
        (0.006 + pre_update_wind_speed * 0.00025 + frame.cloud_overcast.clamp(0.0, 1.0) * 0.003)
            .clamp(0.006, 0.015)
    } else {
        (0.009 + pre_update_wind_speed * 0.00030).clamp(0.009, 0.018)
    };
    dynamics.smoothed_coverage = sky_rate_limited_exp_step(
        dynamics.smoothed_coverage,
        target_coverage,
        dt,
        coverage_response,
        coverage_max_rate,
    )
    .clamp(0.0, 1.0);
    dynamics.smoothed_softness +=
        (frame.cloud_softness.clamp(0.04, 0.98) - dynamics.smoothed_softness) * weather_alpha;
    dynamics.smoothed_shadow +=
        (frame.cloud_shadow_strength.clamp(0.0, 1.0) - dynamics.smoothed_shadow) * optical_alpha;
    dynamics.smoothed_haze +=
        (frame.haze_amount.clamp(0.0, 1.0) - dynamics.smoothed_haze) * optical_alpha;

    // Vertical cloud geometry and atmospheric moisture have their own inertia.
    // A weather state may change immediately at the control plane, but the real
    // deck must lift, deepen and saturate over tens of seconds rather than jump.
    dynamics.smoothed_cloud_base_altitude_m = sky_rate_limited_exp_step(
        dynamics.smoothed_cloud_base_altitude_m,
        frame.cloud_base_altitude_m.clamp(400.0, 4500.0),
        dt,
        62.0,
        15.0,
    )
    .clamp(400.0, 4500.0);
    dynamics.smoothed_cloud_thickness_m = sky_rate_limited_exp_step(
        dynamics.smoothed_cloud_thickness_m,
        frame.cloud_thickness_m.clamp(300.0, 7600.0),
        dt,
        48.0,
        24.0,
    )
    .clamp(300.0, 7600.0);
    dynamics.smoothed_cloud_layer_density = sky_rate_limited_exp_step(
        dynamics.smoothed_cloud_layer_density,
        frame.cloud_layer_density.clamp(0.0, 1.0),
        dt,
        34.0,
        0.020,
    )
    .clamp(0.0, 1.0);
    dynamics.smoothed_high_cloud_coverage = sky_rate_limited_exp_step(
        dynamics.smoothed_high_cloud_coverage,
        frame.high_cloud_coverage.clamp(0.0, 1.0),
        dt,
        58.0,
        0.014,
    )
    .clamp(0.0, 1.0);
    dynamics.smoothed_high_cloud_density = sky_rate_limited_exp_step(
        dynamics.smoothed_high_cloud_density,
        frame.high_cloud_density.clamp(0.0, 1.0),
        dt,
        52.0,
        0.016,
    )
    .clamp(0.0, 1.0);
    dynamics.smoothed_humidity = sky_rate_limited_exp_step(
        dynamics.smoothed_humidity,
        frame.humidity.clamp(0.0, 1.0),
        dt,
        90.0,
        0.010,
    )
    .clamp(0.0, 1.0);
    dynamics.smoothed_aerosol_density = sky_rate_limited_exp_step(
        dynamics.smoothed_aerosol_density,
        frame.aerosol_density.clamp(0.0, 2.0),
        dt,
        76.0,
        0.018,
    )
    .clamp(0.0, 2.0);
    dynamics.smoothed_precipitation_intensity = sky_rate_limited_exp_step(
        dynamics.smoothed_precipitation_intensity,
        frame.precipitation_intensity.clamp(0.0, 1.0),
        dt,
        14.0,
        0.070,
    )
    .clamp(0.0, 1.0);

    let wind_speed = dynamics.smoothed_wind.length().clamp(0.0, 24.0);
    let gust_strength = frame.cloud_gust_strength.clamp(0.0, 1.0);
    dynamics.gust_phase = (dynamics.gust_phase
        + dt * (0.18 + wind_speed * 0.035 + gust_strength * 0.22))
        .rem_euclid(TAU);
    let gust_wave = (dynamics.gust_phase.sin() * 0.56
        + (dynamics.gust_phase * 2.17 + 0.73).sin() * 0.29
        + (dynamics.gust_phase * 4.03 + 2.10).sin() * 0.15)
        .clamp(-1.0, 1.0);
    let gust_factor = (1.0 + gust_strength * gust_wave * 0.42).clamp(0.55, 1.55);

    // Integrate wind velocity rather than multiplying the current wind by total
    // elapsed time. This prevents visible cloud teleporting when the weather
    // provider changes direction or speed.
    dynamics.cloud_offset +=
        dynamics.smoothed_wind * (dt * SKY_CLOUD_ADVECTION_COORDS_PER_METER * gust_factor);
    // Keep the phase bounded without the visible 0..1 discontinuity that
    // appears when non-integer octave coefficients are used by the cloud field.
    dynamics.cloud_offset.x = dynamics.cloud_offset.x.rem_euclid(1024.0);
    dynamics.cloud_offset.y = dynamics.cloud_offset.y.rem_euclid(1024.0);

    let overcast = frame.cloud_overcast.clamp(0.0, 1.0);
    let absorption = frame.cloud_light_absorption.clamp(0.0, 1.0);
    dynamics.evolution_phase = (dynamics.evolution_phase
        + dt * (0.0022 + wind_speed * 0.00018 + overcast * 0.0011))
        .rem_euclid(1.0);
    dynamics.lifecycle_phase = (dynamics.lifecycle_phase
        + dt * (0.00085 + overcast * 0.00075 + absorption * 0.00045))
        .rem_euclid(1.0);
    let lifecycle = sky_lifecycle_value(dynamics.lifecycle_phase);

    let coverage = (dynamics.smoothed_coverage + (lifecycle - 0.5) * 0.085).clamp(0.0, 1.0);
    let softness = (dynamics.smoothed_softness + (gust_factor - 1.0) * 0.035).clamp(0.04, 0.98);
    let raw_sun_occlusion = sky_cloud_sun_density(
        frame,
        coverage,
        softness,
        dynamics.cloud_offset,
        dynamics.evolution_phase,
        lifecycle,
    );
    if first_update {
        // The first rendered frame must represent the actual weather state. A
        // fade from clear sky to the existing cloud cover creates a physically
        // incorrect launch flash and temporarily desynchronizes sky and world light.
        dynamics.smoothed_sun_occlusion = raw_sun_occlusion;
    } else {
        // A real cloud edge can cross the 0.53 degree solar disc in seconds, but
        // not in a single frame because a global coverage target changed. Keep
        // local crossings responsive while preserving finite optical inertia.
        let incoming = raw_sun_occlusion > dynamics.smoothed_sun_occlusion;
        let occlusion_response = if incoming {
            (4.2 - wind_speed * 0.075).clamp(2.4, 4.2)
        } else {
            (5.4 - wind_speed * 0.090).clamp(3.0, 5.4)
        };
        let max_occlusion_rate = if incoming { 0.30 } else { 0.24 };
        dynamics.smoothed_sun_occlusion = sky_rate_limited_exp_step(
            dynamics.smoothed_sun_occlusion,
            raw_sun_occlusion,
            dt,
            occlusion_response,
            max_occlusion_rate,
        )
        .clamp(0.0, 1.0);
    }
    let sun_occlusion =
        sky_cloud_occlusion_from_density(frame, raw_sun_occlusion, dynamics.smoothed_sun_occlusion);
    let offset_delta = (dynamics.cloud_offset - previous_cloud_offset).length();
    let evolution_delta = sky_phase_distance(dynamics.evolution_phase, previous_evolution_phase);
    let lifecycle_delta = (lifecycle - previous_lifecycle).abs();
    let temporal_history_weight = sky_temporal_history_weight(
        first_update,
        raw_dt,
        offset_delta,
        evolution_delta,
        lifecycle_delta,
    );
    let previous_cloud_offset = if first_update {
        dynamics.cloud_offset
    } else {
        previous_cloud_offset
    };
    let previous_evolution_phase = if first_update {
        dynamics.evolution_phase
    } else {
        previous_evolution_phase
    };
    let previous_lifecycle = if first_update {
        lifecycle
    } else {
        previous_lifecycle
    };

    SkyDynamicsFrame {
        cloud_offset: dynamics.cloud_offset,
        coverage,
        softness,
        shadow_strength: dynamics.smoothed_shadow,
        haze: dynamics.smoothed_haze,
        evolution_phase: dynamics.evolution_phase,
        lifecycle,
        gust_factor,
        previous_cloud_offset,
        previous_evolution_phase,
        previous_lifecycle,
        temporal_history_weight,
        sun_occlusion,
    }
}
