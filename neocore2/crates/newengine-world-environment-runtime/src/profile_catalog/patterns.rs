use newengine_world_environment_api::WeatherKind;

use super::WeatherPresentationDescriptor;

pub(super) const PATTERNS: &[WeatherPresentationDescriptor] = &[
    WeatherPresentationDescriptor {
        id: "weather.clear.dry_high_pressure",
        kind: WeatherKind::Clear,
        weather_visual_ref: "weather/clear.yweather@dry_high_pressure",
        required_assets: &["sky/clear_day.ysky@gradient"],
    },
    WeatherPresentationDescriptor {
        id: "weather.cloudy.fair_cumulus",
        kind: WeatherKind::Cloudy,
        weather_visual_ref: "weather/cloudy.yweather@fair_cumulus",
        required_assets: &["clouds/cumulus_fields.ycloud@broken_cumulus"],
    },
    WeatherPresentationDescriptor {
        id: "weather.overcast.stratus_deck",
        kind: WeatherKind::Overcast,
        weather_visual_ref: "weather/overcast.yweather@stratus_deck",
        required_assets: &["clouds/stratus_fields.ycloud@overcast_sheet"],
    },
    WeatherPresentationDescriptor {
        id: "weather.rain.nimbostratus",
        kind: WeatherKind::Rain,
        weather_visual_ref: "weather/rain.yweather@nimbostratus",
        required_assets: &[
            "audio/weather.ybank@rain",
            "materials/wetness.nemat@default",
        ],
    },
    WeatherPresentationDescriptor {
        id: "weather.storm.cumulonimbus",
        kind: WeatherKind::Storm,
        weather_visual_ref: "weather/storm.yweather@cumulonimbus",
        required_assets: &[
            "clouds/cumulonimbus.ycloud@storm_core",
            "audio/weather.ybank@thunder",
            "audio/weather.ybank@rain",
            "materials/wetness.nemat@storm",
        ],
    },
    WeatherPresentationDescriptor {
        id: "weather.fog.ground_radiation",
        kind: WeatherKind::Fog,
        weather_visual_ref: "weather/fog.yweather@ground_radiation",
        required_assets: &["weather/fog.yweather@ground_radiation"],
    },
    WeatherPresentationDescriptor {
        id: "weather.snow.stratiform",
        kind: WeatherKind::Snow,
        weather_visual_ref: "weather/snow.yweather@stratiform",
        required_assets: &[
            "clouds/snow_bands.ycloud@stratiform",
            "audio/weather.ybank@snow",
        ],
    },
    WeatherPresentationDescriptor {
        id: "weather.dust_storm.front",
        kind: WeatherKind::DustStorm,
        weather_visual_ref: "weather/dust.yweather@front",
        required_assets: &[
            "weather/dust.yweather@front",
            "audio/weather.ybank@wind_heavy",
        ],
    },
];
