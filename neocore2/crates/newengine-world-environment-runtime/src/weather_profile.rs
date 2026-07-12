use crate::math::{circular_window, clamp01_f32, range_lerp};
use crate::profile_catalog::{
    pattern_by_id, table_by_id, EnvironmentProfileDescriptor, WeatherPatternDescriptor,
};
use newengine_world_environment_api::{
    PrecipitationKind, PrecipitationStateDto, SnowStateDto, ThunderStateDto, TimeOfDayPhase,
    WeatherStateDto, WetnessStateDto,
};

#[derive(Clone, Debug)]
pub(crate) struct WeatherEvaluation {
    pub pattern: &'static WeatherPatternDescriptor,
    pub weather: WeatherStateDto,
    pub cloud_floor: f32,
    pub overcast_bias: f32,
    pub fog_bias: f32,
    pub haze_bias: f32,
    pub wind_base_mps: f32,
    pub wind_gain_mps: f32,
    pub gust_base: f32,
    pub gust_gain: f32,
    pub visibility_factor: f32,
}

pub(crate) fn evaluate_weather(
    profile: &EnvironmentProfileDescriptor,
    tod: f32,
    pressure: f32,
    cloud_seed: f32,
) -> WeatherEvaluation {
    let table = table_by_id(profile.weather_table_ref);
    let selected_pattern = select_pattern(table.bands, pressure, tod, cloud_seed);
    let pattern = pattern_by_id(selected_pattern);
    let normalized = band_normalized_intensity(
        pattern.intensity_min,
        pattern.intensity_max,
        pressure,
        cloud_seed,
    );
    let intensity = range_lerp(pattern.intensity_min, pattern.intensity_max, normalized);
    let precipitation = precipitation_state(pattern, intensity);
    let thunder = ThunderStateDto {
        probability: pattern.thunder_factor * intensity,
        distance_meters: 600.0 + (1.0 - pressure).clamp(0.0, 1.0) * 2600.0,
    };
    let wetness = WetnessStateDto {
        surface_wetness: precipitation.intensity * pattern.wetness_factor,
        accumulation_rate: precipitation.intensity * 0.12 * pattern.wetness_factor,
        drying_rate: drying_rate(pattern.precipitation_kind),
    };
    let snow = SnowStateDto {
        surface_snow: intensity * pattern.snow_factor * 0.42,
        accumulation_rate: intensity * pattern.snow_factor * 0.055,
        melt_rate: snow_melt_rate(pattern.precipitation_kind),
    };
    let tags = pattern
        .tags
        .iter()
        .map(|tag| (*tag).to_owned())
        .collect::<Vec<_>>();
    WeatherEvaluation {
        pattern,
        weather: WeatherStateDto {
            weather_id: pattern.id.to_owned(),
            state: pattern.kind,
            intensity,
            transition_progress: transition_progress(tod, pressure),
            precipitation,
            thunder,
            wetness,
            snow,
            tags,
        },
        cloud_floor: pattern.cloud_floor,
        overcast_bias: pattern.overcast_bias,
        fog_bias: pattern.fog_factor,
        haze_bias: pattern.haze_factor,
        wind_base_mps: pattern.wind_base_mps,
        wind_gain_mps: pattern.wind_gain_mps,
        gust_base: pattern.gust_base,
        gust_gain: pattern.gust_gain,
        visibility_factor: pattern.visibility_factor,
    }
}

pub(crate) fn enrich_weather_tags(
    weather: &mut WeatherStateDto,
    phase: TimeOfDayPhase,
    visibility_meters: f32,
    cloud_coverage: f32,
) {
    let mut tags = weather.tags.clone();
    tags.extend(phase_tags(phase).iter().map(|tag| (*tag).to_owned()));
    tags.push(cloud_tag(cloud_coverage).to_owned());
    tags.push(visibility_tag(visibility_meters).to_owned());
    if weather.precipitation.intensity > 0.05 {
        tags.push("surface.wet".to_owned());
    }
    tags.sort();
    tags.dedup();
    weather.tags = tags;
}

fn select_pattern(
    bands: &[crate::profile_catalog::WeatherBandDescriptor],
    pressure: f32,
    tod: f32,
    cloud_seed: f32,
) -> &str {
    bands
        .iter()
        .map(|band| (band, band_score(*band, pressure, tod, cloud_seed)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(band, _)| band.pattern_id)
        .unwrap_or("weather.clear.dry_high_pressure")
}

fn band_score(
    band: crate::profile_catalog::WeatherBandDescriptor,
    pressure: f32,
    tod: f32,
    cloud_seed: f32,
) -> f32 {
    let pressure_mid = (band.pressure_min + band.pressure_max) * 0.5;
    let pressure_half = ((band.pressure_max - band.pressure_min) * 0.5).max(0.001);
    let pressure_score = 1.0 - ((pressure - pressure_mid).abs() / pressure_half).clamp(0.0, 1.0);
    let time_score = band
        .time_center
        .map(|center| circular_window(tod, center, band.time_half_width).max(0.0))
        .unwrap_or(1.0);
    pressure_score * 0.72 + time_score * 0.20 + cloud_seed * 0.04 + band.score_bias
}

fn band_normalized_intensity(min: f32, max: f32, pressure: f32, cloud_seed: f32) -> f32 {
    let width = (max - min).abs().max(0.001);
    clamp01_f32(((pressure - min) / width) * 0.82 + cloud_seed * 0.18)
}

fn precipitation_state(
    pattern: &WeatherPatternDescriptor,
    intensity: f32,
) -> PrecipitationStateDto {
    PrecipitationStateDto {
        kind: pattern.precipitation_kind,
        intensity: intensity * pattern.precipitation_factor,
    }
}

fn drying_rate(kind: PrecipitationKind) -> f32 {
    match kind {
        PrecipitationKind::None => 0.035,
        PrecipitationKind::Dust => 0.045,
        PrecipitationKind::Rain | PrecipitationKind::Snow => 0.008,
    }
}

fn snow_melt_rate(kind: PrecipitationKind) -> f32 {
    match kind {
        PrecipitationKind::Snow => 0.002,
        _ => 0.0,
    }
}

fn phase_tags(phase: TimeOfDayPhase) -> &'static [&'static str] {
    match phase {
        TimeOfDayPhase::Night => &["state.night"],
        TimeOfDayPhase::Dawn => &["state.dawn"],
        TimeOfDayPhase::Day => &["state.day"],
        TimeOfDayPhase::Dusk => &["state.dusk"],
    }
}

fn cloud_tag(cloud_coverage: f32) -> &'static str {
    const TAGS: &[(f32, &str)] = &[
        (0.82, "cloud.overcast"),
        (0.45, "cloud.broken"),
        (0.0, "cloud.sparse"),
    ];
    TAGS.iter()
        .find(|(threshold, _)| cloud_coverage >= *threshold)
        .map(|(_, tag)| *tag)
        .unwrap_or("cloud.sparse")
}

fn visibility_tag(visibility_meters: f32) -> &'static str {
    const TAGS: &[(f32, &str)] = &[
        (650.0, "visibility.very_low"),
        (3500.0, "visibility.low"),
        (f32::MAX, "visibility.normal"),
    ];
    TAGS.iter()
        .find(|(max_distance, _)| visibility_meters < *max_distance)
        .map(|(_, tag)| *tag)
        .unwrap_or("visibility.normal")
}

fn transition_progress(tod: f32, pressure: f32) -> f32 {
    (0.72 + 0.28 * ((tod + pressure).fract())).clamp(0.0, 1.0)
}
