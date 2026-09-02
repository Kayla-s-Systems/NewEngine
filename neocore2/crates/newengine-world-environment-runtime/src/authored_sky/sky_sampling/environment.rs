use super::*;

pub(crate) fn sample_sky_frame_from_environment(
    cycle: &SkyCycleRuntime,
    environment: &newengine_world_environment_api::EnvironmentFrameDto,
) -> SkyFrameSample {
    let to_sun = env_vec_to_vec3(
        environment.celestial.sun.direction_world,
        Vec3::new(0.0, 1.0, 0.0),
    );
    let render = &environment.consumer_packets.render;
    let day_strength = (render.sun_intensity_hint / 105_000.0).clamp(0.0, 1.0);
    let moon_strength = (render.moon_intensity_hint / 0.25).clamp(0.0, 1.0);
    let overcast = environment.sky.overcast_blend.clamp(0.0, 1.0);
    let weather_intensity = environment.weather.intensity.clamp(0.0, 1.0);
    let preset = sky_cloud_visual_preset(environment.weather.state);
    let preset_blend = (0.24 + weather_intensity * 0.66 + overcast * 0.10).clamp(0.0, 1.0);
    let cloud_profile = cycle.cloud_profile.trim().to_ascii_lowercase();
    let force_cloudless = matches!(
        cloud_profile.as_str(),
        "clear" | "cloudless" | "clear_sky" | "clear-sky" | "none"
    );
    let haze =
        (environment.atmosphere.haze_amount + preset.haze_bias * preset_blend).clamp(0.0, 1.0);
    let overcast_loss = 1.0 - overcast * 0.32;
    let sky_rgb = sky_mul3(
        sky_lerp3(
            env_color_to_rgb(environment.sky.zenith_color_linear),
            env_color_to_rgb(environment.sky.horizon_color_linear),
            0.36 + environment.sky.dusk_dawn_blend.clamp(0.0, 1.0) * 0.30,
        ),
        overcast_loss,
    );
    let phase_tint = match environment.time_of_day_state.phase {
        newengine_world_environment_api::TimeOfDayPhase::Dawn => [1.06, 0.96, 0.88],
        newengine_world_environment_api::TimeOfDayPhase::Dusk => [1.08, 0.93, 0.84],
        newengine_world_environment_api::TimeOfDayPhase::Night => [0.88, 0.94, 1.10],
        newengine_world_environment_api::TimeOfDayPhase::Day => [1.0, 1.0, 1.0],
    };
    let sky_phase_weight = (environment.sky.dusk_dawn_blend * 0.20
        + environment.sky.night_blend * 0.10)
        .clamp(0.0, 0.24);
    let sky_rgb = sky_mul3_components(
        sky_rgb,
        sky_lerp3([1.0, 1.0, 1.0], phase_tint, sky_phase_weight),
    );

    let cloud_base_rgb = sky_mul3(
        sky_lerp3(
            env_color_to_rgb(environment.sky.horizon_color_linear),
            env_color_to_rgb(environment.sky.sun_horizon_color_linear),
            environment.sky.dusk_dawn_blend.clamp(0.0, 1.0) * 0.52,
        ),
        (0.76 + day_strength * 0.42 - environment.clouds.light_absorption * 0.28).clamp(0.05, 1.25),
    );
    let preset_tint = sky_lerp3(
        preset.night_tint,
        preset.day_tint,
        environment.time_of_day_state.day_blend.clamp(0.0, 1.0),
    );
    let cloud_rgb = sky_mul3_components(
        cloud_base_rgb,
        sky_lerp3([1.0, 1.0, 1.0], preset_tint, preset_blend * 0.62),
    );
    let sun_color = sky_lerp3(
        env_color_to_rgb(environment.celestial.moon.color_linear),
        env_color_to_rgb(environment.celestial.sun.color_linear),
        day_strength.max(environment.sky.dusk_dawn_blend * 0.28),
    );
    let absorption = environment.clouds.light_absorption.clamp(0.0, 1.0);
    let dusk = environment.sky.dusk_dawn_blend.clamp(0.0, 1.0);
    // Convert the provider's physically-inspired lux hints into the compact
    // renderer light range. A daylight floor is deliberate: overcast removes
    // directional contrast, not all incident energy. This prevents forest
    // materials from collapsing to black under fair/overcast transitions.
    let daylight_curve = day_strength.powf(0.72);
    let sun_intensity =
        cycle.base_sun_intensity * (0.10 + daylight_curve * 0.90) * (1.0 - absorption * 0.48)
            + cycle.base_sun_intensity * 0.020 * moon_strength
            + cycle.base_sun_intensity * 0.070 * dusk;
    let ambient_color = sky_lerp3(
        [0.020, 0.028, 0.066],
        cycle.base_ambient_color,
        (day_strength.powf(0.55) + dusk * 0.38).clamp(0.0, 1.0),
    );
    let sky_light = environment
        .lighting_intent
        .sky_light_intensity
        .clamp(0.0, 1.0);
    let storm_darkening = environment.exposure_intent.storm_darkening.clamp(0.0, 0.75);
    let ambient_intensity = cycle.base_ambient_intensity
        * (0.11 + day_strength.powf(0.58) * 0.82 + sky_light * 0.55 + overcast * 0.12)
        * (1.0 - storm_darkening * 0.55);
    let cloud_coverage = if force_cloudless {
        0.0
    } else {
        environment.clouds.coverage.clamp(0.0, 1.0)
    };
    // Do not pre-blur the entire cloud field. The shader now owns edge
    // erosion/penumbra while this value describes meteorological morphology.
    let baseline_softness = (0.78 - overcast * 0.22).clamp(0.38, 0.86);
    let cloud_softness = (baseline_softness + (preset.softness - baseline_softness) * preset_blend)
        .clamp(0.34, 0.94);
    let cloud_shadow_strength = if force_cloudless {
        0.0
    } else {
        (environment.clouds.shadow_strength * (1.0 + (preset.shadow_scale - 1.0) * preset_blend))
            .clamp(0.0, 1.0)
    };
    let low_layer = environment.clouds.layers.first();
    let high_layer = environment.clouds.layers.get(1);
    let cloud_base_altitude_m = low_layer
        .map(|layer| layer.altitude_min_meters)
        .unwrap_or(1250.0)
        .clamp(400.0, 4500.0);
    let cloud_thickness_m = low_layer
        .map(|layer| layer.altitude_max_meters - layer.altitude_min_meters)
        .unwrap_or(1100.0)
        .clamp(300.0, 7600.0);
    let cloud_layer_density = if force_cloudless {
        0.0
    } else {
        low_layer
            .map(|layer| layer.density)
            .unwrap_or(cloud_coverage * 0.36)
            .clamp(0.0, 1.0)
    };
    let high_cloud_coverage = if force_cloudless {
        0.0
    } else {
        high_layer
            .map(|layer| layer.coverage)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0)
    };
    let high_cloud_density = if force_cloudless {
        0.0
    } else {
        high_layer
            .map(|layer| layer.density)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0)
    };
    let air_density = environment.atmosphere.air_density_kg_m3.clamp(0.55, 1.55);
    let air_density_scale = (air_density / 1.225).clamp(0.55, 1.35).powf(0.55);
    let precipitable_water = environment
        .atmosphere
        .precipitable_water_mm
        .clamp(0.2, 75.0);
    let water_column_scale = (precipitable_water / 25.0).clamp(0.02, 3.0);
    let cloud_water_path = environment
        .atmosphere
        .cloud_water_path_kg_m2
        .clamp(0.0, 2.8);
    let adv = environment.wind.cloud_advection;
    SkyFrameSample {
        to_sun,
        sky_tint: sky_color_to_rgba(sky_clamp3(sky_rgb, 0.0, 2.5)),
        cloud_tint: sky_color_to_rgba(sky_clamp3(cloud_rgb, 0.0, 2.5)),
        sun_color: sky_clamp3(sun_color, 0.0, 1.25),
        sun_intensity: sun_intensity.max(0.0),
        ambient_color: sky_clamp3(ambient_color, 0.0, 1.0),
        ambient_intensity: ambient_intensity.max(0.0),
        cloud_coverage,
        cloud_softness,
        cloud_shadow_strength,
        haze_amount: haze,
        cloud_advection: Vec2::new(adv.x, adv.z),
        cloud_field_seed: environment.global.environment_seed,
        cloud_world_time_seconds: environment.world_time_seconds,
        rayleigh_strength: ((1.08 - haze * 0.22)
            * air_density_scale
            * (1.0 + (preset.rayleigh_scale - 1.0) * preset_blend))
            .clamp(0.42, 1.35),
        mie_strength: ((0.50 + haze * 1.65 + overcast * 0.25)
            * (0.94 + water_column_scale.sqrt() * 0.10)
            * (1.0 + (preset.mie_scale - 1.0) * preset_blend))
            .clamp(0.30, 3.10),
        star_intensity: (environment.sky.night_blend
            * (1.0 - environment.sky.light_pollution.clamp(0.0, 1.0))
            * (1.0 - overcast * 0.82))
            .clamp(0.0, 1.0),
        cloud_gust_strength: environment.wind.gust_strength.clamp(0.0, 1.0),
        cloud_overcast: overcast,
        cloud_light_absorption: if force_cloudless {
            0.0
        } else {
            environment
                .clouds
                .light_absorption
                .max(cloud_water_path * 0.22)
                .clamp(0.0, 1.0)
        },
        cloud_base_altitude_m,
        cloud_thickness_m,
        cloud_layer_density,
        high_cloud_coverage,
        high_cloud_density,
        humidity: environment.atmosphere.humidity.clamp(0.0, 1.0),
        aerosol_density: environment.atmosphere.aerosol_density.clamp(0.0, 2.0),
        precipitation_intensity: environment.weather.precipitation.intensity.clamp(0.0, 1.0),
    }
}
