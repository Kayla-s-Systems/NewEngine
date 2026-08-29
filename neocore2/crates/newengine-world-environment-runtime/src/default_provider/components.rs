use crate::math::{clamp01_f32, mix_color};
use newengine_world_environment_api::{
    CelestialBodyDto, CloudStateDto, Color3Dto, EnvironmentGameplayModifiersDto,
    EnvironmentLightingIntentDto, ExposureIntentDto, SkyStateDto, TimeOfDayStateDto,
    WeatherStateDto, WindStateDto,
};

pub(super) fn build_sky_state(time_of_day: &TimeOfDayStateDto, overcast: f32) -> SkyStateDto {
    SkyStateDto {
        zenith_color_linear: mix_color(
            Color3Dto::new(0.010, 0.014, 0.035),
            Color3Dto::new(0.18, 0.34, 0.62),
            time_of_day.day_blend,
        ),
        horizon_color_linear: mix_color(
            Color3Dto::new(0.020, 0.026, 0.058),
            Color3Dto::new(0.48, 0.62, 0.84),
            time_of_day.day_blend,
        ),
        sun_horizon_color_linear: mix_color(
            Color3Dto::new(0.14, 0.07, 0.04),
            Color3Dto::new(1.0, 0.48, 0.18),
            time_of_day.dawn_dusk_blend,
        ),
        opposite_horizon_color_linear: mix_color(
            Color3Dto::new(0.018, 0.030, 0.070),
            Color3Dto::new(0.32, 0.45, 0.68),
            time_of_day.day_blend,
        ),
        dusk_dawn_blend: time_of_day.dawn_dusk_blend,
        night_blend: time_of_day.night_blend,
        overcast_blend: overcast,
        light_pollution: 0.04 * time_of_day.night_blend,
    }
}

pub(super) fn build_lighting_intent(
    sun: &CelestialBodyDto,
    moon: &CelestialBodyDto,
    clouds: &CloudStateDto,
    time_of_day: &TimeOfDayStateDto,
    cloud_coverage: f32,
    weather: &WeatherStateDto,
    overcast: f32,
) -> EnvironmentLightingIntentDto {
    EnvironmentLightingIntentDto {
        sun_lux_hint: sun.intensity_lux_hint * (1.0 - clouds.light_absorption),
        moon_lux_hint: moon.intensity_lux_hint * (1.0 - clouds.light_absorption),
        ambient_intensity: (0.04 + time_of_day.day_blend * 0.22 + cloud_coverage * 0.06
            - weather.intensity * 0.025)
            .max(0.015),
        sky_light_intensity: (0.07 + time_of_day.day_blend * 0.45 - overcast * 0.12).max(0.02),
        cloud_shadow_strength: clouds.shadow_strength,
        wetness_specular_boost: weather.wetness.surface_wetness * 0.55,
    }
}

pub(super) fn build_gameplay_modifiers(
    weather: &WeatherStateDto,
    wind: &WindStateDto,
    visibility: f32,
    fog_bias: f32,
) -> EnvironmentGameplayModifiersDto {
    let precipitation = weather.precipitation.intensity;
    EnvironmentGameplayModifiersDto {
        visibility_multiplier: clamp01_f32(visibility / 20_000.0),
        audio_masking_multiplier: clamp01_f32(
            precipitation * 0.55 + weather.thunder.probability * 0.30 + wind.gust_strength * 0.12,
        ),
        weather_hazard_level: clamp01_f32(
            weather.thunder.probability * 0.85 + precipitation * 0.25 + fog_bias * 0.18,
        ),
        shelter_score: clamp01_f32(
            precipitation * 0.60
                + weather.thunder.probability * 0.95
                + weather.snow.surface_snow * 0.25,
        ),
        surface_slipperiness_hint: clamp01_f32(
            weather.wetness.surface_wetness * 0.82 + weather.snow.surface_snow * 0.30,
        ),
    }
}

pub(super) fn build_exposure_intent(
    time_of_day: &TimeOfDayStateDto,
    sun: &CelestialBodyDto,
    weather: &WeatherStateDto,
    overcast: f32,
    cloud_coverage: f32,
) -> ExposureIntentDto {
    ExposureIntentDto {
        night_adaptation_hint: time_of_day.night_blend,
        storm_darkening: weather.thunder.probability * 0.65 + overcast * 0.16,
        // Fractional coverage is not visibility along the Sun ray. Scattered
        // cumulus elsewhere in the sky should not globally suppress glare;
        // local sky/cloud sampling owns actual solar-disc occlusion.
        sun_glare_hint: sun.intensity_lux_hint / 105_000.0
            * (1.0 - overcast * 0.70)
            * (1.0 - cloud_coverage * 0.08),
        interior_exterior_bias: 0.0,
    }
}
