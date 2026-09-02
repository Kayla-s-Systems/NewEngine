use super::*;

pub(crate) fn sky_smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1.0e-5)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub(crate) fn sky_lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[inline]
pub(crate) fn solar_direction_from_cycle(
    time_hours: f32,
    latitude_degrees: f32,
    axial_tilt_degrees: f32,
    day_index: u64,
) -> Vec3 {
    // Solar declination is seasonal, not hourly. The previous implementation
    // varied declination during a single day and produced an asymmetric, visibly
    // accelerating solar arc. This uses a stable tropical-year approximation.
    let latitude = latitude_degrees.to_radians().clamp(-1.5533, 1.5533);
    let axial_tilt = axial_tilt_degrees
        .to_radians()
        .clamp(0.0, std::f32::consts::FRAC_PI_6);
    let season = TAU * ((day_index as f32 - 80.0) / 365.2422);
    let declination = axial_tilt * season.sin();
    let hour_angle = TAU * (time_hours / 24.0 - 0.5);

    let sin_altitude = (latitude.sin() * declination.sin()
        + latitude.cos() * declination.cos() * hour_angle.cos())
    .clamp(-1.0, 1.0);
    let east = declination.cos() * hour_angle.sin();
    let north =
        latitude.cos() * declination.sin() - latitude.sin() * declination.cos() * hour_angle.cos();
    Vec3::new(east, sin_altitude, -north).normalize_or_zero()
}

#[inline]
pub(crate) fn sky_mul3(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

#[inline]
pub(crate) fn sky_mul3_components(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

#[inline]
pub(crate) fn sky_clamp3(a: [f32; 3], lo: f32, hi: f32) -> [f32; 3] {
    [a[0].clamp(lo, hi), a[1].clamp(lo, hi), a[2].clamp(lo, hi)]
}

#[inline]
pub(crate) fn sky_color_to_rgba(a: [f32; 3]) -> [f32; 4] {
    [a[0], a[1], a[2], 1.0]
}

#[inline]
pub(crate) fn sky_safe_dir(v: Vec3, fallback: Vec3) -> Vec3 {
    if v.is_finite() && v.length_squared() > 1.0e-6 {
        v.normalize_or_zero()
    } else {
        fallback
    }
}
