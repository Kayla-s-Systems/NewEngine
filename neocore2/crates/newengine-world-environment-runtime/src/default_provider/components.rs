use crate::math::{clamp01_f32, mix_color, normalize};
use newengine_world_environment_api::{
    AtmosphereStateDto, CelestialBodyDto, CloudStateDto, Color3Dto,
    EnvironmentGameplayModifiersDto, EnvironmentLightingIntentDto, ExposureIntentDto, SkyStateDto,
    TimeOfDayStateDto, Vec3Dto, WeatherStateDto, WindStateDto,
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

pub(super) struct AtmosphereInputs<'a> {
    pub time_of_day: &'a TimeOfDayStateDto,
    pub overcast: f32,
    pub precipitation: f32,
    pub fog_bias: f32,
    pub weather_intensity: f32,
    pub cloud_coverage: f32,
    pub haze: f32,
    pub visibility: f32,
}

pub(super) fn build_atmosphere_state(input: AtmosphereInputs<'_>) -> AtmosphereStateDto {
    let AtmosphereInputs {
        time_of_day,
        overcast,
        precipitation,
        fog_bias,
        weather_intensity,
        cloud_coverage,
        haze,
        visibility,
    } = input;
    AtmosphereStateDto {
        fog_density: 0.006
            + overcast * 0.024
            + precipitation * 0.020
            + fog_bias * weather_intensity,
        fog_height_falloff: 0.12,
        fog_color_linear: mix_color(
            Color3Dto::new(0.06, 0.07, 0.11),
            Color3Dto::new(0.56, 0.62, 0.70),
            time_of_day.day_blend,
        ),
        haze_amount: haze,
        humidity: clamp01_f32(
            0.26 + cloud_coverage * 0.30 + precipitation * 0.34 + fog_bias * 0.28,
        ),
        aerosol_density: 0.08 + haze,
        visibility_distance_meters: visibility,
    }
}

pub(super) fn build_wind_state(
    wind_base_mps: f32,
    wind_gain_mps: f32,
    gust_base: f32,
    gust_gain: f32,
    cloud_coverage: f32,
    weather_intensity: f32,
    overcast: f32,
) -> WindStateDto {
    WindStateDto {
        global_direction: normalize(Vec3Dto::new(0.92, 0.0, 0.38)),
        global_speed_mps: wind_base_mps + cloud_coverage * 1.6 + wind_gain_mps * weather_intensity,
        gust_strength: (gust_base + gust_gain * weather_intensity + overcast * 0.12)
            .clamp(0.0, 1.0),
        cloud_advection: Vec3Dto::new(
            wind_base_mps + cloud_coverage * 1.8 + wind_gain_mps * weather_intensity,
            0.0,
            0.8 + weather_intensity * 1.4,
        ),
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
        sun_glare_hint: sun.intensity_lux_hint / 105_000.0 * (1.0 - cloud_coverage * 0.75),
        interior_exterior_bias: 0.0,
    }
}
