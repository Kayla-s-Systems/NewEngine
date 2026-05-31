use crate::math::{mix_u64, vec_scale};
use crate::profile_catalog::{EnvironmentProfileDescriptor, PhenomenonTemplateDescriptor, WeatherPatternDescriptor};
use newengine_world_environment_api::{
    AabbDto, EnvironmentObjectDto, EnvironmentObjectId, EnvironmentObjectKind, TransformDto, Vec3Dto,
    WeatherStateDto, WindStateDto,
};
use newengine_world_api::WorldCellCoord;

pub(crate) fn build_environment_objects(
    req: &newengine_world_environment_api::EnvironmentFrameRequest,
    profile: &EnvironmentProfileDescriptor,
    pattern: &WeatherPatternDescriptor,
    activation: f32,
    cloud_coverage: f32,
    weather: &WeatherStateDto,
    fog_density: f32,
    wind: &WindStateDto,
) -> Vec<EnvironmentObjectDto> {
    let owning_cells = environment_object_cells(req);
    pattern
        .phenomena
        .iter()
        .filter(|template| activation >= template.activation_threshold)
        .map(|template| build_object(req, profile, template, &owning_cells, cloud_coverage, weather, fog_density, wind))
        .collect()
}

pub(crate) fn required_assets_for_object(object: &EnvironmentObjectDto, pattern: &WeatherPatternDescriptor, profile: &EnvironmentProfileDescriptor) -> Vec<String> {
    let mut assets = Vec::new();
    assets.push(profile.visual_assets.cloud_density_texture_ref.to_owned());
    assets.push(profile.visual_assets.cloud_detail_texture_ref.to_owned());
    assets.push(profile.visual_assets.cloud_dither_texture_ref.to_owned());
    assets.extend(pattern.required_assets.iter().map(|asset| (*asset).to_owned()));
    if let Some(template_id) = object.state_json.get("template_id").and_then(|value| value.as_str()) {
        if let Some(template) = pattern.phenomena.iter().find(|template| template.template_id == template_id) {
            assets.extend(template.required_assets.iter().map(|asset| (*asset).to_owned()));
        }
    }
    assets.sort();
    assets.dedup();
    assets
}

pub(crate) fn environment_object_cells(req: &newengine_world_environment_api::EnvironmentFrameRequest) -> Vec<WorldCellCoord> {
    if !req.resident_cells.is_empty() {
        return req.resident_cells.clone();
    }
    req.observer_cell.map(|cell| vec![cell]).unwrap_or_else(|| vec![WorldCellCoord::new(0, 0)])
}

fn build_object(
    req: &newengine_world_environment_api::EnvironmentFrameRequest,
    profile: &EnvironmentProfileDescriptor,
    template: &PhenomenonTemplateDescriptor,
    owning_cells: &[WorldCellCoord],
    cloud_coverage: f32,
    weather: &WeatherStateDto,
    fog_density: f32,
    wind: &WindStateDto,
) -> EnvironmentObjectDto {
    let center = offset(req.observer_position, template.offset_x, template.offset_y, template.offset_z);
    let density = cloud_density(template.kind, cloud_coverage, weather.intensity, fog_density);
    EnvironmentObjectDto {
        id: stable_environment_object_id(req.seed, req.time.game.day_index, object_salt(template.template_id)),
        kind: template.kind,
        bounds: regional_bounds(center, template.radius, template.y_min, template.y_max),
        owning_cells: owning_cells.to_vec(),
        transform: TransformDto { translation: center, ..TransformDto::default() },
        tags: object_tags(template, profile, weather),
        state_json: serde_json::json!({
            "template_id": template.template_id,
            "profile": profile.id,
            "priority": template.priority,
            "reason": template.reason,
            "weather": weather.weather_id,
            "density": density,
            "moisture": weather.precipitation.intensity.max(cloud_coverage * 0.35),
            "altitude_min": template.altitude_min,
            "altitude_max": template.altitude_max,
            "wind_velocity": vec_scale(wind.cloud_advection, object_wind_scale(template.kind)),
            "precipitation_potential": weather.precipitation.intensity,
            "shadow_strength": cloud_coverage * object_shadow_scale(template.kind),
            "light_absorption": cloud_coverage * object_absorption_scale(template.kind),
            "fog_density": fog_density,
        }),
    }
}

fn object_tags(template: &PhenomenonTemplateDescriptor, profile: &EnvironmentProfileDescriptor, weather: &WeatherStateDto) -> Vec<String> {
    let mut tags = template.tags.iter().map(|tag| (*tag).to_owned()).collect::<Vec<_>>();
    tags.push(profile.cloud_profile_ref.to_owned());
    tags.extend(weather.tags.iter().cloned());
    tags.sort();
    tags.dedup();
    tags
}

fn cloud_density(kind: EnvironmentObjectKind, coverage: f32, weather_intensity: f32, fog_density: f32) -> f32 {
    match kind {
        EnvironmentObjectKind::FogBank => (fog_density * 4.0).clamp(0.0, 1.0),
        EnvironmentObjectKind::StormCell => (0.35 + weather_intensity * 0.55).clamp(0.0, 1.0),
        EnvironmentObjectKind::SnowBand => (0.30 + weather_intensity * 0.42).clamp(0.0, 1.0),
        EnvironmentObjectKind::DustWall => (0.38 + weather_intensity * 0.52).clamp(0.0, 1.0),
        _ => (0.16 + coverage * 0.44).clamp(0.0, 1.0),
    }
}

fn object_wind_scale(kind: EnvironmentObjectKind) -> f32 {
    match kind {
        EnvironmentObjectKind::StormCell | EnvironmentObjectKind::DustWall => 1.45,
        EnvironmentObjectKind::SnowBand => 1.20,
        EnvironmentObjectKind::FogBank => 0.30,
        _ => 1.0,
    }
}

fn object_shadow_scale(kind: EnvironmentObjectKind) -> f32 {
    match kind {
        EnvironmentObjectKind::StormCell => 0.74,
        EnvironmentObjectKind::CloudField | EnvironmentObjectKind::CloudVolume => 0.55,
        _ => 0.22,
    }
}

fn object_absorption_scale(kind: EnvironmentObjectKind) -> f32 {
    match kind {
        EnvironmentObjectKind::StormCell => 0.48,
        EnvironmentObjectKind::CloudField | EnvironmentObjectKind::CloudVolume => 0.32,
        EnvironmentObjectKind::FogBank => 0.18,
        _ => 0.26,
    }
}

fn regional_bounds(center: Vec3Dto, radius: f32, y_min: f32, y_max: f32) -> AabbDto {
    AabbDto {
        min: Vec3Dto::new(center.x - radius, y_min, center.z - radius),
        max: Vec3Dto::new(center.x + radius, y_max, center.z + radius),
    }
}

fn offset(v: Vec3Dto, x: f32, y: f32, z: f32) -> Vec3Dto { Vec3Dto::new(v.x + x, v.y + y, v.z + z) }

fn stable_environment_object_id(seed: u64, day_index: u64, salt: u64) -> EnvironmentObjectId {
    EnvironmentObjectId { stable_id: mix_u64(seed ^ day_index.rotate_left(17) ^ salt) }
}

fn object_salt(id: &str) -> u64 {
    id.bytes().fold(0xE470_0000_0000_0001u64, |acc, byte| acc.rotate_left(5) ^ u64::from(byte))
}
