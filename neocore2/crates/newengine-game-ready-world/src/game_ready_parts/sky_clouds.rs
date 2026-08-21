use super::*;

// Procedural cloud coordinates are dimensionless. At the old 0.00012 scale a
// 4 m/s fair-weather wind needed minutes to move one broad macro lobe across
// the solar line of sight. 0.00075 maps ordinary 3-5 m/s advection to roughly
// 40-90 second cumulus crossing times while keeping the motion visually massive.
const SKY_CLOUD_ADVECTION_COORDS_PER_METER: f32 = 0.00075;

#[inline]
fn sky_cloud_seed_u64(mut value: u64) -> u64 {
    // SplitMix64 finalizer: deterministic, cheap and stable across platforms.
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[inline]
fn sky_cloud_seed_unit(seed: u64, lane: u64) -> f32 {
    let bits = sky_cloud_seed_u64(seed ^ lane.wrapping_mul(0xD1B5_4A32_D192_ED03));
    ((bits >> 40) as u32 as f32) * (1.0 / 16_777_216.0)
}

#[inline]
fn sky_cloud_seeded_offset(frame: &SkyFrameSample, wind: Vec2) -> Vec2 {
    let seed = frame.cloud_field_seed;
    // Spread starts over many procedural periods so separate environment seeds
    // do not all begin in the same macro lobe.
    let base_x = sky_cloud_seed_unit(seed, 0) as f64 * 64.0;
    let base_y = sky_cloud_seed_unit(seed, 1) as f64 * 64.0;
    let time = if frame.cloud_world_time_seconds.is_finite() {
        frame.cloud_world_time_seconds.max(0.0)
    } else {
        0.0
    };
    Vec2::new(
        (base_x + wind.x as f64 * time * SKY_CLOUD_ADVECTION_COORDS_PER_METER as f64)
            .rem_euclid(1024.0) as f32,
        (base_y + wind.y as f64 * time * SKY_CLOUD_ADVECTION_COORDS_PER_METER as f64)
            .rem_euclid(1024.0) as f32,
    )
}

#[inline]
fn sky_cloud_seeded_phase(seed: u64, lane: u64, world_time_seconds: f64, rate: f32) -> f32 {
    let base = sky_cloud_seed_unit(seed, lane) as f64;
    let time = if world_time_seconds.is_finite() {
        world_time_seconds.max(0.0)
    } else {
        0.0
    };
    (base + time * rate as f64).rem_euclid(1.0) as f32
}

#[inline]
fn sky_rotate2(value: Vec2, angle: f32) -> Vec2 {
    let (s, c) = angle.sin_cos();
    Vec2::new(c * value.x - s * value.y, s * value.x + c * value.y)
}

#[inline]
fn sky_cloud_plane(direction: Vec3) -> Vec2 {
    let direction = sky_safe_dir(direction, Vec3::new(0.0, 1.0, 0.0));
    let height = (direction.y + 0.16).max(0.075);
    let mut plane = Vec2::new(direction.x, direction.z) / height;
    plane /= 1.0 + plane.length() * 0.055;
    plane
}

#[inline]
pub(super) fn sky_macro_cloud_field(
    cloud_plane: Vec2,
    cloud_offset: Vec2,
    evolution_phase: f32,
    lifecycle: f32,
) -> f32 {
    let angle = evolution_phase.rem_euclid(1.0) * TAU;
    let rotated = sky_rotate2(cloud_plane, angle.sin() * 0.16);
    let coord = rotated * 0.055 + cloud_offset;
    let wave0 = ((coord.x * 1.73 + coord.y * 1.21) * TAU + angle * 1.03).sin();
    let wave1 = ((coord.x * -0.97 + coord.y * 2.37) * TAU - angle * 0.61 + 1.41).sin();
    let wave2 = ((coord.x * 3.17 + coord.y * -1.43) * TAU + angle * 0.29 + 2.17).sin();
    (0.50 + wave0 * 0.24 + wave1 * 0.16 + wave2 * 0.10 + (lifecycle - 0.5) * 0.08).clamp(0.0, 1.0)
}

#[inline]
pub(super) fn sky_cloud_sun_density(
    frame: &SkyFrameSample,
    coverage: f32,
    softness: f32,
    cloud_offset: Vec2,
    evolution_phase: f32,
    lifecycle: f32,
) -> f32 {
    if frame.to_sun.y <= -0.04 {
        return 0.0;
    }
    let macro_field = sky_macro_cloud_field(
        sky_cloud_plane(frame.to_sun),
        cloud_offset,
        evolution_phase,
        lifecycle,
    );
    let evolution_sin = (evolution_phase * TAU).sin();
    let live_coverage =
        (coverage + (lifecycle - 0.5) * 0.10 + evolution_sin * 0.018).clamp(0.0, 1.0);
    let overcast = frame.cloud_overcast.clamp(0.0, 1.0);

    // Match the dome shader's meteorological coverage curve, but remain
    // deliberately conservative: CPU has the macro field only, while the dome
    // owns the actual texture FBM/cirrus samples. Global cloud coverage must not
    // become fake line-of-sight occlusion of the solar disc.
    let threshold = (0.79 + (0.46 - 0.79) * live_coverage - overcast * 0.022).clamp(0.43, 0.82);
    let edge_width = (0.030 + (0.116 - 0.030) * softness.clamp(0.04, 0.98))
        * (0.92 + (1.10 - 0.92) * lifecycle.clamp(0.0, 1.0));
    let dense_core = sky_smoothstep(
        threshold - edge_width * 0.62,
        threshold + edge_width * 0.98,
        macro_field,
    );
    let cloud_presence = sky_smoothstep(0.10, 0.24, live_coverage);
    let altitude_mask = sky_smoothstep(-0.025, 0.12, frame.to_sun.y);
    (dense_core * cloud_presence * altitude_mask).clamp(0.0, 1.0)
}

#[inline]
pub(super) fn sky_cloud_occlusion_from_density(
    frame: &SkyFrameSample,
    raw_density: f32,
    smoothed_density: f32,
) -> CloudSunOcclusionRuntime {
    let sun_height = frame.to_sun.y.max(0.0);
    let air_mass = 1.0 / sun_height.max(0.20).powf(0.32);
    let absorption = frame.cloud_light_absorption.clamp(0.0, 1.0);
    let overcast = frame.cloud_overcast.clamp(0.0, 1.0);
    let optical_depth = smoothed_density * (1.30 + absorption * 2.80 + overcast * 1.35) * air_mass;
    let transmittance = (-optical_depth * 1.42).exp().clamp(0.025, 1.0);
    let shadow_strength = frame.cloud_shadow_strength.clamp(0.0, 1.0);
    CloudSunOcclusionRuntime {
        raw_density: raw_density.clamp(0.0, 1.0),
        smoothed_density: smoothed_density.clamp(0.0, 1.0),
        optical_depth: optical_depth.max(0.0),
        transmittance,
        direct_light_scale: transmittance,
        world_shadow_strength: (smoothed_density * shadow_strength).clamp(0.0, 1.0),
    }
}

#[inline]
pub(super) fn spatial_cloud_shadow_from_dynamics(
    frame: &SkyFrameSample,
    dynamics: &SkyDynamicsFrame,
) -> CloudShadowRenderState {
    let coverage = dynamics.coverage.clamp(0.0, 1.0);
    let overcast = frame.cloud_overcast.clamp(0.0, 1.0);
    let absorption = frame.cloud_light_absorption.clamp(0.0, 1.0);
    let sun_visible = sky_smoothstep(-0.02, 0.10, frame.to_sun.y);
    // Thin or nearly absent cloud cover may still be visible as high, optically
    // light haze, but it must not project opaque moving shapes onto the ground.
    // Fade local shadows in only after a coherent cumulus deck exists.
    let cloud_presence = sky_smoothstep(0.06, 0.24, coverage);
    let local_strength = (0.32 + dynamics.shadow_strength.clamp(0.0, 1.0) * 0.58 + overcast * 0.10)
        .clamp(0.0, 1.0)
        * sun_visible
        * cloud_presence;
    // This is the large-scale atmospheric attenuation shared by the whole map.
    // Local differences are evaluated per-fragment by the projected cloud field.
    let broad_optical_depth = overcast * 0.92 + absorption * 0.72 + coverage * 0.10;
    let broad_direct_scale = (-broad_optical_depth).exp().clamp(0.32, 1.0);
    let broad_ambient_scale = (1.0 - overcast * 0.15 - absorption * 0.10).clamp(0.68, 1.0);
    // Ground receivers use a deliberately low-frequency mask. The visible dome
    // carries the fine detail; repeating that detail in every world fragment
    // produces noisy, expensive and visually disconnected shadow freckles.
    let world_frequency = (0.0027 + coverage * 0.0009 + overcast * 0.0005).clamp(0.0024, 0.0044);
    let cloud_altitude = (1650.0 + overcast * 420.0 - absorption * 160.0).clamp(1100.0, 2400.0);
    let erosion_frequency = (world_frequency * (5.8 + coverage * 1.2)).clamp(0.016, 0.034);
    let erosion_strength = (0.055 + coverage * 0.055 + overcast * 0.030).clamp(0.05, 0.15);
    let erosion_fade_distance = (84.0 + coverage * 38.0 + overcast * 28.0).clamp(72.0, 150.0);
    CloudShadowRenderState {
        map0: [
            dynamics.cloud_offset.x,
            dynamics.cloud_offset.y,
            dynamics.evolution_phase,
            dynamics.lifecycle,
        ],
        map1: [
            world_frequency,
            cloud_altitude,
            coverage,
            dynamics.softness.clamp(0.04, 0.98),
        ],
        map2: [
            local_strength,
            absorption,
            broad_direct_scale,
            if sun_visible > 0.01 && cloud_presence > 0.01 && local_strength > 0.01 {
                1.0
            } else {
                0.0
            },
        ],
        map3: [
            dynamics.previous_cloud_offset.x,
            dynamics.previous_cloud_offset.y,
            dynamics.previous_evolution_phase,
            dynamics.previous_lifecycle,
        ],
        map4: [
            dynamics.temporal_history_weight,
            erosion_frequency,
            erosion_strength,
            erosion_fade_distance,
        ],
        broad_ambient_scale,
    }
}

#[inline]
fn spatial_cloud_shadow_field_cpu(
    projected: Vec2,
    state: [f32; 4],
    frequency: f32,
) -> (f32, f32, f32) {
    let evolution = state[2].rem_euclid(1.0);
    let lifecycle = state[3].clamp(0.0, 1.0);
    let angle = evolution * TAU;
    let coord =
        sky_rotate2(projected * frequency, angle.sin() * 0.16) + Vec2::new(state[0], state[1]);
    let wave0 = ((coord.x * 1.73 + coord.y * 1.21) * TAU + angle * 1.03).sin();
    let wave1 = ((coord.x * -0.97 + coord.y * 2.37) * TAU - angle * 0.61 + 1.41).sin();
    let wave2 = ((coord.x * 3.17 + coord.y * -1.43) * TAU + angle * 0.29 + 2.17).sin();
    let field = (0.50 + wave0 * 0.24 + wave1 * 0.16 + wave2 * 0.10 + (lifecycle - 0.5) * 0.08)
        .clamp(0.0, 1.0);
    (field, evolution, lifecycle)
}

#[inline]
fn spatial_cloud_shadow_erosion_cpu(
    projected: Vec2,
    state: [f32; 4],
    detail_frequency: f32,
) -> f32 {
    let angle = state[2].rem_euclid(1.0) * TAU;
    let offset = Vec2::new(state[0], state[1]);
    let coord = sky_rotate2(projected * detail_frequency, angle.cos() * 0.11) + offset * 4.71;
    let warp = Vec2::new(
        ((coord.x * 0.73 + coord.y * 1.17) * TAU + angle * 0.61).sin(),
        ((coord.x * -1.31 + coord.y * 0.47) * TAU - angle * 0.83).cos(),
    ) * 0.18;
    let p = coord + warp;
    let detail0 = ((p.x * 1.91 + p.y * 1.27) * TAU + angle * 1.37).sin();
    let detail1 = ((p.x * -2.83 + p.y * 2.19) * TAU - angle * 0.79).sin();
    let detail2 = ((p.x * 4.61 + p.y * -3.73) * TAU + angle * 0.43).sin();
    (detail0 * 0.55 + detail1 * 0.30 + detail2 * 0.15).clamp(-1.0, 1.0)
}

#[inline]
fn spatial_cloud_density_cpu(
    shadow: &CloudShadowRenderState,
    projected: Vec2,
    state: [f32; 4],
    world_pos: Vec3,
    camera_pos: Vec3,
    with_erosion: bool,
) -> f32 {
    let frequency = shadow.map1[0].clamp(0.0001, 0.05);
    let (mut field, evolution, lifecycle) =
        spatial_cloud_shadow_field_cpu(projected, state, frequency);
    let coverage = shadow.map1[2].clamp(0.0, 1.0);
    let softness = shadow.map1[3].clamp(0.04, 0.98);
    let live_coverage =
        (coverage + (lifecycle - 0.5) * 0.10 + (evolution * TAU).sin() * 0.018).clamp(0.0, 1.0);
    let threshold = 0.77 + (0.47 - 0.77) * live_coverage;
    let edge = (0.032 + (0.115 - 0.032) * softness) * (0.92 + (1.10 - 0.92) * lifecycle);

    if with_erosion {
        let fade_distance = shadow.map4[3].clamp(16.0, 512.0);
        let distance = (world_pos - camera_pos).length();
        let near_weight = 1.0 - sky_smoothstep(fade_distance * 0.22, fade_distance, distance);
        if near_weight > 0.0 {
            let detail = spatial_cloud_shadow_erosion_cpu(
                projected,
                state,
                shadow.map4[1].clamp(0.001, 0.25),
            );
            let edge_proximity =
                1.0 - sky_smoothstep(edge * 1.4, edge * 5.2, (field - threshold).abs());
            let erosion_strength = shadow.map4[2].clamp(0.0, 0.45);
            field = (field
                + detail * erosion_strength * near_weight * (0.30 + edge_proximity * 0.70))
                .clamp(0.0, 1.0);
        }
    }
    sky_smoothstep(threshold - edge, threshold + edge, field)
}

#[inline]
pub(super) fn sample_spatial_cloud_shadow_cpu_at(
    shadow: &CloudShadowRenderState,
    to_sun: Vec3,
    world_pos: Vec3,
    camera_pos: Vec3,
) -> f32 {
    if shadow.map2[3] < 0.5 {
        return 1.0;
    }
    let to_sun = sky_safe_dir(to_sun, Vec3::new(0.0, 1.0, 0.0));
    if to_sun.y <= 0.015 {
        return 1.0;
    }
    let cloud_altitude = shadow.map1[1].max(world_pos.y + 1.0);
    let ray_distance = ((cloud_altitude - world_pos.y) / to_sun.y.max(0.06)).max(0.0);
    let projected =
        Vec2::new(world_pos.x, world_pos.z) + Vec2::new(to_sun.x, to_sun.z) * ray_distance;

    let current_density =
        spatial_cloud_density_cpu(shadow, projected, shadow.map0, world_pos, camera_pos, true);
    let history_weight = shadow.map4[0].clamp(0.0, 0.92);
    let density = if history_weight > 0.0 {
        let previous_density =
            spatial_cloud_density_cpu(shadow, projected, shadow.map3, world_pos, camera_pos, false);
        let reactive = sky_smoothstep(0.055, 0.28, (current_density - previous_density).abs());
        let near_weight = 1.0
            - sky_smoothstep(
                shadow.map4[3] * 0.22,
                shadow.map4[3],
                (world_pos - camera_pos).length(),
            );
        let weight = history_weight * (1.0 - reactive) * (1.0 - near_weight * 0.48);
        let clamped_history =
            previous_density.clamp(current_density - 0.18, current_density + 0.18);
        current_density + (clamped_history - current_density) * weight
    } else {
        current_density
    };

    let absorption = shadow.map2[1].clamp(0.0, 1.0);
    let optical_depth = density * (1.10 + absorption * 2.70);
    let transmittance = (-optical_depth * 1.18).exp();
    let strength = shadow.map2[0].clamp(0.0, 1.0);
    let sun_height_fade = sky_smoothstep(0.015, 0.12, to_sun.y);
    (1.0 + (transmittance - 1.0) * strength * sun_height_fade).clamp(0.0, 1.0)
}

#[inline]
pub(super) fn sample_spatial_cloud_shadow_cpu(
    shadow: &CloudShadowRenderState,
    to_sun: Vec3,
    world_pos: Vec3,
) -> f32 {
    sample_spatial_cloud_shadow_cpu_at(shadow, to_sun, world_pos, Vec3::ZERO)
}

#[inline]
pub(super) fn spatial_cloud_shadow_probe(
    shadow: &CloudShadowRenderState,
    to_sun: Vec3,
) -> ([f32; 5], f32, f32) {
    let points = [
        Vec3::new(-56.0, 0.0, -32.0),
        Vec3::new(-28.0, 0.0, -14.0),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(28.0, 0.0, 14.0),
        Vec3::new(56.0, 0.0, 32.0),
    ];
    let mut samples = [1.0; 5];
    let mut min_value = 1.0f32;
    let mut max_value = 0.0f32;
    for (index, point) in points.into_iter().enumerate() {
        let value = sample_spatial_cloud_shadow_cpu(shadow, to_sun, point);
        samples[index] = value;
        min_value = min_value.min(value);
        max_value = max_value.max(value);
    }
    (samples, min_value, max_value)
}

#[inline]
fn sky_exp_alpha(dt: f32, response_seconds: f32) -> f32 {
    if !dt.is_finite() || dt <= 0.0 {
        return 0.0;
    }
    (1.0 - (-dt / response_seconds.max(0.001)).exp()).clamp(0.0, 1.0)
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

pub(super) fn update_sky_dynamics(
    world: &mut newengine_ecs::World,
    frame: &SkyFrameSample,
    dt: f32,
) -> SkyDynamicsFrame {
    if world.resource::<SkyDynamicsRuntime>().is_none() {
        world.insert_resource(SkyDynamicsRuntime::default());
    }
    let raw_dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
    let dt = raw_dt.clamp(0.0, 0.25);
    let dynamics = world
        .resource_mut::<SkyDynamicsRuntime>()
        .expect("SkyDynamicsRuntime inserted immediately above");

    let previous_cloud_offset = dynamics.cloud_offset;
    let previous_evolution_phase = dynamics.evolution_phase;
    let previous_lifecycle = sky_lifecycle_value(dynamics.lifecycle_phase);

    let target_wind = if frame.cloud_advection.is_finite() {
        frame.cloud_advection
    } else {
        Vec2::ZERO
    };
    let first_update = !dynamics.initialized;
    if first_update {
        dynamics.initialized = true;
        dynamics.smoothed_wind = target_wind;
        dynamics.smoothed_coverage = frame.cloud_coverage.clamp(0.0, 1.0);
        dynamics.smoothed_softness = frame.cloud_softness.clamp(0.04, 0.98);
        dynamics.smoothed_shadow = frame.cloud_shadow_strength.clamp(0.0, 1.0);
        dynamics.smoothed_haze = frame.haze_amount.clamp(0.0, 1.0);
        dynamics.cloud_offset = sky_cloud_seeded_offset(frame, target_wind);

        let initial_wind_speed = target_wind.length().clamp(0.0, 24.0);
        let initial_overcast = frame.cloud_overcast.clamp(0.0, 1.0);
        let initial_absorption = frame.cloud_light_absorption.clamp(0.0, 1.0);
        let evolution_rate = 0.0022 + initial_wind_speed * 0.00018 + initial_overcast * 0.0011;
        let lifecycle_rate = 0.00085 + initial_overcast * 0.00075 + initial_absorption * 0.00045;
        dynamics.evolution_phase = sky_cloud_seeded_phase(
            frame.cloud_field_seed,
            2,
            frame.cloud_world_time_seconds,
            evolution_rate,
        );
        dynamics.lifecycle_phase = sky_cloud_seeded_phase(
            frame.cloud_field_seed,
            3,
            frame.cloud_world_time_seconds,
            lifecycle_rate,
        );
        dynamics.gust_phase = sky_cloud_seed_unit(frame.cloud_field_seed, 4) * TAU;
    }

    let wind_alpha = sky_exp_alpha(dt, 7.5);
    let weather_alpha = sky_exp_alpha(dt, 24.0);
    let optical_alpha = sky_exp_alpha(dt, 12.0);
    dynamics.smoothed_wind += (target_wind - dynamics.smoothed_wind) * wind_alpha;
    dynamics.smoothed_coverage +=
        (frame.cloud_coverage.clamp(0.0, 1.0) - dynamics.smoothed_coverage) * weather_alpha;
    dynamics.smoothed_softness +=
        (frame.cloud_softness.clamp(0.04, 0.98) - dynamics.smoothed_softness) * weather_alpha;
    dynamics.smoothed_shadow +=
        (frame.cloud_shadow_strength.clamp(0.0, 1.0) - dynamics.smoothed_shadow) * optical_alpha;
    dynamics.smoothed_haze +=
        (frame.haze_amount.clamp(0.0, 1.0) - dynamics.smoothed_haze) * optical_alpha;

    let wind_speed = dynamics.smoothed_wind.length().clamp(0.0, 24.0);
    let gust_strength = frame.cloud_gust_strength.clamp(0.0, 1.0);
    dynamics.gust_phase = (dynamics.gust_phase
        + dt * (0.18 + wind_speed * 0.035 + gust_strength * 0.22))
        .rem_euclid(TAU);
    let gust_wave = (dynamics.gust_phase.sin() * 0.56
        + (dynamics.gust_phase * 2.17 + 0.73).sin() * 0.29
        + (dynamics.gust_phase * 4.03 + 2.10).sin() * 0.15)
        .clamp(-1.0, 1.0);
    let gust_factor = (1.0 + gust_strength * gust_wave * 0.42).clamp(0.55, 1.55);

    // Integrate wind velocity rather than multiplying the current wind by total
    // elapsed time. This prevents visible cloud teleporting when the weather
    // provider changes direction or speed.
    dynamics.cloud_offset +=
        dynamics.smoothed_wind * (dt * SKY_CLOUD_ADVECTION_COORDS_PER_METER * gust_factor);
    // Keep the phase bounded without the visible 0..1 discontinuity that
    // appears when non-integer octave coefficients are used by the cloud field.
    dynamics.cloud_offset.x = dynamics.cloud_offset.x.rem_euclid(1024.0);
    dynamics.cloud_offset.y = dynamics.cloud_offset.y.rem_euclid(1024.0);

    let overcast = frame.cloud_overcast.clamp(0.0, 1.0);
    let absorption = frame.cloud_light_absorption.clamp(0.0, 1.0);
    dynamics.evolution_phase = (dynamics.evolution_phase
        + dt * (0.0022 + wind_speed * 0.00018 + overcast * 0.0011))
        .rem_euclid(1.0);
    dynamics.lifecycle_phase = (dynamics.lifecycle_phase
        + dt * (0.00085 + overcast * 0.00075 + absorption * 0.00045))
        .rem_euclid(1.0);
    let lifecycle = sky_lifecycle_value(dynamics.lifecycle_phase);

    let coverage = (dynamics.smoothed_coverage + (lifecycle - 0.5) * 0.085).clamp(0.0, 1.0);
    let softness = (dynamics.smoothed_softness + (gust_factor - 1.0) * 0.035).clamp(0.04, 0.98);
    let raw_sun_occlusion = sky_cloud_sun_density(
        frame,
        coverage,
        softness,
        dynamics.cloud_offset,
        dynamics.evolution_phase,
        lifecycle,
    );
    if first_update {
        // The first rendered frame must represent the actual weather state. A
        // fade from clear sky to the existing cloud cover creates a physically
        // incorrect launch flash and temporarily desynchronizes sky and world light.
        dynamics.smoothed_sun_occlusion = raw_sun_occlusion;
    } else {
        let occlusion_response = if raw_sun_occlusion > dynamics.smoothed_sun_occlusion {
            0.48
        } else {
            1.15
        };
        let occlusion_alpha = sky_exp_alpha(dt, occlusion_response);
        dynamics.smoothed_sun_occlusion +=
            (raw_sun_occlusion - dynamics.smoothed_sun_occlusion) * occlusion_alpha;
    }
    let sun_occlusion =
        sky_cloud_occlusion_from_density(frame, raw_sun_occlusion, dynamics.smoothed_sun_occlusion);
    let offset_delta = (dynamics.cloud_offset - previous_cloud_offset).length();
    let evolution_delta = sky_phase_distance(dynamics.evolution_phase, previous_evolution_phase);
    let lifecycle_delta = (lifecycle - previous_lifecycle).abs();
    let temporal_history_weight = sky_temporal_history_weight(
        first_update,
        raw_dt,
        offset_delta,
        evolution_delta,
        lifecycle_delta,
    );
    let previous_cloud_offset = if first_update {
        dynamics.cloud_offset
    } else {
        previous_cloud_offset
    };
    let previous_evolution_phase = if first_update {
        dynamics.evolution_phase
    } else {
        previous_evolution_phase
    };
    let previous_lifecycle = if first_update {
        lifecycle
    } else {
        previous_lifecycle
    };

    SkyDynamicsFrame {
        cloud_offset: dynamics.cloud_offset,
        coverage,
        softness,
        shadow_strength: dynamics.smoothed_shadow,
        haze: dynamics.smoothed_haze,
        evolution_phase: dynamics.evolution_phase,
        lifecycle,
        gust_factor,
        previous_cloud_offset,
        previous_evolution_phase,
        previous_lifecycle,
        temporal_history_weight,
        sun_occlusion,
    }
}
