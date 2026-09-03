mod clouds;
mod components;
mod frame_assembly;
mod inputs;
mod mesoscale;
mod observation;
mod physics;
mod surface;

use crate::celestial::{moon_body, sun_body, time_of_day_state};
use crate::math::clamp01_f32;
use crate::profile_catalog::{atmosphere_profile_by_id, profile_by_id};
use clouds::cloud_layers;
use components::{
    build_exposure_intent, build_gameplay_modifiers, build_lighting_intent, build_sky_state,
};
pub(crate) use inputs::deterministic_key;
use inputs::{normalized_day_from_time, normalized_profile_id, profile_warning};
use newengine_world_environment_api::{
    CloudStateDto, EnvironmentFrameDto, EnvironmentFrameRequest, EnvironmentObjectKind,
    EnvironmentWeatherConstraint, PrecipitationStateDto, ThunderStateDto, WeatherKind,
};
use observation::{enrich_tags, observe};
use physics::{step as step_atmosphere_graph, AtmosphereGraphInput, ColumnMemory};

pub(crate) fn build_default_environment_frame(
    provider: &str,
    provider_route: &str,
    req: EnvironmentFrameRequest,
) -> EnvironmentFrameDto {
    build_default_environment_frame_with_history(provider, provider_route, req, None)
}

pub(crate) fn build_default_environment_frame_with_history(
    provider: &str,
    provider_route: &str,
    req: EnvironmentFrameRequest,
    previous: Option<&EnvironmentFrameDto>,
) -> EnvironmentFrameDto {
    let normalized_day = normalized_day_from_time(&req);
    let day_index_u64 = req.time.game.day_index;
    let day_index = day_index_u64.min(u32::MAX as u64) as u32;
    let world_time_seconds = req.time.game.day_index as f64
        * req.time.game.seconds_per_game_day.max(1.0)
        + req.time.game.seconds_of_day.max(0.0);

    let requested_profile_id = normalized_profile_id(&req);
    let (profile, profile_found) = profile_by_id(requested_profile_id);
    let profile_warnings = profile_warning(profile_found, requested_profile_id);
    let atmosphere_profile = atmosphere_profile_by_id(profile.atmosphere_profile_ref);

    let sun = sun_body(
        normalized_day,
        profile.latitude_degrees,
        profile.axial_tilt_degrees,
        day_index_u64,
    );
    let time_of_day = time_of_day_state(normalized_day, sun.direction_world.y);
    let moon = moon_body(normalized_day, req.seed, day_index_u64);

    // History continuity depends only on world/profile identity. `seed` is not a
    // meteorological input and therefore cannot reset or alter the physical atmosphere.
    let history = previous.filter(|frame| {
        frame.world_instance_id == req.world_instance_id
            && frame.global.active_environment_profile == profile.id
    });

    let graph = step_atmosphere_graph(AtmosphereGraphInput {
        profile: atmosphere_profile,
        surface: None,
        large_scale_wind: None,
        sun_elevation_sine: sun.direction_world.y,
        day_blend: time_of_day.day_blend,
        world_time_seconds,
        previous: history.map(ColumnMemory::from_frame),
    });
    let fallback_atmosphere = graph.atmosphere;
    let fallback_cloud_coverage = graph.cloud_coverage;
    let fallback_overcast = graph.overcast;
    let fallback_wind = graph.wind;

    let observed = observe(
        profile,
        &fallback_atmosphere,
        fallback_cloud_coverage,
        fallback_overcast,
        graph.precipitation_kind,
        graph.precipitation_rate_mm_h,
        graph.precipitation_intensity,
        graph.thunder_probability,
    );
    let fallback_pattern = observed.pattern;
    let mut fallback_weather = observed.weather;
    surface::integrate(
        &mut fallback_weather,
        &fallback_atmosphere,
        &fallback_wind,
        sun.direction_world.y,
        graph.evaporation_flux_kg_m2_s,
        history.map(surface::SurfaceMemory::from_frame),
        world_time_seconds,
    );
    enrich_tags(
        &mut fallback_weather,
        time_of_day.phase,
        fallback_atmosphere.visibility_distance_meters,
        fallback_cloud_coverage,
    );
    let fallback_clouds = CloudStateDto {
        coverage: fallback_cloud_coverage,
        overcast: fallback_overcast,
        shadow_strength: clamp01_f32(
            fallback_cloud_coverage * 0.36
                + fallback_overcast * 0.18
                + fallback_atmosphere.cloud_water_path_kg_m2 * 0.16,
        ),
        light_absorption: clamp01_f32(
            fallback_cloud_coverage * 0.20
                + fallback_overcast * 0.18
                + fallback_atmosphere.cloud_water_path_kg_m2 * 0.20
                + fallback_weather.precipitation.intensity * 0.10,
        ),
        layers: cloud_layers(
            profile,
            fallback_cloud_coverage,
            fallback_overcast,
            fallback_atmosphere.lifting_condensation_level_meters,
            fallback_atmosphere.cloud_water_path_kg_m2,
            fallback_atmosphere.convective_cloud_top_meters,
            graph.upper_ice_cloud_signal,
            &fallback_atmosphere.vertical_layers,
            &fallback_wind,
        ),
        volumes: Vec::new(),
        storm_cells: Vec::new(),
    };

    let mesoscale = mesoscale::step(
        &req,
        profile,
        atmosphere_profile,
        history,
        world_time_seconds,
        sun.direction_world.y,
        time_of_day.day_blend,
        time_of_day.phase,
    );
    let observer_spatial_cell = req
        .observer_cell
        .and_then(|observer| mesoscale.cells.iter().find(|cell| cell.cell == observer));
    let (atmosphere, mut weather, mut clouds, wind, mut pattern) =
        if let Some(cell) = observer_spatial_cell {
            (
                cell.atmosphere,
                cell.weather.clone(),
                cell.clouds.clone(),
                cell.wind,
                observation::pattern_for_observation(profile, cell.weather.state),
            )
        } else {
            (
                fallback_atmosphere,
                fallback_weather,
                fallback_clouds,
                fallback_wind,
                fallback_pattern,
            )
        };
    // Apply world-authored meteorological constraints here, before *any* lighting,
    // exposure or consumer packet is derived. This prevents an impossible state such as
    // `cloudless visuals + cumulonimbus attenuation`, and keeps the policy out of maps,
    // shaders and renderer-specific code.
    if matches!(
        req.weather_constraint,
        EnvironmentWeatherConstraint::ClearSky
    ) {
        clouds.coverage = 0.0;
        clouds.overcast = 0.0;
        clouds.shadow_strength = 0.0;
        clouds.light_absorption = 0.0;
        clouds.layers.clear();
        clouds.volumes.clear();
        clouds.storm_cells.clear();

        weather.weather_id = "weather.clear.dry_high_pressure".to_owned();
        weather.state = WeatherKind::Clear;
        weather.intensity = 0.0;
        weather.transition_progress = 1.0;
        weather.precipitation = PrecipitationStateDto::default();
        weather.thunder = ThunderStateDto::default();
        pattern = observation::pattern_for_observation(profile, WeatherKind::Clear);
        enrich_tags(
            &mut weather,
            time_of_day.phase,
            atmosphere.visibility_distance_meters,
            0.0,
        );
    }

    let cloud_coverage = clouds.coverage;
    let overcast = clouds.overcast;
    let sky = build_sky_state(&time_of_day, overcast);
    // Spatial objects exist only when extracted from resolved mesoscale topology.
    // Presentation descriptors never instantiate them.
    let environment_objects = mesoscale.objects.clone();
    clouds.volumes = environment_objects
        .iter()
        .filter(|object| {
            matches!(
                object.kind,
                EnvironmentObjectKind::CloudField
                    | EnvironmentObjectKind::CloudVolume
                    | EnvironmentObjectKind::FogBank
                    | EnvironmentObjectKind::SnowBand
                    | EnvironmentObjectKind::DustWall
                    | EnvironmentObjectKind::HeatHazeZone
            )
        })
        .cloned()
        .collect();
    clouds.storm_cells = environment_objects
        .iter()
        .filter(|object| {
            matches!(
                object.kind,
                EnvironmentObjectKind::StormCell | EnvironmentObjectKind::WeatherFront
            )
        })
        .cloned()
        .collect();

    let lighting = build_lighting_intent(
        &sun,
        &moon,
        &clouds,
        &time_of_day,
        cloud_coverage,
        &weather,
        overcast,
    );
    let gameplay = build_gameplay_modifiers(
        &weather,
        &wind,
        atmosphere.visibility_distance_meters,
        atmosphere.fog_density,
    );
    let exposure = build_exposure_intent(&time_of_day, &sun, &weather, overcast, cloud_coverage);

    frame_assembly::assemble(
        provider,
        provider_route,
        req,
        profile,
        profile_found,
        profile_warnings,
        pattern,
        world_time_seconds,
        normalized_day,
        day_index,
        day_index_u64,
        time_of_day,
        sun,
        moon,
        sky,
        atmosphere,
        weather,
        clouds,
        wind,
        lighting,
        gameplay,
        exposure,
        environment_objects,
        mesoscale.cells,
        mesoscale.diagnostics,
        graph.net_radiative_flux_w_m2,
        graph.evaporation_flux_kg_m2_s,
    )
}
