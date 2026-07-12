use crate::{
    math::{clamp01_f32, unit_noise},
    profile_catalog::EnvironmentProfileDescriptor,
};
use newengine_world_environment_api::EnvironmentFrameRequest;

pub(crate) fn deterministic_key(req: &EnvironmentFrameRequest, provider: &str) -> String {
    deterministic_key_for_day(req, provider, normalized_day_from_time(req))
}

pub(super) fn deterministic_key_for_day(
    req: &EnvironmentFrameRequest,
    provider: &str,
    normalized_day: f32,
) -> String {
    format!(
        "{}:{}:{}:{:.6}:{}",
        provider,
        req.world_instance_id,
        req.seed,
        normalized_day,
        req.environment_profile.profile_id
    )
}

pub(super) fn normalized_day_from_time(req: &EnvironmentFrameRequest) -> f32 {
    let normalized = req.time.game.normalized_day;
    if normalized.is_finite() && (0.0..=1.0).contains(&normalized) {
        return normalized as f32;
    }
    let seconds_per_day = req.time.game.seconds_per_game_day.max(1.0);
    let seconds = req.time.game.seconds_of_day.rem_euclid(seconds_per_day);
    (seconds / seconds_per_day) as f32
}

pub(super) fn normalized_profile_id(req: &EnvironmentFrameRequest) -> &str {
    let trimmed = req.environment_profile.profile_id.trim();
    if trimmed.is_empty() {
        "environment.default"
    } else {
        trimmed
    }
}

pub(super) fn profile_warning(found: bool, requested: &str) -> Vec<String> {
    if found {
        Vec::new()
    } else {
        vec![format!(
            "unknown environment profile '{}' routed to descriptor 'environment.default'",
            requested
        )]
    }
}

pub(super) fn weather_pressure(seed: u64, day_index: u64, tod: f32) -> f32 {
    let base = unit_noise(seed, day_index, 0xAE17_0001);
    let front = ((std::f32::consts::TAU * (tod * 1.7 + base)).sin() + 1.0) * 0.5;
    let slow = unit_noise(seed, day_index / 2, 0xAE17_0002);
    (front * 0.55 + slow * 0.30 + base * 0.15).clamp(0.0, 1.0)
}

pub(super) fn baseline_cloud_coverage(
    seed: u64,
    day_index: u64,
    tod: f32,
    profile: &EnvironmentProfileDescriptor,
) -> f32 {
    let seed_phase = unit_noise(seed, day_index, 0xC10D_7001);
    let daily_wave = ((std::f32::consts::TAU * (tod + seed_phase)).sin() + 1.0) * 0.5;
    let slow_front = unit_noise(seed, day_index / 3, 0xC10D_7002);
    let profile_bias = profile
        .cloud_profile_ref
        .bytes()
        .fold(0.0f32, |acc, byte| acc + byte as f32 * 0.00001)
        .fract()
        * 0.08;
    clamp01_f32(0.08 + daily_wave * 0.34 + slow_front * 0.40 + profile_bias)
}
