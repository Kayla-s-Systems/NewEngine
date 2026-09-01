#![forbid(unsafe_op_in_unsafe_fn)]

mod acoustics;
mod diagnostics;
mod environment;
mod music;
mod orchestration;
mod playback;
mod service;
mod streaming;
mod transport;
pub use acoustics::*;
pub use diagnostics::*;
pub use environment::*;
pub use music::*;
pub use orchestration::*;
pub use playback::*;
pub use service::*;
pub use streaming::*;
pub use transport::*;

use serde::{Deserialize, Serialize};

#[inline]
fn protocol_version_one() -> u32 {
    1
}

#[inline]
fn default_gain() -> f32 {
    1.0
}

#[inline]
fn default_speed() -> f32 {
    1.0
}

#[inline]
fn default_ear_distance() -> f32 {
    0.18
}

#[inline]
fn default_forward() -> [f32; 3] {
    [0.0, 0.0, -1.0]
}

#[inline]
fn default_up() -> [f32; 3] {
    [0.0, 1.0, 0.0]
}

#[inline]
fn sanitize_unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

#[inline]
pub fn sanitize_gain(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 4.0)
    } else {
        1.0
    }
}

#[inline]
pub fn sanitize_speed(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.05, 4.0)
    } else {
        1.0
    }
}

#[inline]
fn sanitize_vec3(value: [f32; 3]) -> [f32; 3] {
    value.map(|component| {
        if component.is_finite() {
            component
        } else {
            0.0
        }
    })
}

#[inline]
fn sanitize_range(
    value: [f32; 2],
    min_allowed: f32,
    max_allowed: f32,
    fallback: [f32; 2],
) -> [f32; 2] {
    if !value[0].is_finite() || !value[1].is_finite() {
        return fallback;
    }
    let a = value[0].clamp(min_allowed, max_allowed);
    let b = value[1].clamp(min_allowed, max_allowed);
    [a.min(b), a.max(b)]
}

#[inline]
fn finite_clamped(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[inline]
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
fn normalize_or(value: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
    let length_sq = value[0] * value[0] + value[1] * value[1] + value[2] * value[2];
    if !length_sq.is_finite() || length_sq <= 1.0e-10 {
        return fallback;
    }
    let inv = length_sq.sqrt().recip();
    [value[0] * inv, value[1] * inv, value[2] * inv]
}

#[cfg(test)]
mod tests;
