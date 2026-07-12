use serde::{Deserialize, Serialize};

use crate::{Color3Dto, Vec3Dto};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CelestialBodyDto {
    pub direction_world: Vec3Dto,
    pub altitude_radians: f32,
    pub azimuth_radians: f32,
    pub angular_radius_radians: f32,
    pub color_linear: Color3Dto,
    pub intensity_lux_hint: f32,
    pub visible: bool,
}

impl Default for CelestialBodyDto {
    #[inline]
    fn default() -> Self {
        Self {
            direction_world: Vec3Dto::up(),
            altitude_radians: 0.0,
            azimuth_radians: 0.0,
            angular_radius_radians: 0.00465,
            color_linear: Color3Dto::white(),
            intensity_lux_hint: 0.0,
            visible: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CelestialStateDto {
    pub sun: CelestialBodyDto,
    pub moon: CelestialBodyDto,
    pub moon_phase: f32,
    pub stars_visibility: f32,
    pub night_sky_visibility: f32,
}

impl Default for CelestialStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            sun: CelestialBodyDto::default(),
            moon: CelestialBodyDto::default(),
            moon_phase: 0.5,
            stars_visibility: 0.0,
            night_sky_visibility: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SkyStateDto {
    pub zenith_color_linear: Color3Dto,
    pub horizon_color_linear: Color3Dto,
    pub sun_horizon_color_linear: Color3Dto,
    pub opposite_horizon_color_linear: Color3Dto,
    pub dusk_dawn_blend: f32,
    pub night_blend: f32,
    pub overcast_blend: f32,
    pub light_pollution: f32,
}

impl Default for SkyStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            zenith_color_linear: Color3Dto::new(0.10, 0.15, 0.24),
            horizon_color_linear: Color3Dto::new(0.12, 0.14, 0.18),
            sun_horizon_color_linear: Color3Dto::new(0.20, 0.16, 0.12),
            opposite_horizon_color_linear: Color3Dto::new(0.08, 0.10, 0.14),
            dusk_dawn_blend: 0.0,
            night_blend: 1.0,
            overcast_blend: 0.0,
            light_pollution: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AtmosphereStateDto {
    pub fog_density: f32,
    pub fog_height_falloff: f32,
    pub fog_color_linear: Color3Dto,
    pub haze_amount: f32,
    pub humidity: f32,
    pub aerosol_density: f32,
    pub visibility_distance_meters: f32,
}

impl Default for AtmosphereStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            fog_density: 0.0,
            fog_height_falloff: 0.12,
            fog_color_linear: Color3Dto::new(0.45, 0.50, 0.58),
            haze_amount: 0.05,
            humidity: 0.25,
            aerosol_density: 0.10,
            visibility_distance_meters: 20_000.0,
        }
    }
}
