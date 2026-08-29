use serde::{Deserialize, Serialize};

use crate::{EnvironmentObjectDto, PrecipitationKind, Vec3Dto, WeatherKind};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PrecipitationStateDto {
    pub kind: PrecipitationKind,
    /// Normalized consumer-facing severity. Mass conservation must use rate_mm_per_hour.
    pub intensity: f32,
    /// Surface-equivalent hydrometeor mass flux in millimeters of liquid water per hour.
    #[serde(default)]
    pub rate_mm_per_hour: f32,
}

impl Default for PrecipitationStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            kind: PrecipitationKind::None,
            intensity: 0.0,
            rate_mm_per_hour: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ThunderStateDto {
    pub probability: f32,
    pub distance_meters: f32,
}

impl Default for ThunderStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            probability: 0.0,
            distance_meters: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WetnessStateDto {
    pub surface_wetness: f32,
    /// Explicit liquid-water storage on exposed surfaces [mm].
    #[serde(default)]
    pub surface_water_mm: f32,
    pub accumulation_rate: f32,
    pub drying_rate: f32,
}

impl Default for WetnessStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            surface_wetness: 0.0,
            surface_water_mm: 0.0,
            accumulation_rate: 0.0,
            drying_rate: 0.02,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SnowStateDto {
    pub surface_snow: f32,
    /// Snow-water equivalent stored on the surface [mm].
    #[serde(default)]
    pub snow_water_equivalent_mm: f32,
    pub accumulation_rate: f32,
    pub melt_rate: f32,
}

impl Default for SnowStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            surface_snow: 0.0,
            snow_water_equivalent_mm: 0.0,
            accumulation_rate: 0.0,
            melt_rate: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherStateDto {
    pub weather_id: String,
    pub state: WeatherKind,
    pub intensity: f32,
    pub transition_progress: f32,
    pub precipitation: PrecipitationStateDto,
    pub thunder: ThunderStateDto,
    pub wetness: WetnessStateDto,
    pub snow: SnowStateDto,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Default for WeatherStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            weather_id: "weather.clear".to_owned(),
            state: WeatherKind::Clear,
            intensity: 0.0,
            transition_progress: 1.0,
            precipitation: PrecipitationStateDto::default(),
            thunder: ThunderStateDto::default(),
            wetness: WetnessStateDto::default(),
            snow: SnowStateDto::default(),
            tags: vec!["weather.clear".to_owned(), "visibility.normal".to_owned()],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CloudLayerDto {
    pub altitude_min_meters: f32,
    pub altitude_max_meters: f32,
    pub coverage: f32,
    pub density: f32,
    pub wind_velocity: Vec3Dto,
}

impl Default for CloudLayerDto {
    #[inline]
    fn default() -> Self {
        Self {
            altitude_min_meters: 1800.0,
            altitude_max_meters: 3200.0,
            coverage: 0.15,
            density: 0.20,
            wind_velocity: Vec3Dto::new(1.0, 0.0, 0.35),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CloudStateDto {
    pub coverage: f32,
    pub overcast: f32,
    pub shadow_strength: f32,
    pub light_absorption: f32,
    pub layers: Vec<CloudLayerDto>,
    pub volumes: Vec<EnvironmentObjectDto>,
    pub storm_cells: Vec<EnvironmentObjectDto>,
}

impl Default for CloudStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            coverage: 0.15,
            overcast: 0.0,
            shadow_strength: 0.05,
            light_absorption: 0.04,
            layers: vec![CloudLayerDto::default()],
            volumes: Vec::new(),
            storm_cells: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindStateDto {
    pub global_direction: Vec3Dto,
    pub global_speed_mps: f32,
    pub gust_strength: f32,
    pub cloud_advection: Vec3Dto,
}

impl Default for WindStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            global_direction: Vec3Dto::new(1.0, 0.0, 0.35),
            global_speed_mps: 2.0,
            gust_strength: 0.1,
            cloud_advection: Vec3Dto::new(2.0, 0.0, 0.7),
        }
    }
}
