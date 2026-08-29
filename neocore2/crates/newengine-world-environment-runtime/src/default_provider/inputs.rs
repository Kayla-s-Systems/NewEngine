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
