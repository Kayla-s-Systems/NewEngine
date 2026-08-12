use super::*;

#[inline]
pub(super) fn sky_postfx_sanitize(mut value: EnvironmentPostFxState) -> EnvironmentPostFxState {
    value.exposure = value.exposure.clamp(0.45, 2.40);
    value.gamma = value.gamma.clamp(1.8, 2.6);
    value.black_lift = value.black_lift.clamp(0.0, 0.035);
    value.saturation = value.saturation.clamp(0.72, 1.28);
    value.contrast = value.contrast.clamp(0.82, 1.25);
    value.temperature = value.temperature.clamp(-0.22, 0.22);
    value.vignette_strength = value.vignette_strength.clamp(0.0, 0.22);
    value.local_contrast_strength = value.local_contrast_strength.clamp(0.0, 0.18);
    value.dither_strength = value.dither_strength.clamp(0.0, 1.5);
    value.bloom_threshold = value.bloom_threshold.clamp(0.45, 2.5);
    value.bloom_knee = value.bloom_knee.clamp(0.02, 0.8);
    value.bloom_intensity = value.bloom_intensity.clamp(0.0, 0.30);
    value.bloom_radius = value.bloom_radius.clamp(0.5, 2.0);
    value.sun_glare_scale = value.sun_glare_scale.clamp(0.0, 1.8);
    value.sun_ray_scale = value.sun_ray_scale.clamp(0.0, 1.8);
    value
}

pub(super) fn sky_postfx_from_environment(
    environment: &newengine_world_environment_api::EnvironmentFrameDto,
) -> EnvironmentPostFxState {
    let day = environment.time_of_day_state.day_blend.clamp(0.0, 1.0);
    let night = environment.time_of_day_state.night_blend.clamp(0.0, 1.0);
    let twilight = environment.sky.dusk_dawn_blend.clamp(0.0, 1.0);
    let overcast = environment.sky.overcast_blend.clamp(0.0, 1.0);
    let haze = environment.atmosphere.haze_amount.clamp(0.0, 1.0);
    let storm = environment.exposure_intent.storm_darkening.clamp(0.0, 1.0);
    let glare = environment.exposure_intent.sun_glare_hint.clamp(0.0, 1.0);
    let night_adaptation = environment
        .exposure_intent
        .night_adaptation_hint
        .clamp(0.0, 2.0);

    // Exposure is intentionally conservative. The sky/light bridge establishes
    // the physical range; tone mapping only adapts the display, never compensates
    // for missing illumination with a large arbitrary multiplier.
    let exposure = 1.04 + night * (0.22 + night_adaptation * 0.10) + overcast * 0.08 + haze * 0.035
        - storm * 0.10
        - glare * day * 0.035;
    let saturation = 1.055 - overcast * 0.085 - storm * 0.10 - haze * 0.035 + twilight * 0.035;
    let contrast = 1.025 + day * 0.018 - overcast * 0.045 - storm * 0.055 + twilight * 0.020;
    let temperature = twilight * 0.105 - night * 0.055 - overcast * 0.018;
    let black_lift = night * 0.0045 + overcast * 0.0025 + haze * 0.0015;
    let local_contrast = 0.058 + day * 0.030 - haze * 0.030 - overcast * 0.015;
    let vignette = 0.045 + night * 0.030 + storm * 0.018;
    let bloom_intensity = 0.045 + glare * 0.090 + twilight * 0.035;
    let bloom_threshold = 1.15 - glare * 0.30 - twilight * 0.12;

    sky_postfx_sanitize(EnvironmentPostFxState {
        exposure,
        gamma: 2.2,
        black_lift,
        saturation,
        contrast,
        temperature,
        vignette_strength: vignette,
        local_contrast_strength: local_contrast,
        dither_strength: 1.0 + night * 0.18,
        bloom_threshold,
        bloom_knee: 0.28 + haze * 0.10,
        bloom_intensity,
        bloom_radius: 0.92 + haze * 0.18,
        sun_glare_scale: (0.70 + glare * 0.85) * (1.0 - overcast * 0.55),
        sun_ray_scale: (0.62 + glare * 0.72) * (1.0 - overcast * 0.68),
    })
}

pub(super) fn sky_postfx_from_authored_frame(frame: &SkyFrameSample) -> EnvironmentPostFxState {
    let sun = frame.to_sun.y.clamp(-1.0, 1.0);
    let day = sky_smoothstep(-0.04, 0.16, sun);
    let night = 1.0 - sky_smoothstep(-0.20, 0.02, sun);
    let twilight = (1.0 - day) * (1.0 - night);
    sky_postfx_sanitize(EnvironmentPostFxState {
        exposure: 1.04 + night * 0.30 + frame.haze_amount * 0.04,
        saturation: 1.05 - frame.cloud_coverage * 0.05 + twilight * 0.03,
        contrast: 1.025 - frame.cloud_coverage * 0.025,
        temperature: twilight * 0.09 - night * 0.05,
        black_lift: night * 0.004,
        vignette_strength: 0.045 + night * 0.025,
        local_contrast_strength: 0.060 - frame.haze_amount * 0.025,
        bloom_threshold: 1.12 - twilight * 0.12,
        bloom_intensity: 0.045 + twilight * 0.035,
        sun_glare_scale: 0.85 + day * 0.25,
        sun_ray_scale: 0.72 + day * 0.22,
        ..EnvironmentPostFxState::default()
    })
}
