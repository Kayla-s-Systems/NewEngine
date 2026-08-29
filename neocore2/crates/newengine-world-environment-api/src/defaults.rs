use crate::{
    AtmosphereStateDto, CelestialStateDto, CloudStateDto, EnvironmentConsumerPacketsDto,
    EnvironmentDiagnosticsDto, EnvironmentFrameDto, EnvironmentGameplayModifiersDto,
    EnvironmentGlobalStateDto, EnvironmentLightingIntentDto, EnvironmentSnapshotResponse,
    EnvironmentVisualAssetRefsDto, ExposureIntentDto, SkyStateDto, TimeOfDayStateDto, Vec3Dto,
    WeatherStateDto, WindStateDto,
};

impl EnvironmentFrameDto {
    #[inline]
    pub fn neutral_degraded(
        frame_id: u64,
        world_instance_id: impl Into<String>,
        key: impl Into<String>,
    ) -> Self {
        let world_instance_id = world_instance_id.into();
        let key = key.into();
        Self {
            frame_id,
            world_instance_id,
            world_time_seconds: 0.0,
            time_of_day_normalized: 0.0,
            day_index: 0,
            time_of_day_state: TimeOfDayStateDto::default(),
            global: EnvironmentGlobalStateDto::default(),
            visual_assets: EnvironmentVisualAssetRefsDto::default(),
            celestial: CelestialStateDto::default(),
            sky: SkyStateDto::default(),
            atmosphere: AtmosphereStateDto::default(),
            weather: WeatherStateDto::default(),
            clouds: CloudStateDto {
                coverage: 0.0,
                overcast: 0.0,
                shadow_strength: 0.0,
                light_absorption: 0.0,
                layers: Vec::new(),
                volumes: Vec::new(),
                storm_cells: Vec::new(),
            },
            wind: WindStateDto {
                global_direction: Vec3Dto::zero(),
                global_speed_mps: 0.0,
                gust_strength: 0.0,
                cloud_advection: Vec3Dto::zero(),
            },
            lighting_intent: EnvironmentLightingIntentDto::default(),
            gameplay_modifiers: EnvironmentGameplayModifiersDto::default(),
            exposure_intent: ExposureIntentDto::default(),
            environment_objects: Vec::new(),
            spatial_cell_size_meters: 0.0,
            spatial_atmosphere: Vec::new(),
            consumer_packets: EnvironmentConsumerPacketsDto::default(),
            diagnostics: EnvironmentDiagnosticsDto {
                provider: "environment.null".to_owned(),
                provider_route: "engine.world.null.environment".to_owned(),
                degraded: true,
                deterministic_key: key,
                active_profile: "environment.null".to_owned(),
                reasons: vec!["neutral degraded environment frame".to_owned()],
                warnings: Vec::new(),
            },
        }
    }
}

impl Default for EnvironmentFrameDto {
    #[inline]
    fn default() -> Self {
        Self::neutral_degraded(0, "world.runtime.default", "environment.default")
    }
}

impl Default for EnvironmentSnapshotResponse {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.world.environment.snapshot.v1".to_owned(),
            frame: EnvironmentFrameDto::default(),
        }
    }
}
