use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentLightingIntentDto {
    pub sun_lux_hint: f32,
    pub moon_lux_hint: f32,
    pub ambient_intensity: f32,
    pub sky_light_intensity: f32,
    pub cloud_shadow_strength: f32,
    pub wetness_specular_boost: f32,
}

impl Default for EnvironmentLightingIntentDto {
    #[inline]
    fn default() -> Self {
        Self {
            sun_lux_hint: 0.0,
            moon_lux_hint: 0.0,
            ambient_intensity: 0.05,
            sky_light_intensity: 0.05,
            cloud_shadow_strength: 0.0,
            wetness_specular_boost: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ExposureIntentDto {
    pub night_adaptation_hint: f32,
    pub storm_darkening: f32,
    pub sun_glare_hint: f32,
    pub interior_exterior_bias: f32,
}

impl Default for ExposureIntentDto {
    #[inline]
    fn default() -> Self {
        Self {
            night_adaptation_hint: 1.0,
            storm_darkening: 0.0,
            sun_glare_hint: 0.0,
            interior_exterior_bias: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentGameplayModifiersDto {
    pub visibility_multiplier: f32,
    pub audio_masking_multiplier: f32,
    pub weather_hazard_level: f32,
    pub shelter_score: f32,
    pub surface_slipperiness_hint: f32,
}

impl Default for EnvironmentGameplayModifiersDto {
    #[inline]
    fn default() -> Self {
        Self {
            visibility_multiplier: 1.0,
            audio_masking_multiplier: 0.0,
            weather_hazard_level: 0.0,
            shelter_score: 0.0,
            surface_slipperiness_hint: 0.0,
        }
    }
}
