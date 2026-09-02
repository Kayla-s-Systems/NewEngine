use crate::profile_catalog::{
    presentation_by_id, presentation_table_by_id, EnvironmentProfileDescriptor,
    WeatherPresentationDescriptor,
};
use newengine_world_environment_api::{
    AtmosphereStateDto, PrecipitationKind, PrecipitationStateDto, SnowStateDto, ThunderStateDto,
    TimeOfDayPhase, WeatherKind, WeatherStateDto, WetnessStateDto,
};

pub(super) struct ObservedWeather {
    pub pattern: &'static WeatherPresentationDescriptor,
    pub weather: WeatherStateDto,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn observe(
    profile: &EnvironmentProfileDescriptor,
    atmosphere: &AtmosphereStateDto,
    cloud_coverage: f32,
    overcast: f32,
    precipitation_kind: PrecipitationKind,
    precipitation_rate_mm_h: f32,
    precipitation_intensity: f32,
    thunder_probability: f32,
) -> ObservedWeather {
    let kind = classify(
        profile,
        atmosphere,
        cloud_coverage,
        overcast,
        precipitation_kind,
        precipitation_rate_mm_h,
        thunder_probability,
    );
    let pattern = pattern_for_observation(profile, kind);
    let intensity = match kind {
        WeatherKind::Storm => thunder_probability
            .max(precipitation_intensity)
            .max((atmosphere.cape_j_per_kg / 2200.0).clamp(0.0, 1.0)),
        WeatherKind::Rain | WeatherKind::Snow => precipitation_intensity,
        WeatherKind::Fog => atmosphere.fog_density,
        WeatherKind::Overcast => overcast,
        WeatherKind::Cloudy => cloud_coverage,
        WeatherKind::DustStorm | WeatherKind::HeatHaze => atmosphere.haze_amount,
        WeatherKind::Clear => (1.0 - cloud_coverage).clamp(0.0, 1.0) * 0.15,
    }
    .clamp(0.0, 1.0);

    ObservedWeather {
        pattern,
        weather: WeatherStateDto {
            weather_id: pattern.id.to_owned(),
            state: kind,
            intensity,
            transition_progress: 1.0,
            precipitation: PrecipitationStateDto {
                kind: precipitation_kind,
                intensity: precipitation_intensity,
                rate_mm_per_hour: precipitation_rate_mm_h,
            },
            thunder: ThunderStateDto {
                probability: thunder_probability,
                // Until mesoscale cells provide a lightning origin, this is only an
                // observer-range hint derived from electrically active storm severity.
                distance_meters: if thunder_probability > 0.01 {
                    800.0 + (1.0 - thunder_probability) * 5200.0
                } else {
                    0.0
                },
            },
            wetness: WetnessStateDto::default(),
            snow: SnowStateDto::default(),
            tags: Vec::new(),
        },
    }
}

fn classify(
    profile: &EnvironmentProfileDescriptor,
    atmosphere: &AtmosphereStateDto,
    cloud_coverage: f32,
    overcast: f32,
    precipitation_kind: PrecipitationKind,
    precipitation_rate_mm_h: f32,
    thunder_probability: f32,
) -> WeatherKind {
    let climate = crate::profile_catalog::atmosphere_profile_by_id(profile.atmosphere_profile_ref);
    let dry_erodible_surface = climate.surface_moisture_availability < 0.12;
    if dry_erodible_surface && atmosphere.aerosol_density > 0.42 && atmosphere.haze_amount > 0.45 {
        return WeatherKind::DustStorm;
    }
    if thunder_probability > 0.14 && precipitation_rate_mm_h > 0.25 {
        return WeatherKind::Storm;
    }
    if precipitation_rate_mm_h > 0.05 {
        return if matches!(precipitation_kind, PrecipitationKind::Snow) {
            WeatherKind::Snow
        } else {
            WeatherKind::Rain
        };
    }
    if atmosphere.fog_density > 0.22 {
        WeatherKind::Fog
    } else if overcast > 0.64 {
        WeatherKind::Overcast
    } else if cloud_coverage > 0.24 {
        WeatherKind::Cloudy
    } else if atmosphere.temperature_celsius > 32.0 && atmosphere.haze_amount > 0.22 {
        WeatherKind::HeatHaze
    } else {
        WeatherKind::Clear
    }
}

pub(super) fn pattern_for_observation(
    profile: &EnvironmentProfileDescriptor,
    kind: WeatherKind,
) -> &'static WeatherPresentationDescriptor {
    let table = presentation_table_by_id(profile.weather_table_ref);
    if let Some(pattern) = table
        .bands
        .iter()
        .map(|band| presentation_by_id(band.pattern_id))
        .find(|pattern| pattern.kind == kind)
    {
        pattern
    } else {
        presentation_by_id(table.fallback_pattern_id)
    }
}


pub(super) fn enrich_tags(
    weather: &mut WeatherStateDto,
    phase: TimeOfDayPhase,
    visibility_meters: f32,
    cloud_coverage: f32,
) {
    let mut tags = Vec::new();
    tags.push(weather_tag(weather.state).to_owned());
    tags.push(
        match phase {
            TimeOfDayPhase::Night => "state.night",
            TimeOfDayPhase::Dawn => "state.dawn",
            TimeOfDayPhase::Day => "state.day",
            TimeOfDayPhase::Dusk => "state.dusk",
        }
        .to_owned(),
    );
    tags.push(
        if cloud_coverage >= 0.82 {
            "cloud.overcast"
        } else if cloud_coverage >= 0.45 {
            "cloud.broken"
        } else {
            "cloud.sparse"
        }
        .to_owned(),
    );
    tags.push(
        if visibility_meters < 650.0 {
            "visibility.very_low"
        } else if visibility_meters < 3500.0 {
            "visibility.low"
        } else {
            "visibility.normal"
        }
        .to_owned(),
    );
    if weather.precipitation.rate_mm_per_hour > 0.05 {
        match weather.precipitation.kind {
            PrecipitationKind::Rain => {
                tags.push("surface.wet".to_owned());
                tags.push(if weather.precipitation.rate_mm_per_hour > 8.0 {
                    "audio.rain_heavy".to_owned()
                } else {
                    "audio.rain".to_owned()
                });
            }
            PrecipitationKind::Snow => tags.push("surface.snow".to_owned()),
            PrecipitationKind::None | PrecipitationKind::Dust => {}
        }
    }
    if weather.thunder.probability > 0.14 {
        tags.push("ai.shelter_preferred".to_owned());
    }
    tags.sort();
    tags.dedup();
    weather.tags = tags;
}

fn weather_tag(kind: WeatherKind) -> &'static str {
    match kind {
        WeatherKind::Clear => "weather.clear",
        WeatherKind::Cloudy => "weather.cloudy",
        WeatherKind::Overcast => "weather.overcast",
        WeatherKind::Rain => "weather.rain",
        WeatherKind::Storm => "weather.storm",
        WeatherKind::Snow => "weather.snow",
        WeatherKind::Fog => "weather.fog",
        WeatherKind::DustStorm => "weather.dust_storm",
        WeatherKind::HeatHaze => "weather.heat_haze",
    }
}
