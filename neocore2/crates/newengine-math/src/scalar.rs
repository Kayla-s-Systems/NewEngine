// Copyright (c) 2026 NewEngine | Take Some(). All rights reserved.
#![forbid(unsafe_op_in_unsafe_fn)]

//! Small scalar helpers.
//!
//! Kept private to preserve the public API surface.
//!
//! Rationale:
//! - Stabilize normalization (avoid NaNs on zero/denormals).
//! - Keep hot paths branch-light and consistent across vector/quaternion types.

#[inline]
pub(crate) fn inv_sqrt_checked(x: f32) -> f32 {
    // Avoid NaN/Inf and denormals propagation
    if x.is_finite() && x > 0.0 {
        let r = x.sqrt();
        if r.is_finite() && r > 0.0 {
            1.0 / r
        } else {
            0.0
        }
    } else {
        0.0
    }
}

#[inline]
#[allow(dead_code)]
pub(crate) fn inv_checked(x: f32) -> f32 {
    if x.is_finite() && x.abs() > 0.0 {
        1.0 / x
    } else {
        0.0
    }
}
