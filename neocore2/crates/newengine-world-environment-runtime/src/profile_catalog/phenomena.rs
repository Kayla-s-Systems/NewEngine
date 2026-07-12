use newengine_world_environment_api::EnvironmentObjectKind;

use super::PhenomenonTemplateDescriptor;

pub(super) const FAIR_CLOUD_FIELD: PhenomenonTemplateDescriptor = PhenomenonTemplateDescriptor {
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

pub(super) const LOCAL_CUMULUS_VOLUME: PhenomenonTemplateDescriptor =
    PhenomenonTemplateDescriptor {
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

pub(super) const PHENOMENA_CLEAR: &[PhenomenonTemplateDescriptor] = &[FAIR_CLOUD_FIELD];

pub(super) const PHENOMENA_CLOUDY: &[PhenomenonTemplateDescriptor] = &[
    PhenomenonTemplateDescriptor {
        activation_threshold: 0.18,
        template_id: "cloud_field.broken_cumulus",
        required_assets: &["clouds/cumulus_fields.ycloud@broken_cumulus"],
        ..FAIR_CLOUD_FIELD
    },
    LOCAL_CUMULUS_VOLUME,
];

pub(super) const PHENOMENA_OVERCAST: &[PhenomenonTemplateDescriptor] = &[
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

pub(super) const PHENOMENA_RAIN: &[PhenomenonTemplateDescriptor] = &[
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

pub(super) const PHENOMENA_STORM: &[PhenomenonTemplateDescriptor] = &[
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

pub(super) const PHENOMENA_FOG: &[PhenomenonTemplateDescriptor] = &[PhenomenonTemplateDescriptor {
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

pub(super) const PHENOMENA_SNOW: &[PhenomenonTemplateDescriptor] =
    &[PhenomenonTemplateDescriptor {
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

pub(super) const PHENOMENA_DUST: &[PhenomenonTemplateDescriptor] =
    &[PhenomenonTemplateDescriptor {
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
