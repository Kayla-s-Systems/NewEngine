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

