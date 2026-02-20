// Copyright (c) 2026 NewEngine | Kayla's Systems. All rights reserved.
#![forbid(unsafe_op_in_unsafe_fn)]

use core::ops::{Mul, MulAssign};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{Quat, Vec3};

/// 3×3 matrix of `f32` stored in **column-major** form.
///
/// Layout:
/// - `x_axis` is column 0
/// - `y_axis` is column 1
/// - `z_axis` is column 2
///
/// This matches common GPU conventions and allows efficient multiplication by a vector:
/// `M * v = x_axis * v.x + y_axis * v.y + z_axis * v.z`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Mat3 {
    pub x_axis: Vec3,
    pub y_axis: Vec3,
    pub z_axis: Vec3,
}

impl Mat3 {
    pub const IDENTITY: Self = Self {
        x_axis: Vec3::new(1.0, 0.0, 0.0),
        y_axis: Vec3::new(0.0, 1.0, 0.0),
        z_axis: Vec3::new(0.0, 0.0, 1.0),
    };

    #[inline]
    pub const fn from_cols(x_axis: Vec3, y_axis: Vec3, z_axis: Vec3) -> Self {
        Self { x_axis, y_axis, z_axis }
    }

    /// Builds a **rotation** matrix from a quaternion.
    ///
    /// Notes:
    /// - For a *pure* rotation (orthonormal matrix), `q` should be normalized.
    /// - This implementation is intentionally consistent with `Quat * Vec3` rotation
    ///   (i.e. `q * v * q⁻¹`).
    ///
    /// The previous implementation had sign errors in the `w`-terms, which caused view-matrix
    /// construction (via quaternion-to-matrix) to diverge from quaternion-vector rotation.
    /// That divergence manifests as "non-linear" or "uneven" camera motion.
    #[inline]
    pub fn from_quat(q: Quat) -> Self {
        // The algebra is arranged to minimize multiplies.
        // This is the canonical form used by many math libraries.
        let x2 = q.x + q.x;
        let y2 = q.y + q.y;
        let z2 = q.z + q.z;

        let xx = q.x * x2;
        let yy = q.y * y2;
        let zz = q.z * z2;
        let xy = q.x * y2;
        let xz = q.x * z2;
        let yz = q.y * z2;
        let wx = q.w * x2;
        let wy = q.w * y2;
        let wz = q.w * z2;

        // Row-major elements (m00..m22), then packed into column vectors.
        let m00 = 1.0 - (yy + zz);
        let m01 = xy - wz;
        let m02 = xz + wy;

        let m10 = xy + wz;
        let m11 = 1.0 - (xx + zz);
        let m12 = yz - wx;

        let m20 = xz - wy;
        let m21 = yz + wx;
        let m22 = 1.0 - (xx + yy);

        Self::from_cols(
            Vec3::new(m00, m10, m20),
            Vec3::new(m01, m11, m21),
            Vec3::new(m02, m12, m22),
        )
    }
}

impl Mul for Mat3 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        let a = self;
        let b = rhs;

        let bx = b.x_axis;
        let by = b.y_axis;
        let bz = b.z_axis;

        Self::from_cols(a * bx, a * by, a * bz)
    }
}

impl MulAssign for Mat3 {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl Mul<Vec3> for Mat3 {
    type Output = Vec3;

    #[inline]
    fn mul(self, rhs: Vec3) -> Self::Output {
        self.x_axis * rhs.x + self.y_axis * rhs.y + self.z_axis * rhs.z
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[inline]
    fn xorshift32(state: &mut u32) -> u32 {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        *state = x;
        x
    }

    #[inline]
    fn f32_from_u32(x: u32) -> f32 {
        // Map to [-1, 1].
        let v = (x as f32) * (1.0 / (u32::MAX as f32));
        v * 2.0 - 1.0
    }

    #[test]
    fn mat3_from_quat_matches_quat_vec_rotation() {
        let mut rng = 0xC0FFEEu32;

        for _ in 0..4096 {
            let q = Quat::from_xyzw(
                f32_from_u32(xorshift32(&mut rng)),
                f32_from_u32(xorshift32(&mut rng)),
                f32_from_u32(xorshift32(&mut rng)),
                f32_from_u32(xorshift32(&mut rng)),
            )
                .normalize_or_identity();

            let v = Vec3::new(
                f32_from_u32(xorshift32(&mut rng)),
                f32_from_u32(xorshift32(&mut rng)),
                f32_from_u32(xorshift32(&mut rng)),
            );

            let a = q * v;
            let b = Mat3::from_quat(q) * v;

            let err = (a - b).length();
            assert!(
                err <= 1.0e-4,
                "Mat3::from_quat mismatch: err={} a={:?} b={:?} q={:?} v={:?}",
                err,
                a,
                b,
                q,
                v
            );
        }
    }

    #[test]
    fn mat3_from_quat_is_orthonormal_for_unit_quat() {
        let mut rng = 0xBADC0DEu32;

        for _ in 0..2048 {
            let q = Quat::from_xyzw(
                f32_from_u32(xorshift32(&mut rng)),
                f32_from_u32(xorshift32(&mut rng)),
                f32_from_u32(xorshift32(&mut rng)),
                f32_from_u32(xorshift32(&mut rng)),
            )
                .normalize_or_identity();

            let m = Mat3::from_quat(q);

            let x = m.x_axis;
            let y = m.y_axis;
            let z = m.z_axis;

            // Unit length.
            assert!((x.length() - 1.0).abs() <= 1.0e-4);
            assert!((y.length() - 1.0).abs() <= 1.0e-4);
            assert!((z.length() - 1.0).abs() <= 1.0e-4);

            // Orthogonality.
            assert!(x.dot(y).abs() <= 1.0e-4);
            assert!(x.dot(z).abs() <= 1.0e-4);
            assert!(y.dot(z).abs() <= 1.0e-4);

            // Right-handed basis (x × y = z).
            let c = x.cross(y);
            assert!((c - z).length() <= 1.0e-4);
        }
    }
}
