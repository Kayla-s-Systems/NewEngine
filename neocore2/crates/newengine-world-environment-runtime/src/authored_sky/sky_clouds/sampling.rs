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

