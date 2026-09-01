#[inline]
fn fade_multiplier(life_fraction: f32, fade_start_fraction: f32) -> f32 {
    if life_fraction <= fade_start_fraction {
        1.0
    } else {
        let span = (1.0 - fade_start_fraction).max(1.0e-5);
        (1.0 - (life_fraction - fade_start_fraction) / span).clamp(0.0, 1.0)
    }
}

#[inline]
fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[inline]
fn unit_float(value: u64) -> f32 {
    ((value >> 40) as u32 as f32) / ((1u32 << 24) as f32)
}

#[inline]
fn signed_unit_float(value: u64) -> f32 {
    unit_float(value) * 2.0 - 1.0
}

fn random_direction_in_cone(axis: Vec3, cone_radians: f32, seed: u64) -> Vec3 {
    let axis = axis.normalize_or_zero();
    if axis.length_squared() <= 1.0e-8 {
        return random_unit_vector(seed);
    }
    let cone = if cone_radians.is_finite() {
        cone_radians.clamp(0.0, core::f32::consts::PI)
    } else {
        core::f32::consts::FRAC_PI_2
    };
    let cos_min = cone.cos();
    let cos_theta = 1.0 - unit_float(mix64(seed ^ 0x1f83_d9ab_fb41_bd6b)) * (1.0 - cos_min);
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    let phi = core::f32::consts::TAU * unit_float(mix64(seed ^ 0x5be0_cd19_137e_2179));
    let helper = if axis.y.abs() < 0.95 {
        Vec3::Y
    } else {
        Vec3::X
    };
    let tangent = axis.cross(helper).normalize_or_zero();
    let bitangent = tangent.cross(axis).normalize_or_zero();
    (axis * cos_theta + tangent * (phi.cos() * sin_theta) + bitangent * (phi.sin() * sin_theta))
        .normalize_or_zero()
}

fn random_unit_vector(seed: u64) -> Vec3 {
    let x = unit_float(mix64(seed ^ 0x243f_6a88_85a3_08d3)) * 2.0 - 1.0;
    let y = unit_float(mix64(seed ^ 0x1319_8a2e_0370_7344)) * 2.0 - 1.0;
    let z = unit_float(mix64(seed ^ 0xa409_3822_299f_31d0)) * 2.0 - 1.0;
    Vec3::new(x, y, z).normalize_or_zero()
}

#[inline]
fn vec3_from_array(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}
