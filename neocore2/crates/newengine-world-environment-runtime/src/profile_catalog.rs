use crate::visual_asset_catalog::EnvironmentVisualAssetGroupDescriptor;
use newengine_world_environment_api::WeatherKind;

mod atmosphere;
mod clouds;
mod patterns;
mod profiles;
mod tables;

use atmosphere::ATMOSPHERE_PROFILES;
use clouds::CLOUD_PROFILES;
use patterns::PATTERNS;
use profiles::PROFILES;
use tables::TABLES;

// Provider-owned baseline descriptors. Runtime evaluation reads these tables; it must not
// infer weather by string substrings or hidden renderer state. The next production step is
// loading the same descriptor shape from `.yenv/.yweather/.ycloud/.ywind` ListFile entries.

#[derive(Clone, Copy, Debug)]
pub(crate) struct EnvironmentProfileDescriptor {
    pub id: &'static str,
    pub region: &'static str,
    pub biome: &'static str,
    pub weather_table_ref: &'static str,
    pub sky_profile_ref: &'static str,
    pub cloud_profile_ref: &'static str,
    pub atmosphere_profile_ref: &'static str,
    pub wind_profile_ref: &'static str,
    pub visual_assets: &'static EnvironmentVisualAssetGroupDescriptor,
    pub latitude_degrees: f32,
    pub axial_tilt_degrees: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AtmosphereProfileDescriptor {
    pub id: &'static str,
    pub mean_temperature_c: f32,
    pub terrain_elevation_m: f32,
    pub sea_level_pressure_hpa: f32,
    pub base_specific_humidity_g_per_kg: f32,
    pub background_aerosol: f32,
    pub lapse_rate_k_per_km: f32,
    pub surface_moisture_availability: f32,
    pub surface_albedo: f32,
    pub boundary_layer_heat_capacity_j_m2_k: f32,
    pub boundary_layer_depth_m: f32,
    pub geostrophic_wind_mps: f32,
    pub geostrophic_wind_x: f32,
    pub geostrophic_wind_z: f32,
    pub surface_roughness_m: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CloudProfileDescriptor {
    pub id: &'static str,
    /// Sub-grid morphology only. These coefficients cannot create condensate or set altitude.
    pub low_coverage_scale: f32,
    pub low_overcast_coverage_gain: f32,
    pub low_density_scale: f32,
    pub high_cloud_coverage_scale: f32,
    pub high_density_scale: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WeatherPresentationEntryDescriptor {
    /// Presentation mapping candidate for an already-observed physical WeatherKind.
    pub pattern_id: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WeatherPresentationTableDescriptor {
    pub bands: &'static [WeatherPresentationEntryDescriptor],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WeatherPresentationDescriptor {
    pub id: &'static str,
    pub kind: WeatherKind,
    pub weather_visual_ref: &'static str,
    pub required_assets: &'static [&'static str],
}

#[inline]
pub(crate) fn default_profile() -> &'static EnvironmentProfileDescriptor {
    &PROFILES[2]
}

#[inline]
pub(crate) fn profile_by_id(id: &str) -> (&'static EnvironmentProfileDescriptor, bool) {
    match id {
        "environment.game_ready_forest_road" => (&PROFILES[0], true),
        "environment.game_ready_highlands" => (&PROFILES[1], true),
        "environment.default" => (&PROFILES[2], true),
        "environment.alpine_winter" => (&PROFILES[3], true),
        "environment.desert_dusk" => (&PROFILES[4], true),
        _ => (default_profile(), false),
    }
}

#[inline]
pub(crate) fn cloud_profile_by_id(id: &str) -> &'static CloudProfileDescriptor {
    CLOUD_PROFILES
        .iter()
        .find(|profile| profile.id == id)
        .unwrap_or(&CLOUD_PROFILES[2])
}

#[inline]
pub(crate) fn atmosphere_profile_by_id(id: &str) -> &'static AtmosphereProfileDescriptor {
    ATMOSPHERE_PROFILES
        .iter()
        .find(|profile| profile.id == id)
        .unwrap_or(&ATMOSPHERE_PROFILES[2])
}

#[inline]
pub(crate) fn presentation_table_by_id(id: &str) -> &'static WeatherPresentationTableDescriptor {
    match id {
        "weather/game_ready_forest_road.yweather@table" => &TABLES[0],
        "weather/game_ready_highlands.yweather@table" => &TABLES[1],
        "weather/default_temperate.yweather@table" => &TABLES[2],
        "weather/alpine_winter.yweather@table" => &TABLES[3],
        "weather/desert_dust.yweather@table" => &TABLES[4],
        _ => &TABLES[1],
    }
}

#[inline]
pub(crate) fn presentation_by_id(id: &str) -> &'static WeatherPresentationDescriptor {
    match id {
        "weather.clear.dry_high_pressure" => &PATTERNS[0],
        "weather.cloudy.fair_cumulus" => &PATTERNS[1],
        "weather.overcast.stratus_deck" => &PATTERNS[2],
        "weather.rain.nimbostratus" => &PATTERNS[3],
        "weather.storm.cumulonimbus" => &PATTERNS[4],
        "weather.fog.ground_radiation" => &PATTERNS[5],
        "weather.snow.stratiform" => &PATTERNS[6],
        "weather.dust_storm.front" => &PATTERNS[7],
        _ => &PATTERNS[0],
    }
}
