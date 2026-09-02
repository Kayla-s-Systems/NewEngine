#[inline]
fn sky_exp_alpha(dt: f32, response_seconds: f32) -> f32 {
    if !dt.is_finite() || dt <= 0.0 {
        return 0.0;
    }
    (1.0 - (-dt / response_seconds.max(0.001)).exp()).clamp(0.0, 1.0)
}

/// Exponential relaxation with a physical slew-rate ceiling. The exponential
/// term removes frame-rate dependence; the slew limit prevents a large target
/// discontinuity from becoming an implausibly fast weather front.
#[inline]
fn sky_rate_limited_exp_step(
    current: f32,
    target: f32,
    dt: f32,
    response_seconds: f32,
    max_rate_per_second: f32,
) -> f32 {
    if !dt.is_finite() || dt <= 0.0 {
        return current;
    }
    let alpha = sky_exp_alpha(dt, response_seconds);
    let requested_delta = (target - current) * alpha;
    let max_delta = max_rate_per_second.max(0.0) * dt;
    current + requested_delta.clamp(-max_delta, max_delta)
}

#[inline]
fn sky_lifecycle_value(phase: f32) -> f32 {
    let angle = phase.rem_euclid(1.0) * TAU;
    (0.5 + 0.31 * angle.sin()
        + 0.13 * (angle * 2.0 + 1.27).sin()
        + 0.06 * (angle * 3.0 + 2.41).sin())
    .clamp(0.0, 1.0)
}

#[inline]
fn sky_phase_distance(a: f32, b: f32) -> f32 {
    let delta = (a - b).abs().rem_euclid(1.0);
    delta.min(1.0 - delta)
}

#[inline]
fn sky_temporal_history_weight(
    first_update: bool,
    raw_dt: f32,
    offset_delta: f32,
    evolution_delta: f32,
    lifecycle_delta: f32,
) -> f32 {
    if first_update
        || !raw_dt.is_finite()
        || raw_dt <= 0.0
        || raw_dt > 0.12
        || offset_delta > 0.10
        || evolution_delta > 0.045
        || lifecycle_delta > 0.18
    {
        return 0.0;
    }
    (-raw_dt / 0.19).exp().clamp(0.28, 0.88)
}
