use newengine_world_environment_api::{
    AtmosphereStateDto, EnvironmentAtmosphereCellDto, EnvironmentFrameDto, PrecipitationKind,
    WeatherStateDto, WindStateDto,
};

#[derive(Clone, Copy, Debug)]
pub(super) struct SurfaceMemory {
    pub world_time_seconds: f64,
    pub surface_water_mm: f32,
    pub snow_water_equivalent_mm: f32,
}

impl SurfaceMemory {
    pub(super) fn from_frame(frame: &EnvironmentFrameDto) -> Self {
        Self {
            world_time_seconds: frame.world_time_seconds,
            surface_water_mm: frame.weather.wetness.surface_water_mm,
            snow_water_equivalent_mm: frame.weather.snow.snow_water_equivalent_mm,
        }
    }

    pub(super) fn from_cell(world_time_seconds: f64, cell: &EnvironmentAtmosphereCellDto) -> Self {
        Self {
            world_time_seconds,
            surface_water_mm: cell.weather.wetness.surface_water_mm,
            snow_water_equivalent_mm: cell.weather.snow.snow_water_equivalent_mm,
        }
    }
}

const LIQUID_FILM_CAPACITY_MM: f32 = 1.5;
const SNOW_REFERENCE_SWE_MM: f32 = 40.0;

#[allow(clippy::too_many_arguments)]
pub(super) fn integrate(
    weather: &mut WeatherStateDto,
    atmosphere: &AtmosphereStateDto,
    wind: &WindStateDto,
    sun_elevation_sine: f32,
    evaporation_flux_kg_m2_s: f32,
    previous: Option<SurfaceMemory>,
    world_time_seconds: f64,
) {
    let dt = previous
        .map(|state| (world_time_seconds - state.world_time_seconds) as f32)
        .filter(|dt| dt.is_finite() && *dt > 0.0 && *dt <= 600.0)
        .unwrap_or(0.0);
    let previous_water = previous.map(|state| state.surface_water_mm).unwrap_or(0.0);
    let previous_swe = previous
        .map(|state| state.snow_water_equivalent_mm)
        .unwrap_or(0.0);
    let rate_mm_s = weather.precipitation.rate_mm_per_hour.max(0.0) / 3600.0;
    let rain_rate = if matches!(weather.precipitation.kind, PrecipitationKind::Rain) {
        rate_mm_s
    } else {
        0.0
    };
    let snow_rate = if matches!(weather.precipitation.kind, PrecipitationKind::Snow) {
        rate_mm_s
    } else {
        0.0
    };

    let melt_rate_mm_s = if atmosphere.temperature_celsius > -0.5 && previous_swe > 0.0 {
        (0.000_015
            + atmosphere.temperature_celsius.max(0.0) * 0.000_018
            + sun_elevation_sine.max(0.0) * 0.000_16)
            .clamp(0.0, 0.0015)
    } else {
        0.0
    };
    let actual_melt = if dt > 0.0 {
        (melt_rate_mm_s * dt).min(previous_swe)
    } else {
        0.0
    };
    let evaporation_rate = (evaporation_flux_kg_m2_s
        * (0.35 + wind.global_speed_mps.min(12.0) / 24.0)
        * (1.0 - atmosphere.humidity * 0.55))
        .clamp(0.0, 0.000_5);

    let surface_water_mm = if dt > 0.0 {
        (previous_water + (rain_rate - evaporation_rate) * dt + actual_melt).clamp(0.0, 8.0)
    } else {
        (rain_rate * 60.0).clamp(0.0, 8.0)
    };
    let snow_swe_mm = if dt > 0.0 {
        (previous_swe + snow_rate * dt - actual_melt).clamp(0.0, 500.0)
    } else {
        (snow_rate * 60.0).clamp(0.0, 500.0)
    };

    weather.wetness.surface_water_mm = surface_water_mm;
    weather.wetness.surface_wetness = (surface_water_mm / LIQUID_FILM_CAPACITY_MM).clamp(0.0, 1.0);
    weather.wetness.accumulation_rate = rain_rate;
    weather.wetness.drying_rate = evaporation_rate;

    weather.snow.snow_water_equivalent_mm = snow_swe_mm;
    weather.snow.surface_snow = (snow_swe_mm / SNOW_REFERENCE_SWE_MM).clamp(0.0, 1.0);
    weather.snow.accumulation_rate = snow_rate;
    weather.snow.melt_rate = melt_rate_mm_s;
}
