mod clouds;
mod components;
mod inputs;

use crate::celestial::{moon_body, moon_phase, sun_body, time_of_day_state};
use crate::consumer_packets::build_consumer_packets;
use crate::math::{clamp01_f32, unit_noise};
use crate::phenomena::{build_environment_objects, environment_object_cells};
use crate::profile_catalog::profile_by_id;
use crate::weather_profile::{enrich_weather_tags, evaluate_weather, WeatherEvaluation};
use clouds::cloud_layers;
use components::{
    build_atmosphere_state, build_exposure_intent, build_gameplay_modifiers, build_lighting_intent,
    build_sky_state, build_wind_state, AtmosphereInputs,
};
pub(crate) use inputs::deterministic_key;
use inputs::{
    baseline_cloud_coverage, deterministic_key_for_day, normalized_day_from_time,
    normalized_profile_id, profile_warning, weather_pressure,
};
use newengine_world_environment_api::{
    CelestialStateDto, CloudStateDto, EnvironmentDiagnosticsDto, EnvironmentFrameDto,
    EnvironmentFrameRequest, EnvironmentGlobalStateDto, EnvironmentObjectKind,
    EnvironmentVisualAssetRefsDto,
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
    let (profile, profile_found) = profile_by_id(requested_profile_id);
    let profile_warnings = profile_warning(profile_found, requested_profile_id);
    let sun = sun_body(
        tod,
        profile.latitude_degrees,
        profile.axial_tilt_degrees,
        day_index_u64,
    );
    let time_of_day_state = time_of_day_state(tod, sun.direction_world.y);
    let moon = moon_body(tod, req.seed, day_index_u64);
    let cloud_seed = unit_noise(req.seed, day_index_u64, 0xC10D_0001);
    let pressure = weather_pressure(req.seed, day_index_u64, tod);
    let WeatherEvaluation {
        pattern,
        mut weather,
        cloud_floor,
        overcast_bias,
        fog_bias,
        haze_bias,
        wind_base_mps,
        wind_gain_mps,
        gust_base,
        gust_gain,
        visibility_factor,
    } = evaluate_weather(profile, tod, pressure, cloud_seed);
    let visual_assets = profile.visual_assets;

    let baseline_coverage = baseline_cloud_coverage(req.seed, day_index_u64, tod, profile);
    let cloud_coverage = baseline_coverage.max(cloud_floor).clamp(0.0, 1.0);
    let overcast =
        clamp01_f32((cloud_coverage - 0.55) * 1.9 + weather.intensity * 0.20 + overcast_bias);
    let precipitation = weather.precipitation.intensity;
    let fog_weather = fog_bias * weather.intensity;
    let haze = 0.04
        + 0.10 * time_of_day_state.dawn_dusk_blend
        + 0.08 * cloud_coverage
        + 0.22 * precipitation
        + haze_bias
        + 0.16 * fog_weather;
    let visibility =
        (20_000.0 * visibility_factor * (1.0 - overcast * 0.34) * (1.0 - haze * 0.45)).max(120.0);

    enrich_weather_tags(
        &mut weather,
        time_of_day_state.phase,
        visibility,
        cloud_coverage,
    );

    let sky = build_sky_state(&time_of_day_state, overcast);

    let atmosphere = build_atmosphere_state(AtmosphereInputs {
        time_of_day: &time_of_day_state,
        overcast,
        precipitation,
        fog_bias,
        weather_intensity: weather.intensity,
        cloud_coverage,
        haze,
        visibility,
    });

    let wind = build_wind_state(
        wind_base_mps,
        wind_gain_mps,
        gust_base,
        gust_gain,
        cloud_coverage,
        weather.intensity,
        overcast,
    );

    let activation = (cloud_coverage * 0.55 + weather.intensity * 0.45).clamp(0.0, 1.0);
    let environment_objects = build_environment_objects(
        &req,
        profile,
        pattern,
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

    let lighting_intent = build_lighting_intent(
        &sun,
        &moon,
        &clouds,
        &time_of_day_state,
        cloud_coverage,
        &weather,
        overcast,
    );

    let gameplay_modifiers = build_gameplay_modifiers(&weather, &wind, visibility, fog_bias);

    let exposure_intent =
        build_exposure_intent(&time_of_day_state, &sun, &weather, overcast, cloud_coverage);

    let affected_cells = environment_object_cells(&req);
    let consumer_packets = build_consumer_packets(
        profile,
        pattern,
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

    let key = deterministic_key_for_day(&req, provider, tod);
    let environment_object_count = environment_objects.len();
    let active_weather_profile = weather.weather_id.clone();
    let weather_intensity = weather.intensity;
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
            active_weather_profile,
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
            weather_visual_ref: pattern.weather_visual_ref.to_owned(),
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
                format!("weather_pattern={} intensity={:.3} coverage={:.3}", pattern.id, weather_intensity, cloud_coverage),
                format!("visual_assets group='{}' dictionary='{}' sky='{}' sun='{}' moon='{}' cloud_density='{}'", visual_assets.id, visual_assets.texture_dictionary_ref, visual_assets.sky_texture_ref, visual_assets.sun_disk_texture_ref, visual_assets.moon_disk_texture_ref, visual_assets.cloud_density_texture_ref),
                format!("environment_objects={}", environment_object_count),
                "engine.time provides clock authority".to_owned(),
                "weather is selected from profile table, not string substring branches".to_owned(),
                "engine.world.environment resolves environmental meaning".to_owned(),
                "engine.render remains a consumer of resolved packets".to_owned(),
            ],
            warnings: profile_warnings,
        },
    }
}
