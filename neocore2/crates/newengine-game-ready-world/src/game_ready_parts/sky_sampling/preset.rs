#[derive(Clone, Copy)]
pub(super) struct SkyCloudVisualPreset {
    pub(super) softness: f32,
    pub(super) haze_bias: f32,
    pub(super) shadow_scale: f32,
    pub(super) day_tint: [f32; 3],
    pub(super) night_tint: [f32; 3],
    pub(super) rayleigh_scale: f32,
    pub(super) mie_scale: f32,
}

pub(super) fn sky_cloud_visual_preset(
    weather: newengine_world_environment_api::WeatherKind,
) -> SkyCloudVisualPreset {
    use newengine_world_environment_api::WeatherKind;

    match weather {
        WeatherKind::Clear => SkyCloudVisualPreset {
            // Fair-weather cumulus needs a coherent body; softness controls only
            // the eroded fringe, not a globally blurred cloud silhouette.
            softness: 0.74,
            haze_bias: -0.02,
            shadow_scale: 0.45,
            day_tint: [1.03, 1.02, 1.00],
            night_tint: [0.90, 0.96, 1.08],
            rayleigh_scale: 1.06,
            mie_scale: 0.86,
        },
        WeatherKind::Cloudy => SkyCloudVisualPreset {
            softness: 0.66,
            haze_bias: 0.01,
            shadow_scale: 0.92,
            day_tint: [1.00, 0.99, 0.98],
            night_tint: [0.90, 0.96, 1.07],
            rayleigh_scale: 0.98,
            mie_scale: 1.00,
        },
        WeatherKind::Overcast => SkyCloudVisualPreset {
            softness: 0.56,
            haze_bias: 0.05,
            shadow_scale: 1.04,
            day_tint: [0.82, 0.88, 0.98],
            night_tint: [0.76, 0.84, 1.02],
            rayleigh_scale: 0.84,
            mie_scale: 1.18,
        },
        WeatherKind::Rain => SkyCloudVisualPreset {
            softness: 0.52,
            haze_bias: 0.09,
            shadow_scale: 1.12,
            day_tint: [0.70, 0.78, 0.90],
            night_tint: [0.66, 0.75, 0.92],
            rayleigh_scale: 0.74,
            mie_scale: 1.32,
        },
        WeatherKind::Storm => SkyCloudVisualPreset {
            softness: 0.42,
            haze_bias: 0.13,
            shadow_scale: 1.25,
            day_tint: [0.52, 0.60, 0.72],
            night_tint: [0.50, 0.59, 0.78],
            rayleigh_scale: 0.64,
            mie_scale: 1.48,
        },
        WeatherKind::Snow => SkyCloudVisualPreset {
            softness: 0.66,
            haze_bias: 0.07,
            shadow_scale: 0.88,
            day_tint: [1.05, 1.09, 1.17],
            night_tint: [0.82, 0.91, 1.10],
            rayleigh_scale: 0.94,
            mie_scale: 1.12,
        },
        WeatherKind::Fog => SkyCloudVisualPreset {
            softness: 0.92,
            haze_bias: 0.16,
            shadow_scale: 0.34,
            day_tint: [0.90, 0.95, 1.01],
            night_tint: [0.76, 0.85, 0.99],
            rayleigh_scale: 0.68,
            mie_scale: 1.52,
        },
        WeatherKind::DustStorm => SkyCloudVisualPreset {
            softness: 0.72,
            haze_bias: 0.22,
            shadow_scale: 0.56,
            day_tint: [1.16, 0.88, 0.64],
            night_tint: [0.88, 0.66, 0.50],
            rayleigh_scale: 0.58,
            mie_scale: 1.78,
        },
        WeatherKind::HeatHaze => SkyCloudVisualPreset {
            softness: 0.90,
            haze_bias: 0.10,
            shadow_scale: 0.22,
            day_tint: [1.10, 0.99, 0.82],
            night_tint: [0.90, 0.82, 0.72],
            rayleigh_scale: 0.82,
            mie_scale: 1.32,
        },
    }
}
