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

pub(super) fn cloud_coverage_signal(
    seed: u64,
    day_index: u64,
    tod: f32,
    profile: &EnvironmentProfileDescriptor,
) -> f32 {
    // Two smoothly interpolated deterministic weather scales avoid both the old
    // once-per-day random jump and the broad sinusoid that kept mean coverage
    // near 50%. The signal describes variation *inside* the selected weather
    // regime; the pattern owns the physically meaningful coverage range.
    let synoptic = smooth_noise_over_days(seed, day_index, tod, 0xC10D_7101, 4);
    let mesoscale = smooth_noise_over_days(seed, day_index, tod, 0xC10D_7102, 1);
    let phase = unit_noise(seed, day_index / 2, 0xC10D_7103);
    let diurnal =
        0.5 + 0.5 * (std::f32::consts::TAU * (tod.rem_euclid(1.0) - 0.34 + phase * 0.18)).sin();

    // Keep profile identity as a very small morphology bias, centred around
    // zero. It must never become a hidden minimum cloud cover.
    let profile_hash = profile
        .cloud_profile_ref
        .bytes()
        .fold(0_u64, |acc, byte| acc.wrapping_mul(16777619) ^ byte as u64);
    let profile_bias = (unit_noise(profile_hash, 0, 0xC10D_7104) - 0.5) * 0.08;

    let raw = synoptic * 0.52 + mesoscale * 0.34 + diurnal * 0.14 + profile_bias;
    // A slight clear-sky skew gives high-pressure regimes real blue-sky
    // intervals while preserving dense values when a cloudy/overcast pattern
    // is selected.
    clamp01_f32(raw).powf(1.12)
}

fn smooth_noise_over_days(seed: u64, day_index: u64, tod: f32, salt: u64, period_days: u64) -> f32 {
    let period_days = period_days.max(1);
    let segment = day_index / period_days;
    let day_in_segment = day_index % period_days;
    let local = (day_in_segment as f32 + tod.clamp(0.0, 1.0)) / period_days as f32;
    let t = local.clamp(0.0, 1.0);
    let smooth_t = t * t * (3.0 - 2.0 * t);
    let a = unit_noise(seed, segment, salt);
    let b = unit_noise(seed, segment.saturating_add(1), salt);
    a + (b - a) * smooth_t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_catalog::profile_by_id;

    #[test]
    fn cloud_signal_is_continuous_across_day_boundary() {
        let (profile, _) = profile_by_id("environment.game_ready_forest_road");
        for seed in 0..32_u64 {
            let before = cloud_coverage_signal(seed, 171, 0.9999, profile);
            let after = cloud_coverage_signal(seed, 172, 0.0001, profile);
            assert!(
                (before - after).abs() < 0.015,
                "seed={seed} cloud signal jumped across midnight before={before} after={after}"
            );
        }
    }

    #[test]
    fn cloud_signal_has_real_variation_without_a_hidden_cloud_floor() {
        let (profile, _) = profile_by_id("environment.game_ready_forest_road");
        let mut min_value = 1.0_f32;
        let mut max_value = 0.0_f32;
        for seed in 0..128_u64 {
            for hour in [0.0_f32, 0.25, 0.50, 0.75] {
                let value = cloud_coverage_signal(seed, 171, hour, profile);
                min_value = min_value.min(value);
                max_value = max_value.max(value);
            }
        }
        assert!(
            min_value < 0.24,
            "cloud signal never reaches clear morphology min={min_value}"
        );
        assert!(
            max_value > 0.76,
            "cloud signal never reaches dense morphology max={max_value}"
        );
    }
}
