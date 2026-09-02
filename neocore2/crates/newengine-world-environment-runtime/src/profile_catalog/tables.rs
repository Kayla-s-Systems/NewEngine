use super::{WeatherPresentationEntryDescriptor, WeatherPresentationTableDescriptor};

pub(super) const DEFAULT_WEATHER_BANDS: &[WeatherPresentationEntryDescriptor] = &[
    WeatherPresentationEntryDescriptor {
        pattern_id: "weather.fog.ground_radiation",
    },
    WeatherPresentationEntryDescriptor {
        pattern_id: "weather.clear.dry_high_pressure",
    },
    WeatherPresentationEntryDescriptor {
        pattern_id: "weather.cloudy.fair_cumulus",
    },
    WeatherPresentationEntryDescriptor {
        pattern_id: "weather.overcast.stratus_deck",
    },
    WeatherPresentationEntryDescriptor {
        pattern_id: "weather.rain.nimbostratus",
    },
    WeatherPresentationEntryDescriptor {
        pattern_id: "weather.storm.cumulonimbus",
    },
];

pub(super) const HIGHLANDS_WEATHER_BANDS: &[WeatherPresentationEntryDescriptor] =
    DEFAULT_WEATHER_BANDS;
pub(super) const FOREST_ROAD_WEATHER_BANDS: &[WeatherPresentationEntryDescriptor] = &[
    WeatherPresentationEntryDescriptor {
        pattern_id: "weather.fog.ground_radiation",
    },
    WeatherPresentationEntryDescriptor {
        pattern_id: "weather.clear.dry_high_pressure",
    },
    WeatherPresentationEntryDescriptor {
        pattern_id: "weather.cloudy.fair_cumulus",
    },
    WeatherPresentationEntryDescriptor {
        pattern_id: "weather.overcast.stratus_deck",
    },
];
pub(super) const SNOW_WEATHER_BANDS: &[WeatherPresentationEntryDescriptor] = &[
    WeatherPresentationEntryDescriptor {
        pattern_id: "weather.snow.stratiform",
    },
    WeatherPresentationEntryDescriptor {
        pattern_id: "weather.overcast.stratus_deck",
    },
    WeatherPresentationEntryDescriptor {
        pattern_id: "weather.clear.dry_high_pressure",
    },
];
pub(super) const DESERT_WEATHER_BANDS: &[WeatherPresentationEntryDescriptor] = &[
    WeatherPresentationEntryDescriptor {
        pattern_id: "weather.dust_storm.front",
    },
    WeatherPresentationEntryDescriptor {
        pattern_id: "weather.clear.dry_high_pressure",
    },
];

pub(super) const TABLES: &[WeatherPresentationTableDescriptor] = &[
    WeatherPresentationTableDescriptor {
        bands: FOREST_ROAD_WEATHER_BANDS,
        fallback_pattern_id: "weather.clear.dry_high_pressure",
    },
    WeatherPresentationTableDescriptor {
        bands: HIGHLANDS_WEATHER_BANDS,
        fallback_pattern_id: "weather.clear.dry_high_pressure",
    },
    WeatherPresentationTableDescriptor {
        bands: DEFAULT_WEATHER_BANDS,
        fallback_pattern_id: "weather.clear.dry_high_pressure",
    },
    WeatherPresentationTableDescriptor {
        bands: SNOW_WEATHER_BANDS,
        fallback_pattern_id: "weather.snow.stratiform",
    },
    WeatherPresentationTableDescriptor {
        bands: DESERT_WEATHER_BANDS,
        fallback_pattern_id: "weather.clear.dry_high_pressure",
    },
];
