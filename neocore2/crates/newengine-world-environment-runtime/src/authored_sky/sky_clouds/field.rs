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

/// Low-frequency meteorological occupancy field. Cloud coverage changes are
/// admitted through this coherent front rather than by shifting the threshold
/// of every high-frequency cloud texel at once. That prevents a weather profile
/// change from materializing a complete cloud deck over the whole sky.
#[inline]
pub(super) fn sky_cloud_front_field(
    cloud_plane: Vec2,
    cloud_offset: Vec2,
    evolution_phase: f32,
    lifecycle: f32,
) -> f32 {
    let angle = evolution_phase.rem_euclid(1.0) * TAU;
    let rotated = sky_rotate2(cloud_plane, angle.cos() * 0.075);
    let coord = rotated * 0.018 + cloud_offset * 0.23 + Vec2::new(0.317, -0.193);
    let wave0 = ((coord.x * 0.83 + coord.y * 0.57) * TAU + angle * 0.19).sin();
    let wave1 = ((coord.x * -0.41 + coord.y * 1.11) * TAU - angle * 0.13 + 1.71).sin();
    (0.52 + wave0 * 0.29 + wave1 * 0.19 + (lifecycle - 0.5) * 0.04).clamp(0.0, 1.0)
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
    let cloud_plane = sky_cloud_plane(frame.to_sun);
    let macro_field = sky_macro_cloud_field(cloud_plane, cloud_offset, evolution_phase, lifecycle);
    let front_field = sky_cloud_front_field(cloud_plane, cloud_offset, evolution_phase, lifecycle);
    let evolution_sin = (evolution_phase * TAU).sin();
    let live_coverage =
        (coverage + (lifecycle - 0.5) * 0.10 + evolution_sin * 0.018).clamp(0.0, 1.0);
    let overcast = frame.cloud_overcast.clamp(0.0, 1.0);
    let softness = softness.clamp(0.04, 0.98);

    // Coverage controls the connected synoptic/mesoscale cloud mass. Fine cloud
    // morphology only changes modestly with coverage, so a provider transition
    // cannot suddenly turn every suitable FBM texel into a cloud.
    let front_threshold =
        (0.84 + (0.24 - 0.84) * live_coverage - overcast * 0.035).clamp(0.20, 0.88);
    let front_width = 0.075 + (0.160 - 0.075) * softness;
    let weather_mass = sky_smoothstep(
        front_threshold - front_width,
        front_threshold + front_width,
        front_field,
    );
    let threshold = (0.73 + (0.54 - 0.73) * live_coverage - overcast * 0.018).clamp(0.50, 0.76);
    let edge_width =
        (0.034 + (0.112 - 0.034) * softness) * (0.92 + (1.10 - 0.92) * lifecycle.clamp(0.0, 1.0));
    let dense_core = sky_smoothstep(
        threshold - edge_width * 0.72,
        threshold + edge_width * 0.92,
        macro_field,
    );
    let cloud_presence = sky_smoothstep(0.08, 0.22, live_coverage);
    let altitude_mask = sky_smoothstep(-0.025, 0.12, frame.to_sun.y);
    (dense_core * weather_mass * cloud_presence * altitude_mask).clamp(0.0, 1.0)
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

