#![forbid(unsafe_op_in_unsafe_fn)]

use core::ops::{Mul, MulAssign};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{Quat, Vec3};

/// 3x3 float matrix in **column-major** form.
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

    #[inline]
    pub fn from_quat(q: Quat) -> Self {
        // Column-major rotation matrix.
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

        let m00 = 1.0 - (yy + zz);
        let m01 = xy + wz;
        let m02 = xz - wy;

        let m10 = xy - wz;
        let m11 = 1.0 - (xx + zz);
        let m12 = yz + wx;

        let m20 = xz + wy;
        let m21 = yz - wx;
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
        // (self) * (rhs)
        let a = self;
        let b = rhs;

        // Multiply columns: result_col = a * b_col
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
        // Column-major: v' = x*vx + y*vy + z*vz
        self.x_axis * rhs.x + self.y_axis * rhs.y + self.z_axis * rhs.z
    }
}
