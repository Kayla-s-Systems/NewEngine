use crate::visual_asset_catalog::{
    EnvironmentVisualAssetGroupDescriptor, GAME_READY_SKYDOME_VISUALS,
};
use newengine_world_environment_api::{EnvironmentObjectKind, PrecipitationKind, WeatherKind};

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
    pub id: &'static str,
    pub bands: &'static [WeatherBandDescriptor],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WeatherPatternDescriptor {
    pub id: &'static str,
    pub kind: WeatherKind,
    pub weather_visual_ref: &'static str,
    pub intensity_min: f32,
    pub intensity_max: f32,
    pub cloud_floor: f32,
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

const FAIR_CLOUD_FIELD: PhenomenonTemplateDescriptor = PhenomenonTemplateDescriptor {
    kind: EnvironmentObjectKind::CloudField,
    template_id: "cloud_field.fair_weather",
    activation_threshold: 0.10,
    offset_x: 0.0,
    offset_y: 2500.0,
    offset_z: 0.0,
    radius: 5200.0,
    y_min: 900.0,
    y_max: 5600.0,
    altitude_min: 1200.0,
    altitude_max: 4200.0,
    priority: "normal",
    reason: "environment_cloud",
    tags: &[
        "environment.cloud_field",
        "cloud.cumulus",
        "streaming.environment.cloud",
    ],
    required_assets: &["clouds/cumulus_fields.ycloud@fair_weather"],
};

const LOCAL_CUMULUS_VOLUME: PhenomenonTemplateDescriptor = PhenomenonTemplateDescriptor {
    kind: EnvironmentObjectKind::CloudVolume,
    template_id: "cloud_volume.local_cumulus_bank",
    activation_threshold: 0.42,
    offset_x: 900.0,
    offset_y: 2400.0,
    offset_z: -850.0,
    radius: 2200.0,
    y_min: 900.0,
    y_max: 4200.0,
    altitude_min: 1200.0,
    altitude_max: 3600.0,
    priority: "normal",
    reason: "environment_cloud",
    tags: &[
        "environment.cloud_volume",
        "cloud.cumulus",
        "streaming.environment.cloud",
    ],
    required_assets: &["clouds/cumulus_fields.ycloud@local_volume"],
};

const PHENOMENA_CLEAR: &[PhenomenonTemplateDescriptor] = &[FAIR_CLOUD_FIELD];

const PHENOMENA_CLOUDY: &[PhenomenonTemplateDescriptor] = &[
    PhenomenonTemplateDescriptor {
        activation_threshold: 0.18,
        template_id: "cloud_field.broken_cumulus",
        required_assets: &["clouds/cumulus_fields.ycloud@broken_cumulus"],
        ..FAIR_CLOUD_FIELD
    },
    LOCAL_CUMULUS_VOLUME,
];

const PHENOMENA_OVERCAST: &[PhenomenonTemplateDescriptor] = &[
    PhenomenonTemplateDescriptor {
        activation_threshold: 0.25,
        template_id: "cloud_field.overcast_sheet",
        required_assets: &["clouds/stratus_fields.ycloud@overcast_sheet"],
        tags: &[
            "environment.cloud_field",
            "cloud.overcast",
            "streaming.environment.cloud",
        ],
        ..FAIR_CLOUD_FIELD
    },
    PhenomenonTemplateDescriptor {
        activation_threshold: 0.62,
        template_id: "cloud_volume.low_stratus_bank",
        required_assets: &["clouds/stratus_fields.ycloud@low_bank"],
        tags: &[
            "environment.cloud_volume",
            "cloud.stratus",
            "streaming.environment.cloud",
        ],
        ..LOCAL_CUMULUS_VOLUME
    },
];

const PHENOMENA_RAIN: &[PhenomenonTemplateDescriptor] = &[
    PhenomenonTemplateDescriptor {
        activation_threshold: 0.30,
        template_id: "cloud_field.nimbostratus_rain",
        required_assets: &[
            "clouds/nimbostratus.ycloud@rain_sheet",
            "audio/weather.ybank@rain",
        ],
        tags: &[
            "environment.cloud_field",
            "cloud.nimbostratus",
            "weather.rain",
            "streaming.environment.weather",
        ],
        ..FAIR_CLOUD_FIELD
    },
    PhenomenonTemplateDescriptor {
        activation_threshold: 0.45,
        template_id: "storm_cell.rain_band",
        required_assets: &[
            "clouds/rain_bands.ycloud@moving_band",
            "audio/weather.ybank@rain",
        ],
        kind: EnvironmentObjectKind::WeatherFront,
        priority: "high",
        reason: "weather_front",
        tags: &[
            "environment.weather_front",
            "weather.rain",
            "streaming.environment.weather",
        ],
        ..LOCAL_CUMULUS_VOLUME
    },
];

const PHENOMENA_STORM: &[PhenomenonTemplateDescriptor] = &[
    PhenomenonTemplateDescriptor {
        activation_threshold: 0.22,
        template_id: "cloud_field.cumulonimbus_anvil",
        required_assets: &[
            "clouds/cumulonimbus.ycloud@anvil_field",
            "audio/weather.ybank@thunder",
            "audio/weather.ybank@rain",
        ],
        tags: &[
            "environment.cloud_field",
            "cloud.cumulonimbus",
            "weather.storm",
            "streaming.environment.weather",
        ],
        ..FAIR_CLOUD_FIELD
    },
    PhenomenonTemplateDescriptor {
        activation_threshold: 0.36,
        template_id: "storm_cell.cumulonimbus_core",
        required_assets: &[
            "clouds/cumulonimbus.ycloud@storm_core",
            "audio/weather.ybank@thunder",
            "materials/wetness.nemat@storm",
        ],
        kind: EnvironmentObjectKind::StormCell,
        offset_x: -700.0,
        offset_y: 2500.0,
        offset_z: 1200.0,
        radius: 3600.0,
        priority: "critical",
        reason: "storm_cell",
        tags: &[
            "environment.storm_cell",
            "weather.storm",
            "cloud.cumulonimbus",
            "streaming.environment.weather",
        ],
        ..LOCAL_CUMULUS_VOLUME
    },
];

const PHENOMENA_FOG: &[PhenomenonTemplateDescriptor] = &[PhenomenonTemplateDescriptor {
    activation_threshold: 0.20,
    template_id: "fog_bank.ground_radiation",
    required_assets: &["weather/fog.yweather@ground_radiation"],
    kind: EnvironmentObjectKind::FogBank,
    offset_x: 0.0,
    offset_y: 0.0,
    offset_z: 0.0,
    radius: 2200.0,
    y_min: 0.0,
    y_max: 360.0,
    altitude_min: 0.0,
    altitude_max: 320.0,
    priority: "high",
    reason: "environment_fog",
    tags: &["environment.fog_bank", "weather.fog", "visibility.low"],
}];

const PHENOMENA_SNOW: &[PhenomenonTemplateDescriptor] = &[PhenomenonTemplateDescriptor {
    activation_threshold: 0.28,
    template_id: "snow_band.stratiform",
    required_assets: &[
        "clouds/snow_bands.ycloud@stratiform",
        "audio/weather.ybank@snow",
    ],
    kind: EnvironmentObjectKind::SnowBand,
    priority: "high",
    reason: "snow_band",
    tags: &[
        "environment.snow_band",
        "weather.snow",
        "streaming.environment.weather",
    ],
    ..LOCAL_CUMULUS_VOLUME
}];

const PHENOMENA_DUST: &[PhenomenonTemplateDescriptor] = &[PhenomenonTemplateDescriptor {
    activation_threshold: 0.34,
    template_id: "dust_wall.front",
    required_assets: &[
        "weather/dust.yweather@front",
        "audio/weather.ybank@wind_heavy",
    ],
    kind: EnvironmentObjectKind::DustWall,
    priority: "critical",
    reason: "dust_wall",
    tags: &[
        "environment.dust_wall",
        "weather.dust_storm",
        "visibility.low",
    ],
    ..LOCAL_CUMULUS_VOLUME
}];

const PATTERNS: &[WeatherPatternDescriptor] = &[
    WeatherPatternDescriptor {
        id: "weather.clear.dry_high_pressure",
        kind: WeatherKind::Clear,
        weather_visual_ref: "weather/clear.yweather@dry_high_pressure",
        intensity_min: 0.00,
        intensity_max: 0.18,
        cloud_floor: 0.08,
        overcast_bias: 0.00,
        precipitation_kind: PrecipitationKind::None,
        precipitation_factor: 0.0,
        thunder_factor: 0.0,
        wetness_factor: 0.0,
        snow_factor: 0.0,
        fog_factor: 0.0,
        haze_factor: 0.04,
        wind_base_mps: 1.2,
        wind_gain_mps: 1.8,
        gust_base: 0.04,
        gust_gain: 0.10,
        visibility_factor: 1.0,
        tags: &["weather.clear", "visibility.normal"],
        required_assets: &["sky/clear_day.ysky@gradient"],
        phenomena: PHENOMENA_CLEAR,
    },
    WeatherPatternDescriptor {
        id: "weather.cloudy.fair_cumulus",
        kind: WeatherKind::Cloudy,
        weather_visual_ref: "weather/cloudy.yweather@fair_cumulus",
        intensity_min: 0.30,
        intensity_max: 0.62,
        cloud_floor: 0.42,
        overcast_bias: 0.10,
        precipitation_kind: PrecipitationKind::None,
        precipitation_factor: 0.0,
        thunder_factor: 0.0,
        wetness_factor: 0.0,
        snow_factor: 0.0,
        fog_factor: 0.0,
        haze_factor: 0.09,
        wind_base_mps: 2.2,
        wind_gain_mps: 2.3,
        gust_base: 0.08,
        gust_gain: 0.18,
        visibility_factor: 0.88,
        tags: &["weather.cloudy", "cloud.broken"],
        required_assets: &["clouds/cumulus_fields.ycloud@broken_cumulus"],
        phenomena: PHENOMENA_CLOUDY,
    },
    WeatherPatternDescriptor {
        id: "weather.overcast.stratus_deck",
        kind: WeatherKind::Overcast,
        weather_visual_ref: "weather/overcast.yweather@stratus_deck",
        intensity_min: 0.55,
        intensity_max: 0.84,
        cloud_floor: 0.72,
        overcast_bias: 0.34,
        precipitation_kind: PrecipitationKind::None,
        precipitation_factor: 0.0,
        thunder_factor: 0.0,
        wetness_factor: 0.0,
        snow_factor: 0.0,
        fog_factor: 0.02,
        haze_factor: 0.18,
        wind_base_mps: 2.8,
        wind_gain_mps: 3.0,
        gust_base: 0.12,
        gust_gain: 0.25,
        visibility_factor: 0.72,
        tags: &["weather.overcast", "cloud.overcast"],
        required_assets: &["clouds/stratus_fields.ycloud@overcast_sheet"],
        phenomena: PHENOMENA_OVERCAST,
    },
    WeatherPatternDescriptor {
        id: "weather.rain.nimbostratus",
        kind: WeatherKind::Rain,
        weather_visual_ref: "weather/rain.yweather@nimbostratus",
        intensity_min: 0.48,
        intensity_max: 0.92,
        cloud_floor: 0.78,
        overcast_bias: 0.46,
        precipitation_kind: PrecipitationKind::Rain,
        precipitation_factor: 0.68,
        thunder_factor: 0.02,
        wetness_factor: 0.92,
        snow_factor: 0.0,
        fog_factor: 0.04,
        haze_factor: 0.26,
        wind_base_mps: 3.4,
        wind_gain_mps: 4.8,
        gust_base: 0.18,
        gust_gain: 0.32,
        visibility_factor: 0.46,
        tags: &["weather.rain", "surface.wet", "audio.rain"],
        required_assets: &[
            "audio/weather.ybank@rain",
            "materials/wetness.nemat@default",
        ],
        phenomena: PHENOMENA_RAIN,
    },
    WeatherPatternDescriptor {
        id: "weather.storm.cumulonimbus",
        kind: WeatherKind::Storm,
        weather_visual_ref: "weather/storm.yweather@cumulonimbus",
        intensity_min: 0.70,
        intensity_max: 1.00,
        cloud_floor: 0.90,
        overcast_bias: 0.62,
        precipitation_kind: PrecipitationKind::Rain,
        precipitation_factor: 0.92,
        thunder_factor: 0.45,
        wetness_factor: 1.00,
        snow_factor: 0.0,
        fog_factor: 0.04,
        haze_factor: 0.32,
        wind_base_mps: 6.5,
        wind_gain_mps: 8.0,
        gust_base: 0.36,
        gust_gain: 0.58,
        visibility_factor: 0.26,
        tags: &[
            "weather.storm",
            "surface.wet",
            "audio.rain_heavy",
            "ai.shelter_preferred",
        ],
        required_assets: &[
            "clouds/cumulonimbus.ycloud@storm_core",
            "audio/weather.ybank@thunder",
            "audio/weather.ybank@rain",
            "materials/wetness.nemat@storm",
        ],
        phenomena: PHENOMENA_STORM,
    },
    WeatherPatternDescriptor {
        id: "weather.fog.ground_radiation",
        kind: WeatherKind::Fog,
        weather_visual_ref: "weather/fog.yweather@ground_radiation",
        intensity_min: 0.42,
        intensity_max: 0.90,
        cloud_floor: 0.48,
        overcast_bias: 0.18,
        precipitation_kind: PrecipitationKind::None,
        precipitation_factor: 0.0,
        thunder_factor: 0.0,
        wetness_factor: 0.06,
        snow_factor: 0.0,
        fog_factor: 0.62,
        haze_factor: 0.36,
        wind_base_mps: 0.5,
        wind_gain_mps: 1.4,
        gust_base: 0.02,
        gust_gain: 0.06,
        visibility_factor: 0.18,
        tags: &["weather.fog", "visibility.low", "ai.visibility_reduced"],
        required_assets: &["weather/fog.yweather@ground_radiation"],
        phenomena: PHENOMENA_FOG,
    },
    WeatherPatternDescriptor {
        id: "weather.snow.stratiform",
        kind: WeatherKind::Snow,
        weather_visual_ref: "weather/snow.yweather@stratiform",
        intensity_min: 0.50,
        intensity_max: 0.88,
        cloud_floor: 0.82,
        overcast_bias: 0.42,
        precipitation_kind: PrecipitationKind::Snow,
        precipitation_factor: 0.58,
        thunder_factor: 0.0,
        wetness_factor: 0.20,
        snow_factor: 0.84,
        fog_factor: 0.08,
        haze_factor: 0.24,
        wind_base_mps: 2.2,
        wind_gain_mps: 4.0,
        gust_base: 0.10,
        gust_gain: 0.24,
        visibility_factor: 0.36,
        tags: &["weather.snow", "surface.snow", "visibility.low"],
        required_assets: &[
            "clouds/snow_bands.ycloud@stratiform",
            "audio/weather.ybank@snow",
        ],
        phenomena: PHENOMENA_SNOW,
    },
    WeatherPatternDescriptor {
        id: "weather.dust_storm.front",
        kind: WeatherKind::DustStorm,
        weather_visual_ref: "weather/dust.yweather@front",
        intensity_min: 0.58,
        intensity_max: 0.94,
        cloud_floor: 0.60,
        overcast_bias: 0.30,
        precipitation_kind: PrecipitationKind::Dust,
        precipitation_factor: 0.72,
        thunder_factor: 0.0,
        wetness_factor: 0.0,
        snow_factor: 0.0,
        fog_factor: 0.18,
        haze_factor: 0.64,
        wind_base_mps: 7.0,
        wind_gain_mps: 9.0,
        gust_base: 0.44,
        gust_gain: 0.50,
        visibility_factor: 0.12,
        tags: &["weather.dust_storm", "visibility.very_low", "wind.strong"],
        required_assets: &[
            "weather/dust.yweather@front",
            "audio/weather.ybank@wind_heavy",
        ],
        phenomena: PHENOMENA_DUST,
    },
];

const DEFAULT_WEATHER_BANDS: &[WeatherBandDescriptor] = &[
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

const HIGHLANDS_WEATHER_BANDS: &[WeatherBandDescriptor] = DEFAULT_WEATHER_BANDS;
const FOREST_ROAD_WEATHER_BANDS: &[WeatherBandDescriptor] = &[
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
        pressure_max: 0.64,
        time_center: None,
        time_half_width: 0.0,
        score_bias: 0.15,
    },
    WeatherBandDescriptor {
        pattern_id: "weather.cloudy.fair_cumulus",
        pressure_min: 0.28,
        pressure_max: 0.88,
        time_center: None,
        time_half_width: 0.0,
        score_bias: 0.12,
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
const SNOW_WEATHER_BANDS: &[WeatherBandDescriptor] = &[
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
const DESERT_WEATHER_BANDS: &[WeatherBandDescriptor] = &[
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

const TABLES: &[WeatherTableDescriptor] = &[
    WeatherTableDescriptor {
        id: "weather/game_ready_forest_road.yweather@table",
        bands: FOREST_ROAD_WEATHER_BANDS,
    },
    WeatherTableDescriptor {
        id: "weather/game_ready_highlands.yweather@table",
        bands: HIGHLANDS_WEATHER_BANDS,
    },
    WeatherTableDescriptor {
        id: "weather/default_temperate.yweather@table",
        bands: DEFAULT_WEATHER_BANDS,
    },
    WeatherTableDescriptor {
        id: "weather/alpine_winter.yweather@table",
        bands: SNOW_WEATHER_BANDS,
    },
    WeatherTableDescriptor {
        id: "weather/desert_dust.yweather@table",
        bands: DESERT_WEATHER_BANDS,
    },
];

const PROFILES: &[EnvironmentProfileDescriptor] = &[
    EnvironmentProfileDescriptor {
        id: "environment.game_ready_forest_road",
        region: "game_ready.forest_road",
        biome: "temperate_forest",
        weather_table_ref: "weather/game_ready_forest_road.yweather@table",
        sky_profile_ref: "sky/temperate_forest_morning.ysky@gradient",
        cloud_profile_ref: "clouds/temperate_cumulus.ycloud@profile",
        wind_profile_ref: "wind/forest_canopy_breeze.ywind@profile",
        visual_assets: &GAME_READY_SKYDOME_VISUALS,
        latitude_degrees: 45.0,
        axial_tilt_degrees: 23.44,
    },
    EnvironmentProfileDescriptor {
        id: "environment.game_ready_highlands",
        region: "game_ready.highlands",
        biome: "highlands",
        weather_table_ref: "weather/game_ready_highlands.yweather@table",
        sky_profile_ref: "sky/highlands_day.ysky@gradient",
        cloud_profile_ref: "clouds/highlands_fields.ycloud@profile",
        wind_profile_ref: "wind/highlands_breeze.ywind@profile",
        visual_assets: &GAME_READY_SKYDOME_VISUALS,
        latitude_degrees: 45.0,
        axial_tilt_degrees: 23.44,
    },
    EnvironmentProfileDescriptor {
        id: "environment.default",
        region: "world.default",
        biome: "temperate",
        weather_table_ref: "weather/default_temperate.yweather@table",
        sky_profile_ref: "sky/default_temperate.ysky@gradient",
        cloud_profile_ref: "clouds/default_temperate.ycloud@profile",
        wind_profile_ref: "wind/default_breeze.ywind@profile",
        visual_assets: &GAME_READY_SKYDOME_VISUALS,
        latitude_degrees: 38.0,
        axial_tilt_degrees: 23.44,
    },
    EnvironmentProfileDescriptor {
        id: "environment.alpine_winter",
        region: "world.alpine",
        biome: "alpine",
        weather_table_ref: "weather/alpine_winter.yweather@table",
        sky_profile_ref: "sky/alpine_winter.ysky@gradient",
        cloud_profile_ref: "clouds/alpine_winter.ycloud@profile",
        wind_profile_ref: "wind/alpine_gusts.ywind@profile",
        visual_assets: &GAME_READY_SKYDOME_VISUALS,
        latitude_degrees: 58.0,
        axial_tilt_degrees: 23.44,
    },
    EnvironmentProfileDescriptor {
        id: "environment.desert_dusk",
        region: "world.desert",
        biome: "desert",
        weather_table_ref: "weather/desert_dust.yweather@table",
        sky_profile_ref: "sky/desert_dusk.ysky@gradient",
        cloud_profile_ref: "clouds/desert_dust.ycloud@profile",
        wind_profile_ref: "wind/desert_front.ywind@profile",
        visual_assets: &GAME_READY_SKYDOME_VISUALS,
        latitude_degrees: 27.0,
        axial_tilt_degrees: 23.44,
    },
];

pub(crate) fn default_profile() -> &'static EnvironmentProfileDescriptor {
    PROFILES
        .iter()
        .find(|profile| profile.id == "environment.default")
        .unwrap_or(&PROFILES[0])
}

pub(crate) fn profile_by_id(id: &str) -> (&'static EnvironmentProfileDescriptor, bool) {
    PROFILES
        .iter()
        .find(|profile| profile.id == id)
        .map(|profile| (profile, true))
        .unwrap_or_else(|| (default_profile(), false))
}

pub(crate) fn table_by_id(id: &str) -> &'static WeatherTableDescriptor {
    TABLES
        .iter()
        .find(|table| table.id == id)
        .unwrap_or(&TABLES[1])
}

pub(crate) fn pattern_by_id(id: &str) -> &'static WeatherPatternDescriptor {
    PATTERNS
        .iter()
        .find(|pattern| pattern.id == id)
        .unwrap_or(&PATTERNS[0])
}
