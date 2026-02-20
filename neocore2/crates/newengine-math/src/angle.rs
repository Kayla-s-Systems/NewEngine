#![forbid(unsafe_op_in_unsafe_fn)]

/// Angle helpers (radians).
///
/// Keep these in `newengine-math` to avoid ad-hoc reimplementations across crates.
/// Precision matters for long-running editor sessions: if yaw is allowed to grow unbounded,
/// `sin/cos` will eventually lose precision and the camera will start rotating "unevenly".

/// Wraps angle to the `[-PI, PI]` range.
#[inline]
pub fn wrap_pi(mut radians: f32) -> f32 {
    if !radians.is_finite() {
        return 0.0;
    }

    // Using `rem_euclid` to keep monotonic behavior for negatives.
    const TAU: f32 = core::f32::consts::PI * 2.0;
    radians = (radians + core::f32::consts::PI).rem_euclid(TAU) - core::f32::consts::PI;

    // Avoid returning -PI which can cause tiny discontinuities when compared/serialized.
    if radians <= -core::f32::consts::PI {
        core::f32::consts::PI
    } else {
        radians
    }
}
