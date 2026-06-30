use crate::celestial::{moon_body, moon_phase, sun_body, time_of_day_state};
use crate::consumer_packets::build_consumer_packets;
use crate::math::{clamp01_f32, mix_color, normalize, unit_noise};
use crate::phenomena::{build_environment_objects, environment_object_cells};
use crate::profile_catalog::{profile_by_id, EnvironmentProfileDescriptor};
use crate::weather_profile::{enrich_weather_tags, evaluate_weather};
use newengine_world_environment_api::{
    AtmosphereStateDto, CelestialStateDto, CloudLayerDto, CloudStateDto, Color3Dto,
    EnvironmentDiagnosticsDto, EnvironmentFrameDto, EnvironmentFrameRequest,
    EnvironmentGameplayModifiersDto, EnvironmentGlobalStateDto, EnvironmentLightingIntentDto,
    EnvironmentObjectKind, EnvironmentVisualAssetRefsDto, ExposureIntentDto, SkyStateDto, Vec3Dto,
    WindStateDto,
};

pub(crate) fn build_default_environment_frame(
    provider: &str,
    provider_route: &str,
    req: EnvironmentFrameRequest,
) -> EnvironmentFrameDto {
    let tod = normalized_day_from_time(&req);
    let day_index_u64 = req.time.game.day_index;
    let day_index = day_index_u64.min(u32::MAX as u64) as u32;
    let world_time_seconds = req.time.game.day_index as f64
        * req.time.game.seconds_per_game_day.max(1.0)
        + req.time.game.seconds_of_day.max(0.0);

    let requested_profile_id = normalized_profile_id(&req);
    let (profile, profile_found) = profile_by_id(&requested_profile_id);
    let time_of_day_state = time_of_day_state(tod);
    let sun = sun_body(tod);
    let moon = moon_body(tod, req.seed, day_index_u64);
    let cloud_seed = unit_noise(req.seed, day_index_u64, 0xC10D_0001);
    let pressure = weather_pressure(req.seed, day_index_u64, tod);
    let weather_eval =
        evaluate_weather(profile, tod, pressure, cloud_seed, time_of_day_state.phase);
    let visual_assets = profile.visual_assets;
    let mut weather = weather_eval.weather.clone();

    let baseline_coverage = baseline_cloud_coverage(req.seed, day_index_u64, tod, profile);
    let cloud_coverage = baseline_coverage
        .max(weather_eval.cloud_floor)
        .clamp(0.0, 1.0);
    let overcast = clamp01_f32(
        (cloud_coverage - 0.55) * 1.9 + weather.intensity * 0.20 + weather_eval.overcast_bias,
    );
    let precipitation = weather.precipitation.intensity;
    let fog_weather = weather_eval.fog_bias * weather.intensity;
    let haze = 0.04
        + 0.10 * time_of_day_state.dawn_dusk_blend
        + 0.08 * cloud_coverage
        + 0.22 * precipitation
        + weather_eval.haze_bias
        + 0.16 * fog_weather;
    let visibility =
        (20_000.0 * weather_eval.visibility_factor * (1.0 - overcast * 0.34) * (1.0 - haze * 0.45))
            .max(120.0);

    enrich_weather_tags(
        &mut weather,
        time_of_day_state.phase,
        visibility,
        cloud_coverage,
    );

    let sky = SkyStateDto {
        zenith_color_linear: mix_color(
            Color3Dto::new(0.010, 0.014, 0.035),
            Color3Dto::new(0.18, 0.34, 0.62),
            time_of_day_state.day_blend,
        ),
        horizon_color_linear: mix_color(
            Color3Dto::new(0.020, 0.026, 0.058),
            Color3Dto::new(0.48, 0.62, 0.84),
            time_of_day_state.day_blend,
        ),
        sun_horizon_color_linear: mix_color(
            Color3Dto::new(0.14, 0.07, 0.04),
            Color3Dto::new(1.0, 0.48, 0.18),
            time_of_day_state.dawn_dusk_blend,
        ),
        opposite_horizon_color_linear: mix_color(
            Color3Dto::new(0.018, 0.030, 0.070),
            Color3Dto::new(0.32, 0.45, 0.68),
            time_of_day_state.day_blend,
        ),
        dusk_dawn_blend: time_of_day_state.dawn_dusk_blend,
        night_blend: time_of_day_state.night_blend,
        overcast_blend: overcast,
        light_pollution: 0.04 * time_of_day_state.night_blend,
    };

    let atmosphere = AtmosphereStateDto {
        fog_density: 0.006
            + overcast * 0.024
            + precipitation * 0.020
            + weather_eval.fog_bias * weather.intensity,
        fog_height_falloff: 0.12,
        fog_color_linear: mix_color(
            Color3Dto::new(0.06, 0.07, 0.11),
            Color3Dto::new(0.56, 0.62, 0.70),
            time_of_day_state.day_blend,
        ),
        haze_amount: haze,
        humidity: clamp01_f32(
            0.26 + cloud_coverage * 0.30 + precipitation * 0.34 + weather_eval.fog_bias * 0.28,
        ),
        aerosol_density: 0.08 + haze,
        visibility_distance_meters: visibility,
    };

    let wind = WindStateDto {
        global_direction: normalize(Vec3Dto::new(0.92, 0.0, 0.38)),
        global_speed_mps: weather_eval.wind_base_mps
            + cloud_coverage * 1.6
            + weather_eval.wind_gain_mps * weather.intensity,
        gust_strength: (weather_eval.gust_base
            + weather_eval.gust_gain * weather.intensity
            + overcast * 0.12)
            .clamp(0.0, 1.0),
        cloud_advection: Vec3Dto::new(
            weather_eval.wind_base_mps
                + cloud_coverage * 1.8
                + weather_eval.wind_gain_mps * weather.intensity,
            0.0,
            0.8 + weather.intensity * 1.4,
        ),
    };

    let activation = (cloud_coverage * 0.55 + weather.intensity * 0.45).clamp(0.0, 1.0);
    let environment_objects = build_environment_objects(
        &req,
        profile,
        weather_eval.pattern,
        activation,
        cloud_coverage,
        &weather,
        atmosphere.fog_density,
        &wind,
    );
    let cloud_volumes = environment_objects
        .iter()
        .filter(|it| {
            matches!(
                it.kind,
                EnvironmentObjectKind::CloudVolume
                    | EnvironmentObjectKind::CloudField
                    | EnvironmentObjectKind::FogBank
                    | EnvironmentObjectKind::SnowBand
                    | EnvironmentObjectKind::DustWall
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let storm_cells = environment_objects
        .iter()
        .filter(|it| {
            matches!(
                it.kind,
                EnvironmentObjectKind::StormCell | EnvironmentObjectKind::WeatherFront
            )
        })
        .cloned()
        .collect::<Vec<_>>();

    let clouds = CloudStateDto {
        coverage: cloud_coverage,
        overcast,
        shadow_strength: clamp01_f32(cloud_coverage * 0.46 + weather.intensity * 0.20),
        light_absorption: clamp01_f32(
            cloud_coverage * 0.28 + overcast * 0.24 + precipitation * 0.16,
        ),
        layers: cloud_layers(
            profile,
            cloud_coverage,
            overcast,
            pressure,
            precipitation,
            &wind,
        ),
        volumes: cloud_volumes,
        storm_cells,
    };

    let lighting_intent = EnvironmentLightingIntentDto {
        sun_lux_hint: sun.intensity_lux_hint * (1.0 - clouds.light_absorption),
        moon_lux_hint: moon.intensity_lux_hint * (1.0 - clouds.light_absorption),
        ambient_intensity: (0.04 + time_of_day_state.day_blend * 0.22 + cloud_coverage * 0.06
            - weather.intensity * 0.025)
            .max(0.015),
        sky_light_intensity: (0.07 + time_of_day_state.day_blend * 0.45 - overcast * 0.12)
            .max(0.02),
        cloud_shadow_strength: clouds.shadow_strength,
        wetness_specular_boost: weather.wetness.surface_wetness * 0.55,
    };

    let gameplay_modifiers = EnvironmentGameplayModifiersDto {
        visibility_multiplier: clamp01_f32(visibility / 20_000.0),
        audio_masking_multiplier: clamp01_f32(
            precipitation * 0.55 + weather.thunder.probability * 0.30 + wind.gust_strength * 0.12,
        ),
        weather_hazard_level: clamp01_f32(
            weather.thunder.probability * 0.85
                + precipitation * 0.25
                + weather_eval.fog_bias * 0.18,
        ),
        shelter_score: clamp01_f32(
            precipitation * 0.60
                + weather.thunder.probability * 0.95
                + weather.snow.surface_snow * 0.25,
        ),
        surface_slipperiness_hint: clamp01_f32(
            weather.wetness.surface_wetness * 0.82 + weather.snow.surface_snow * 0.30,
        ),
    };

    let exposure_intent = ExposureIntentDto {
        night_adaptation_hint: time_of_day_state.night_blend,
        storm_darkening: weather.thunder.probability * 0.65 + overcast * 0.16,
        sun_glare_hint: sun.intensity_lux_hint / 105_000.0 * (1.0 - cloud_coverage * 0.75),
        interior_exterior_bias: 0.0,
    };

    let affected_cells = environment_object_cells(&req);
    let consumer_packets = build_consumer_packets(
        profile,
        weather_eval.pattern,
        &time_of_day_state,
        &sun,
        &moon,
        &atmosphere,
        &weather,
        &clouds,
        &wind,
        &lighting_intent,
        &gameplay_modifiers,
        &exposure_intent,
        affected_cells,
    );

    let key = deterministic_key(&req, provider);
    let environment_object_count = environment_objects.len();
    EnvironmentFrameDto {
        frame_id: req.frame_id,
        world_instance_id: req.world_instance_id,
        world_time_seconds,
        time_of_day_normalized: tod,
        day_index,
        time_of_day_state,
        global: EnvironmentGlobalStateDto {
            active_region: req.active_region.or_else(|| Some(profile.region.to_owned())),
            active_biome: req.active_biome.or_else(|| Some(profile.biome.to_owned())),
            active_weather_profile: weather.weather_id.clone(),
            active_environment_profile: profile.id.to_owned(),
            weather_table_ref: profile.weather_table_ref.to_owned(),
            sky_profile_ref: profile.sky_profile_ref.to_owned(),
            cloud_profile_ref: profile.cloud_profile_ref.to_owned(),
            wind_profile_ref: profile.wind_profile_ref.to_owned(),
            environment_seed: req.seed,
        },
        visual_assets: EnvironmentVisualAssetRefsDto {
            visual_group_id: visual_assets.id.to_owned(),
            texture_dictionary_ref: visual_assets.texture_dictionary_ref.to_owned(),
            sky_texture_ref: visual_assets.sky_texture_ref.to_owned(),
            starfield_texture_ref: visual_assets.starfield_texture_ref.to_owned(),
            cloud_field_ref: visual_assets.cloud_density_texture_ref.to_owned(),
            cloud_density_texture_ref: visual_assets.cloud_density_texture_ref.to_owned(),
            cloud_detail_texture_ref: visual_assets.cloud_detail_texture_ref.to_owned(),
            cloud_dither_texture_ref: visual_assets.cloud_dither_texture_ref.to_owned(),
            sun_disk_texture_ref: visual_assets.sun_disk_texture_ref.to_owned(),
            moon_disk_texture_ref: visual_assets.moon_disk_texture_ref.to_owned(),
            weather_table_ref: profile.weather_table_ref.to_owned(),
            weather_visual_ref: weather_eval.pattern.weather_visual_ref.to_owned(),
        },
        celestial: CelestialStateDto {
            sun,
            moon,
            moon_phase: moon_phase(req.seed, day_index_u64),
            stars_visibility: time_of_day_state.night_blend * (1.0 - cloud_coverage * 0.75),
            night_sky_visibility: time_of_day_state.night_blend * (1.0 - cloud_coverage * 0.65),
        },
        sky,
        atmosphere,
        weather,
        clouds,
        wind,
        lighting_intent,
        gameplay_modifiers,
        exposure_intent,
        environment_objects,
        consumer_packets,
        diagnostics: EnvironmentDiagnosticsDto {
            provider: provider.to_owned(),
            provider_route: provider_route.to_owned(),
            degraded: false,
            deterministic_key: key,
            active_profile: profile.id.to_owned(),
            reasons: vec![
                format!("profile={} profile_found={} weather_table={}", profile.id, profile_found, profile.weather_table_ref),
                format!("time_of_day phase={:?} normalized={:.4}", time_of_day_state.phase, tod),
                format!("weather_pattern={} intensity={:.3} coverage={:.3}", weather_eval.pattern.id, weather_eval.weather.intensity, cloud_coverage),
                format!("visual_assets group='{}' dictionary='{}' sky='{}' sun='{}' moon='{}' cloud_density='{}'", visual_assets.id, visual_assets.texture_dictionary_ref, visual_assets.sky_texture_ref, visual_assets.sun_disk_texture_ref, visual_assets.moon_disk_texture_ref, visual_assets.cloud_density_texture_ref),
                format!("environment_objects={}", environment_object_count),
                "engine.time provides clock authority".to_owned(),
                "weather is selected from profile table, not string substring branches".to_owned(),
                "engine.world.environment resolves environmental meaning".to_owned(),
                "engine.render remains a consumer of resolved packets".to_owned(),
            ],
            warnings: profile_warning(profile_found, &requested_profile_id),
        },
    }
}

pub(crate) fn deterministic_key(req: &EnvironmentFrameRequest, provider: &str) -> String {
    format!(
        "{}:{}:{}:{:.6}:{}",
        provider,
        req.world_instance_id,
        req.seed,
        normalized_day_from_time(req),
        req.environment_profile.profile_id
    )
}

pub(crate) fn normalized_day_from_time(req: &EnvironmentFrameRequest) -> f32 {
    let normalized = req.time.game.normalized_day;
    if normalized.is_finite() && (0.0..=1.0).contains(&normalized) {
        return normalized as f32;
    }
    let seconds_per_day = req.time.game.seconds_per_game_day.max(1.0);
    let seconds = req.time.game.seconds_of_day.rem_euclid(seconds_per_day);
    (seconds / seconds_per_day) as f32
}

fn normalized_profile_id(req: &EnvironmentFrameRequest) -> String {
    let trimmed = req.environment_profile.profile_id.trim();
    if trimmed.is_empty() {
        "environment.default".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn profile_warning(found: bool, requested: &str) -> Vec<String> {
    if found {
        Vec::new()
    } else {
        vec![format!(
            "unknown environment profile '{}' routed to descriptor 'environment.default'",
            requested
        )]
    }
}

fn weather_pressure(seed: u64, day_index: u64, tod: f32) -> f32 {
    let base = unit_noise(seed, day_index, 0xAE17_0001);
    let front = ((std::f32::consts::TAU * (tod * 1.7 + base)).sin() + 1.0) * 0.5;
    let slow = unit_noise(seed, day_index / 2, 0xAE17_0002);
    (front * 0.55 + slow * 0.30 + base * 0.15).clamp(0.0, 1.0)
}

fn baseline_cloud_coverage(
    seed: u64,
    day_index: u64,
    tod: f32,
    profile: &EnvironmentProfileDescriptor,
) -> f32 {
    let seed_phase = unit_noise(seed, day_index, 0xC10D_7001);
    let daily_wave = ((std::f32::consts::TAU * (tod + seed_phase)).sin() + 1.0) * 0.5;
    let slow_front = unit_noise(seed, day_index / 3, 0xC10D_7002);
    let profile_bias = profile
        .cloud_profile_ref
        .bytes()
        .fold(0.0f32, |acc, byte| acc + byte as f32 * 0.00001)
        .fract()
        * 0.08;
    clamp01_f32(0.08 + daily_wave * 0.34 + slow_front * 0.40 + profile_bias)
}

fn cloud_layers(
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
