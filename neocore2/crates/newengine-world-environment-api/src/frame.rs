use newengine_time_api::TimeSnapshotV1;
use newengine_world_api::WorldCellCoord;
use serde::{Deserialize, Serialize};

use crate::{
    AtmosphereStateDto, CelestialStateDto, CloudStateDto, EnvironmentConsumerPacketsDto,
    EnvironmentDiagnosticsDto, EnvironmentGameplayModifiersDto, EnvironmentLightingIntentDto,
    EnvironmentObjectDto, EnvironmentProfileRefDto, ExposureIntentDto, SkyStateDto,
    TimeOfDayStateDto, Vec3Dto, WeatherStateDto, WindStateDto,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentFrameRequest {
    pub frame_id: u64,
    pub world_instance_id: String,
    pub time: TimeSnapshotV1,
    pub observer_position: Vec3Dto,
    pub observer_cell: Option<WorldCellCoord>,
    pub active_region: Option<String>,
    pub active_biome: Option<String>,
    pub resident_cells: Vec<WorldCellCoord>,
    pub environment_profile: EnvironmentProfileRefDto,
    pub seed: u64,
}

impl Default for EnvironmentFrameRequest {
    #[inline]
    fn default() -> Self {
        Self {
            frame_id: 0,
            world_instance_id: "world.runtime.default".to_owned(),
            time: TimeSnapshotV1::default(),
            observer_position: Vec3Dto::zero(),
            observer_cell: None,
            active_region: None,
            active_biome: None,
            resident_cells: Vec::new(),
            environment_profile: EnvironmentProfileRefDto {
                profile_id: "environment.default".to_owned(),
            },
            seed: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentVisualAssetRefsDto {
    pub visual_group_id: String,
    pub texture_dictionary_ref: String,
    pub sky_texture_ref: String,
    pub starfield_texture_ref: String,
    pub cloud_field_ref: String,
    pub cloud_density_texture_ref: String,
    pub cloud_detail_texture_ref: String,
    pub cloud_dither_texture_ref: String,
    pub sun_disk_texture_ref: String,
    pub moon_disk_texture_ref: String,
    pub weather_table_ref: String,
    pub weather_visual_ref: String,
}

impl Default for EnvironmentVisualAssetRefsDto {
    #[inline]
    fn default() -> Self {
        Self {
            visual_group_id: String::new(),
            texture_dictionary_ref: String::new(),
            sky_texture_ref: String::new(),
            starfield_texture_ref: String::new(),
            cloud_field_ref: String::new(),
            cloud_density_texture_ref: String::new(),
            cloud_detail_texture_ref: String::new(),
            cloud_dither_texture_ref: String::new(),
            sun_disk_texture_ref: String::new(),
            moon_disk_texture_ref: String::new(),
            weather_table_ref: String::new(),
            weather_visual_ref: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentFrameDto {
    pub frame_id: u64,
    pub world_instance_id: String,
    pub world_time_seconds: f64,
    pub time_of_day_normalized: f32,
    pub day_index: u32,
    #[serde(default)]
    pub time_of_day_state: TimeOfDayStateDto,
    pub global: EnvironmentGlobalStateDto,
    #[serde(default)]
    pub visual_assets: EnvironmentVisualAssetRefsDto,
    pub celestial: CelestialStateDto,
    pub sky: SkyStateDto,
    pub atmosphere: AtmosphereStateDto,
    pub weather: WeatherStateDto,
    pub clouds: CloudStateDto,
    pub wind: WindStateDto,
    pub lighting_intent: EnvironmentLightingIntentDto,
    pub gameplay_modifiers: EnvironmentGameplayModifiersDto,
    pub exposure_intent: ExposureIntentDto,
    #[serde(default)]
    pub environment_objects: Vec<EnvironmentObjectDto>,
    #[serde(default)]
    pub consumer_packets: EnvironmentConsumerPacketsDto,
    pub diagnostics: EnvironmentDiagnosticsDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentGlobalStateDto {
    pub active_region: Option<String>,
    pub active_biome: Option<String>,
    pub active_weather_profile: String,
    pub active_environment_profile: String,
    #[serde(default)]
    pub weather_table_ref: String,
    #[serde(default)]
    pub sky_profile_ref: String,
    #[serde(default)]
    pub cloud_profile_ref: String,
    #[serde(default)]
    pub wind_profile_ref: String,
    pub environment_seed: u64,
}

impl Default for EnvironmentGlobalStateDto {
    #[inline]
    fn default() -> Self {
        Self {
            active_region: None,
            active_biome: None,
            active_weather_profile: "weather.clear".to_owned(),
            active_environment_profile: "environment.default".to_owned(),
            weather_table_ref: String::new(),
            sky_profile_ref: String::new(),
            cloud_profile_ref: String::new(),
            wind_profile_ref: String::new(),
            environment_seed: 0,
        }
    }
}
