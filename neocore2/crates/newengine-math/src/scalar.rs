// Copyright (c) 2026 NewEngine | Kayla's Systems. All rights reserved.
#![forbid(unsafe_op_in_unsafe_fn)]

/// Small scalar helpers.
///
/// Kept private to preserve the public API surface.
///
/// Rationale:
/// - Stabilize normalization (avoid NaNs on zero/denormals).
/// - Keep hot paths branch-light and consistent across vector/quaternion types.

#[inline(always)]
pub(crate) fn inv_sqrt_checked(x: f32) -> f32 {
    // `sqrt` is well-defined for non-negative numbers. For anything else, fall back to 0.
    // This avoids propagating NaNs/Infs through camera/controller math.
    if x.is_finite() && x > 0.0 {
        1.0 / x.sqrt()
    } else {
        0.0
    }
}

#[inline(always)]
pub(crate) fn inv_checked(x: f32) -> f32 {
    if x.is_finite() && x != 0.0 {
        1.0 / x
    } else {
        0.0
    }
}
