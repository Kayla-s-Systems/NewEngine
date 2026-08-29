use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct EnvironmentProfileRefDto {
    #[serde(default)]
    pub profile_id: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct EnvironmentObjectId {
    pub stable_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeatherKind {
    Clear,
    Cloudy,
    Overcast,
    Rain,
    Storm,
    Snow,
    Fog,
    DustStorm,
    HeatHaze,
}

/// World-authored constraint on the meteorological state exposed to consumers.
///
/// `Dynamic` leaves the physically simulated atmosphere/weather authoritative.
/// `ClearSky` is a real environment condition, not a renderer-only cloud toggle: it
/// removes cloud optical depth, precipitation and storm exposure before render/audio/AI
/// packets are built, while keeping the underlying dry-air/aerosol atmosphere intact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EnvironmentWeatherConstraint {
    #[default]
    Dynamic,
    ClearSky,
}

impl Default for WeatherKind {
    #[inline]
    fn default() -> Self {
        Self::Clear
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrecipitationKind {
    None,
    Rain,
    Snow,
    Dust,
}

impl Default for PrecipitationKind {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnvironmentObjectKind {
    CloudField,
    CloudVolume,
    StormCell,
    FogBank,
    WeatherFront,
    DustWall,
    SnowBand,
    HeatHazeZone,
}

impl Default for EnvironmentObjectKind {
    #[inline]
    fn default() -> Self {
        Self::CloudField
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeOfDayPhase {
    Night,
    Dawn,
    Day,
    Dusk,
}

impl Default for TimeOfDayPhase {
    #[inline]
    fn default() -> Self {
        Self::Night
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimeOfDayStateDto {
    pub normalized_day: f32,
    pub hours: f32,
    pub phase: TimeOfDayPhase,
    pub dawn_dusk_blend: f32,
    pub day_blend: f32,
    pub night_blend: f32,
}

impl Default for TimeOfDayStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            normalized_day: 0.0,
            hours: 0.0,
            phase: TimeOfDayPhase::Night,
            dawn_dusk_blend: 0.0,
            day_blend: 0.0,
            night_blend: 1.0,
        }
    }
}
