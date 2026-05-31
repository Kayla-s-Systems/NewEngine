use newengine_world_environment_api::{Color3Dto, Vec3Dto};

#[inline]
pub(crate) fn clamp01(value: f64) -> f64 { value.clamp(0.0, 1.0) }

#[inline]
pub(crate) fn clamp01_f32(value: f32) -> f32 { value.clamp(0.0, 1.0) }

#[inline]
pub(crate) fn normalize(v: Vec3Dto) -> Vec3Dto {
    let len_sq = v.x * v.x + v.y * v.y + v.z * v.z;
    if len_sq <= f32::EPSILON {
        return Vec3Dto::zero();
    }
    let inv = len_sq.sqrt().recip();
    Vec3Dto::new(v.x * inv, v.y * inv, v.z * inv)
}

#[inline]
pub(crate) fn mix_color(a: Color3Dto, b: Color3Dto, t: f32) -> Color3Dto {
    let t = clamp01_f32(t);
    Color3Dto::new(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t)
}

#[inline]
pub(crate) fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let denom = (edge1 - edge0).max(f32::EPSILON);
    let t = clamp01_f32((x - edge0) / denom);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub(crate) fn circular_window(tod: f32, center: f32, half_width: f32) -> f32 {
    let mut d = (tod - center).abs();
    if d > 0.5 {
        d = 1.0 - d;
    }
    1.0 - smoothstep(0.0, half_width.max(0.001), d)
}

#[inline]
pub(crate) fn mix_u64(mut v: u64) -> u64 {
    v ^= v >> 33;
    v = v.wrapping_mul(0xff51afd7ed558ccd);
    v ^= v >> 33;
    v = v.wrapping_mul(0xc4ceb9fe1a85ec53);
    v ^ (v >> 33)
}

#[inline]
pub(crate) fn unit_noise(seed: u64, day_index: u64, salt: u64) -> f32 {
    let v = mix_u64(seed ^ day_index.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt);
    ((v >> 40) as u32) as f32 / (0x00FF_FFFFu32 as f32)
}

#[inline]
pub(crate) fn range_lerp(min: f32, max: f32, t: f32) -> f32 { min + (max - min) * clamp01_f32(t) }

#[inline]
pub(crate) fn vec_scale(v: Vec3Dto, s: f32) -> Vec3Dto { Vec3Dto::new(v.x * s, v.y * s, v.z * s) }
