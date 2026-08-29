use super::mesoscale::MesoscaleDiagnostics;
use crate::{
    consumer_packets::build_consumer_packets,
    default_provider::inputs::deterministic_key_for_day,
    profile_catalog::{EnvironmentProfileDescriptor, WeatherPresentationDescriptor},
};
use newengine_world_environment_api::{
    AtmosphereStateDto, CelestialBodyDto, CelestialStateDto, CloudStateDto,
    EnvironmentAtmosphereCellDto, EnvironmentDiagnosticsDto, EnvironmentFrameDto,
    EnvironmentFrameRequest, EnvironmentGameplayModifiersDto, EnvironmentGlobalStateDto,
    EnvironmentLightingIntentDto, EnvironmentObjectDto, EnvironmentVisualAssetRefsDto,
    ExposureIntentDto, SkyStateDto, TimeOfDayStateDto, WeatherStateDto, WindStateDto,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn assemble(
    provider: &str,
    provider_route: &str,
    req: EnvironmentFrameRequest,
    profile: &EnvironmentProfileDescriptor,
    profile_found: bool,
    profile_warnings: Vec<String>,
    pattern: &WeatherPresentationDescriptor,
    world_time_seconds: f64,
    normalized_day: f32,
    day_index: u32,
    day_index_u64: u64,
    time_of_day: TimeOfDayStateDto,
    sun: CelestialBodyDto,
    moon: CelestialBodyDto,
    sky: SkyStateDto,
    atmosphere: AtmosphereStateDto,
    weather: WeatherStateDto,
    clouds: CloudStateDto,
    wind: WindStateDto,
    lighting: EnvironmentLightingIntentDto,
    gameplay: EnvironmentGameplayModifiersDto,
    exposure: ExposureIntentDto,
    environment_objects: Vec<EnvironmentObjectDto>,
    spatial_atmosphere: Vec<EnvironmentAtmosphereCellDto>,
    mesoscale: MesoscaleDiagnostics,
    net_radiative_flux_w_m2: f32,
    evaporation_flux_kg_m2_s: f32,
) -> EnvironmentFrameDto {
    let visual_assets = profile.visual_assets;
    let affected_cells = if !req.resident_cells.is_empty() {
        req.resident_cells.clone()
    } else if !spatial_atmosphere.is_empty() {
        spatial_atmosphere.iter().map(|cell| cell.cell).collect()
    } else {
        req.observer_cell
            .map(|cell| vec![cell])
            .unwrap_or_else(|| vec![newengine_world_api::WorldCellCoord::new(0, 0)])
    };
    let consumer_packets = build_consumer_packets(
        profile,
        pattern,
        &time_of_day,
        &sun,
        &moon,
        &atmosphere,
        &weather,
        &clouds,
        &wind,
        &lighting,
        &gameplay,
        &exposure,
        affected_cells,
    );
    let key = deterministic_key_for_day(&req, provider, normalized_day);
    let environment_object_count = environment_objects.len();
    let cloud_coverage = clouds.coverage;

    EnvironmentFrameDto {
        frame_id: req.frame_id,
        world_instance_id: req.world_instance_id,
        world_time_seconds,
        time_of_day_normalized: normalized_day,
        day_index,
        time_of_day_state: time_of_day,
        global: EnvironmentGlobalStateDto {
            active_region: req.active_region.or_else(|| Some(profile.region.to_owned())),
            active_biome: req.active_biome.or_else(|| Some(profile.biome.to_owned())),
            active_weather_profile: weather.weather_id.clone(),
            active_environment_profile: profile.id.to_owned(),
            weather_table_ref: profile.weather_table_ref.to_owned(),
            sky_profile_ref: profile.sky_profile_ref.to_owned(),
            cloud_profile_ref: profile.cloud_profile_ref.to_owned(),
            atmosphere_profile_ref: profile.atmosphere_profile_ref.to_owned(),
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
            moon_phase: crate::celestial::moon_phase(req.seed, day_index_u64),
            stars_visibility: time_of_day.night_blend * (1.0 - cloud_coverage * 0.75),
            night_sky_visibility: time_of_day.night_blend * (1.0 - cloud_coverage * 0.65),
        },
        sky,
        atmosphere: atmosphere.clone(),
        weather: weather.clone(),
        clouds,
        wind,
        lighting_intent: lighting,
        gameplay_modifiers: gameplay,
        exposure_intent: exposure,
        environment_objects,
        spatial_cell_size_meters: req.spatial_cell_size_meters.max(0.0),
        spatial_atmosphere,
        consumer_packets,
        diagnostics: EnvironmentDiagnosticsDto {
            provider: provider.to_owned(),
            provider_route: provider_route.to_owned(),
            degraded: false,
            deterministic_key: key,
            active_profile: profile.id.to_owned(),
            reasons: vec![
                format!(
                    "profile={} profile_found={} atmosphere_profile={}",
                    profile.id, profile_found, profile.atmosphere_profile_ref
                ),
                format!(
                    "atmosphere_graph={}",
                    super::physics::graph::diagnostic_path()
                ),
                format!(
                    "radiation net={:.2}W/m2 evaporation={:.8}kg/m2/s",
                    net_radiative_flux_w_m2, evaporation_flux_kg_m2_s
                ),
                format!(
                    "thermodynamics p={:.1}hPa T={:.2}C Td={:.2}C RH={:.3} q={:.2}g/kg rho={:.3}kg/m3 LCL={:.0}m PW={:.2}mm CWP={:.3}kg/m2 CAPE={:.0}J/kg CIN={:.0}J/kg cloud_top={:.0}m",
                    atmosphere.surface_pressure_hpa,
                    atmosphere.temperature_celsius,
                    atmosphere.dew_point_celsius,
                    atmosphere.humidity,
                    atmosphere.specific_humidity_g_per_kg,
                    atmosphere.air_density_kg_m3,
                    atmosphere.lifting_condensation_level_meters,
                    atmosphere.precipitable_water_mm,
                    atmosphere.cloud_water_path_kg_m2,
                    atmosphere.cape_j_per_kg,
                    atmosphere.cin_j_per_kg,
                    atmosphere.convective_cloud_top_meters,
                ),
                format!(
                    "observed_weather={} precip={:.3}mm/h clouds={:.3} visibility={:.0}m aerosol={:.3}",
                    weather.weather_id,
                    weather.precipitation.rate_mm_per_hour,
                    cloud_coverage,
                    atmosphere.visibility_distance_meters,
                    atmosphere.aerosol_density,
                ),
                format!("mesoscale_graph={}", mesoscale.graph_path),
                format!(
                    "mesoscale enabled={} cells={} dt={:.3}s momentum_substeps={} transport_substeps={} CFL={:.3} mass_error={:.6}kg/m2 vapor_error={:.6}kg/m2 CWP_error={:.8}kg/m2 max_dp_accel={:.6}m/s2 max_wind={:.2}m/s duplicates={}",
                    mesoscale.enabled,
                    mesoscale.cell_count,
                    mesoscale.dt_seconds,
                    mesoscale.momentum_substeps,
                    mesoscale.transport_substeps,
                    mesoscale.transport_cfl,
                    mesoscale.column_mass_error_kg_m2_sum,
                    mesoscale.vapor_mass_error_kg_m2_sum,
                    mesoscale.cwp_error_kg_m2_sum,
                    mesoscale.max_pressure_accel_m_s2,
                    mesoscale.max_large_scale_wind_mps,
                    mesoscale.duplicate_boundaries,
                ),
                format!("environment_objects={environment_object_count}"),
                "weather state is diagnosed from physics; yweather maps observations to assets only".to_owned(),
                "weather physics contains no seed/noise/random input".to_owned(),
                "engine.render consumes resolved environment state and cannot author weather".to_owned(),
            ],
            warnings: profile_warnings,
        },
    }
}
