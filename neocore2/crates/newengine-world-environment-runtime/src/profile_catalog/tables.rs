use super::{WeatherBandDescriptor, WeatherTableDescriptor};

pub(super) const DEFAULT_WEATHER_BANDS: &[WeatherBandDescriptor] = &[
    WeatherBandDescriptor {
        pattern_id: "weather.fog.ground_radiation",
        pressure_min: 0.20,
        pressure_max: 0.58,
        time_center: Some(0.23),
        time_half_width: 0.080,
        score_bias: 0.16,
    },
    WeatherBandDescriptor {
        pattern_id: "weather.clear.dry_high_pressure",
        pressure_min: 0.00,
        pressure_max: 0.36,
        time_center: None,
        time_half_width: 0.0,
        score_bias: 0.02,
    },
    WeatherBandDescriptor {
        pattern_id: "weather.cloudy.fair_cumulus",
        pressure_min: 0.30,
        pressure_max: 0.58,
        time_center: None,
        time_half_width: 0.0,
        score_bias: 0.03,
    },
    WeatherBandDescriptor {
        pattern_id: "weather.overcast.stratus_deck",
        pressure_min: 0.52,
        pressure_max: 0.76,
        time_center: None,
        time_half_width: 0.0,
        score_bias: 0.04,
    },
    WeatherBandDescriptor {
        pattern_id: "weather.rain.nimbostratus",
        pressure_min: 0.68,
        pressure_max: 0.90,
        time_center: None,
        time_half_width: 0.0,
        score_bias: 0.08,
    },
    WeatherBandDescriptor {
        pattern_id: "weather.storm.cumulonimbus",
        pressure_min: 0.86,
        pressure_max: 1.00,
        time_center: None,
        time_half_width: 0.0,
        score_bias: 0.10,
    },
];

pub(super) const HIGHLANDS_WEATHER_BANDS: &[WeatherBandDescriptor] = DEFAULT_WEATHER_BANDS;
pub(super) const FOREST_ROAD_WEATHER_BANDS: &[WeatherBandDescriptor] = &[
    WeatherBandDescriptor {
        pattern_id: "weather.fog.ground_radiation",
        pressure_min: 0.18,
        pressure_max: 0.46,
        time_center: Some(0.23),
        time_half_width: 0.055,
        score_bias: 0.06,
    },
    WeatherBandDescriptor {
        pattern_id: "weather.clear.dry_high_pressure",
        pressure_min: 0.00,
        pressure_max: 0.72,
        time_center: None,
        time_half_width: 0.0,
        score_bias: 0.19,
    },
    WeatherBandDescriptor {
        pattern_id: "weather.cloudy.fair_cumulus",
        pressure_min: 0.36,
        pressure_max: 0.90,
        time_center: None,
        time_half_width: 0.0,
        score_bias: 0.07,
    },
    WeatherBandDescriptor {
        pattern_id: "weather.overcast.stratus_deck",
        pressure_min: 0.82,
        pressure_max: 1.00,
        time_center: None,
        time_half_width: 0.0,
        score_bias: -0.08,
    },
];
pub(super) const SNOW_WEATHER_BANDS: &[WeatherBandDescriptor] = &[
    WeatherBandDescriptor {
        pattern_id: "weather.snow.stratiform",
        pressure_min: 0.48,
        pressure_max: 1.00,
        time_center: None,
        time_half_width: 0.0,
        score_bias: 0.16,
    },
    WeatherBandDescriptor {
        pattern_id: "weather.overcast.stratus_deck",
        pressure_min: 0.30,
        pressure_max: 0.62,
        time_center: None,
        time_half_width: 0.0,
        score_bias: 0.03,
    },
    WeatherBandDescriptor {
        pattern_id: "weather.clear.dry_high_pressure",
        pressure_min: 0.00,
        pressure_max: 0.35,
        time_center: None,
        time_half_width: 0.0,
        score_bias: 0.01,
    },
];
pub(super) const DESERT_WEATHER_BANDS: &[WeatherBandDescriptor] = &[
    WeatherBandDescriptor {
        pattern_id: "weather.dust_storm.front",
        pressure_min: 0.72,
        pressure_max: 1.00,
        time_center: None,
        time_half_width: 0.0,
        score_bias: 0.18,
    },
    WeatherBandDescriptor {
        pattern_id: "weather.clear.dry_high_pressure",
        pressure_min: 0.00,
        pressure_max: 0.78,
        time_center: None,
        time_half_width: 0.0,
        score_bias: 0.04,
    },
];

pub(super) const TABLES: &[WeatherTableDescriptor] = &[
    WeatherTableDescriptor {
        bands: FOREST_ROAD_WEATHER_BANDS,
    },
    WeatherTableDescriptor {
        bands: HIGHLANDS_WEATHER_BANDS,
    },
    WeatherTableDescriptor {
        bands: DEFAULT_WEATHER_BANDS,
    },
    WeatherTableDescriptor {
        bands: SNOW_WEATHER_BANDS,
    },
    WeatherTableDescriptor {
        bands: DESERT_WEATHER_BANDS,
    },
];
