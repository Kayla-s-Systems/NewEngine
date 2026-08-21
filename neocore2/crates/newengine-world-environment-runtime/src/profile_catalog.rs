use crate::visual_asset_catalog::EnvironmentVisualAssetGroupDescriptor;
use newengine_world_environment_api::{EnvironmentObjectKind, PrecipitationKind, WeatherKind};

mod patterns;
mod phenomena;
mod profiles;
mod tables;

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
    pub wind_profile_ref: &'static str,
    pub visual_assets: &'static EnvironmentVisualAssetGroupDescriptor,
    pub latitude_degrees: f32,
    pub axial_tilt_degrees: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WeatherBandDescriptor {
    pub pattern_id: &'static str,
    pub pressure_min: f32,
    pub pressure_max: f32,
    pub time_center: Option<f32>,
    pub time_half_width: f32,
    pub score_bias: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WeatherTableDescriptor {
    pub bands: &'static [WeatherBandDescriptor],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WeatherPatternDescriptor {
    pub id: &'static str,
    pub kind: WeatherKind,
    pub weather_visual_ref: &'static str,
    pub intensity_min: f32,
    pub intensity_max: f32,
    /// Minimum fractional sky coverage for this meteorological regime.
    pub cloud_floor: f32,
    /// Maximum fractional sky coverage for this regime. Coverage is resolved
    /// continuously inside `[cloud_floor, cloud_ceiling]`; the floor is not a
    /// global clamp on an unrelated baseline signal.
    pub cloud_ceiling: f32,
    pub overcast_bias: f32,
    pub precipitation_kind: PrecipitationKind,
    pub precipitation_factor: f32,
    pub thunder_factor: f32,
    pub wetness_factor: f32,
    pub snow_factor: f32,
    pub fog_factor: f32,
    pub haze_factor: f32,
    pub wind_base_mps: f32,
    pub wind_gain_mps: f32,
    pub gust_base: f32,
    pub gust_gain: f32,
    pub visibility_factor: f32,
    pub tags: &'static [&'static str],
    pub required_assets: &'static [&'static str],
    pub phenomena: &'static [PhenomenonTemplateDescriptor],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PhenomenonTemplateDescriptor {
    pub kind: EnvironmentObjectKind,
    pub template_id: &'static str,
    pub activation_threshold: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub offset_z: f32,
    pub radius: f32,
    pub y_min: f32,
    pub y_max: f32,
    pub altitude_min: f32,
    pub altitude_max: f32,
    pub priority: &'static str,
    pub reason: &'static str,
    pub tags: &'static [&'static str],
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
pub(crate) fn table_by_id(id: &str) -> &'static WeatherTableDescriptor {
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
pub(crate) fn pattern_by_id(id: &str) -> &'static WeatherPatternDescriptor {
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
