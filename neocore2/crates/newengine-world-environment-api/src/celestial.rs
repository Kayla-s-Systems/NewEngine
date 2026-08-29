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
#[serde(default)]
pub struct AtmosphericLayerDto {
    /// Height above the local surface, meters.
    pub altitude_agl_meters: f32,
    pub pressure_hpa: f32,
    pub temperature_celsius: f32,
    pub relative_humidity: f32,
    pub specific_humidity_g_per_kg: f32,
    pub cloud_water_content_g_m3: f32,
    pub ice_fraction: f32,
    pub vertical_velocity_mps: f32,
}

impl Default for AtmosphericLayerDto {
    fn default() -> Self {
        Self {
            altitude_agl_meters: 0.0,
            pressure_hpa: 1013.25,
            temperature_celsius: 15.0,
            relative_humidity: 0.45,
            specific_humidity_g_per_kg: 4.8,
            cloud_water_content_g_m3: 0.0,
            ice_fraction: 0.0,
            vertical_velocity_mps: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AtmosphereStateDto {
    pub fog_density: f32,
    pub fog_height_falloff: f32,
    pub fog_color_linear: Color3Dto,
    pub haze_amount: f32,
    /// Relative humidity in [0, 1].
    pub humidity: f32,
    pub aerosol_density: f32,
    pub visibility_distance_meters: f32,
    /// Prognostic/diagnostic thermodynamic column state.
    pub surface_pressure_hpa: f32,
    pub temperature_celsius: f32,
    pub dew_point_celsius: f32,
    pub specific_humidity_g_per_kg: f32,
    pub vapor_pressure_hpa: f32,
    pub saturation_vapor_pressure_hpa: f32,
    pub air_density_kg_m3: f32,
    pub lifting_condensation_level_meters: f32,
    pub precipitable_water_mm: f32,
    pub cloud_water_path_kg_m2: f32,
    pub condensation_potential: f32,
    /// Five-level thermodynamic column (surface, 1 km, 2.5 km, 5 km, 9 km AGL).
    pub vertical_layers: [AtmosphericLayerDto; 5],
    /// Parcel-method convective diagnostics integrated through the resolved column.
    pub cape_j_per_kg: f32,
    pub cin_j_per_kg: f32,
    pub convective_cloud_top_meters: f32,
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
            surface_pressure_hpa: 1013.25,
            temperature_celsius: 15.0,
            dew_point_celsius: -4.0,
            specific_humidity_g_per_kg: 2.6,
            vapor_pressure_hpa: 4.1,
            saturation_vapor_pressure_hpa: 17.0,
            air_density_kg_m3: 1.225,
            lifting_condensation_level_meters: 2350.0,
            precipitable_water_mm: 9.0,
            cloud_water_path_kg_m2: 0.0,
            condensation_potential: 0.0,
            vertical_layers: [
                AtmosphericLayerDto::default(),
                AtmosphericLayerDto {
                    altitude_agl_meters: 1000.0,
                    pressure_hpa: 899.0,
                    temperature_celsius: 8.5,
                    relative_humidity: 0.40,
                    specific_humidity_g_per_kg: 3.2,
                    ..AtmosphericLayerDto::default()
                },
                AtmosphericLayerDto {
                    altitude_agl_meters: 2500.0,
                    pressure_hpa: 750.0,
                    temperature_celsius: -1.3,
                    relative_humidity: 0.34,
                    specific_humidity_g_per_kg: 1.6,
                    ..AtmosphericLayerDto::default()
                },
                AtmosphericLayerDto {
                    altitude_agl_meters: 5000.0,
                    pressure_hpa: 560.0,
                    temperature_celsius: -17.0,
                    relative_humidity: 0.28,
                    specific_humidity_g_per_kg: 0.50,
                    ice_fraction: 0.72,
                    ..AtmosphericLayerDto::default()
                },
                AtmosphericLayerDto {
                    altitude_agl_meters: 9000.0,
                    pressure_hpa: 350.0,
                    temperature_celsius: -43.0,
                    relative_humidity: 0.22,
                    specific_humidity_g_per_kg: 0.05,
                    ice_fraction: 1.0,
                    ..AtmosphericLayerDto::default()
                },
            ],
            cape_j_per_kg: 0.0,
            cin_j_per_kg: 0.0,
            convective_cloud_top_meters: 1200.0,
        }
    }
}
