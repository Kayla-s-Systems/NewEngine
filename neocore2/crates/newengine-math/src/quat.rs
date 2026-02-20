#![forbid(unsafe_op_in_unsafe_fn)]

use core::ops::{Mul, MulAssign};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{EulerRot, Mat3, Vec3};

/// Unit quaternion (x, y, z, w).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quat {
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    #[inline]
    pub const fn from_xyzw(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    #[inline]
    pub fn conjugate(self) -> Self {
        Self::from_xyzw(-self.x, -self.y, -self.z, self.w)
    }

    #[inline]
    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    #[inline]
    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    #[inline]
    pub fn normalize(self) -> Self {
        let inv = 1.0 / self.length();
        Self::from_xyzw(self.x * inv, self.y * inv, self.z * inv, self.w * inv)
    }

    #[inline]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite() && self.w.is_finite()
    }

    /// Normalizes the quaternion, returning identity if the input is not finite or too small.
    #[inline]
    pub fn normalize_or_identity(self) -> Self {
        if !self.is_finite() {
            return Self::IDENTITY;
        }
        let ls = self.length_squared();
        if !ls.is_finite() || ls < 1e-12 {
            return Self::IDENTITY;
        }
        let inv = 1.0 / ls.sqrt();
        Self::from_xyzw(self.x * inv, self.y * inv, self.z * inv, self.w * inv)
    }

    #[inline]
    pub fn from_axis_angle(axis: Vec3, angle: f32) -> Self {
        let half = 0.5 * angle;
        let (s, c) = half.sin_cos();
        let a = axis.normalize_or_zero();
        Self::from_xyzw(a.x * s, a.y * s, a.z * s, c)
    }

    #[inline]
    pub fn from_rotation_x(angle: f32) -> Self {
        let half = 0.5 * angle;
        let (s, c) = half.sin_cos();
        Self::from_xyzw(s, 0.0, 0.0, c)
    }

    #[inline]
    pub fn from_rotation_y(angle: f32) -> Self {
        let half = 0.5 * angle;
        let (s, c) = half.sin_cos();
        Self::from_xyzw(0.0, s, 0.0, c)
    }

    #[inline]
    pub fn from_rotation_z(angle: f32) -> Self {
        let half = 0.5 * angle;
        let (s, c) = half.sin_cos();
        Self::from_xyzw(0.0, 0.0, s, c)
    }

    /// Create from Euler angles.
    ///
    /// For `EulerRot::YXZ` inputs are (yaw_y, pitch_x, roll_z).
    #[inline]
    pub fn from_euler(order: EulerRot, a: f32, b: f32, c: f32) -> Self {
        match order {
            EulerRot::YXZ => {
                // q = qy * qx * qz
                (Self::from_rotation_y(a) * Self::from_rotation_x(b) * Self::from_rotation_z(c)).normalize()
            }
        }
    }

    #[inline]
    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z + self.w * rhs.w
    }

    #[inline]
    pub fn lerp(self, rhs: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            x: self.x + (rhs.x - self.x) * t,
            y: self.y + (rhs.y - self.y) * t,
            z: self.z + (rhs.z - self.z) * t,
            w: self.w + (rhs.w - self.w) * t,
        }
    }

    #[inline]
    pub fn nlerp(self, rhs: Self, t: f32) -> Self {
        self.lerp(rhs, t).normalize()
    }

    #[inline]
    pub fn slerp(self, mut rhs: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);

        let mut cos_theta = self.dot(rhs);

        if cos_theta < 0.0 {
            cos_theta = -cos_theta;
            rhs = Self {
                x: -rhs.x,
                y: -rhs.y,
                z: -rhs.z,
                w: -rhs.w,
            };
        }

        if cos_theta > 0.9995 {
            return self.nlerp(rhs, t);
        }

        let theta = cos_theta.acos();
        let sin_theta = theta.sin();

        if !sin_theta.is_finite() || sin_theta.abs() < 1.0e-8 {
            return self.nlerp(rhs, t);
        }

        let a = ((1.0 - t) * theta).sin() / sin_theta;
        let b = (t * theta).sin() / sin_theta;

        Self {
            x: self.x * a + rhs.x * b,
            y: self.y * a + rhs.y * b,
            z: self.z * a + rhs.z * b,
            w: self.w * a + rhs.w * b,
        }
    }

    /// Convert quaternion to Euler angles.
    ///
    /// Returns angles in radians.
    #[inline]
    pub fn to_euler(self, order: EulerRot) -> (f32, f32, f32) {
        match order {
            EulerRot::YXZ => {
                // Derived from rotation matrix for YXZ.
                // We convert to matrix elements and then extract.
                let m = Mat3::from_quat(self);

                // Pitch around X from m[1][2] with sign conventions matching our Mat3.
                // Using standard YXZ extraction (right-handed).
                //
                // m = Ry(yaw) * Rx(pitch) * Rz(roll)
                let sp = (-m.y_axis.z).clamp(-1.0, 1.0);
                let pitch = sp.asin();

                let cp = pitch.cos();
                if cp.abs() < 1e-6 {
                    // Gimbal lock.
                    let yaw = (-m.z_axis.x).atan2(m.x_axis.x);
                    let roll = 0.0;
                    (yaw, pitch, roll)
                } else {
                    let yaw = m.x_axis.z.atan2(m.z_axis.z);
                    let roll = m.y_axis.x.atan2(m.y_axis.y);
                    (yaw, pitch, roll)
                }
            }
        }
    }

    #[inline]
    pub fn from_mat3(m: &Mat3) -> Self {
        // Standard robust conversion from rotation matrix.
        let m00 = m.x_axis.x;
        let m11 = m.y_axis.y;
        let m22 = m.z_axis.z;
        let trace = m00 + m11 + m22;

        if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            let inv = 1.0 / s;
            Self::from_xyzw(
                (m.y_axis.z - m.z_axis.y) * inv,
                (m.z_axis.x - m.x_axis.z) * inv,
                (m.x_axis.y - m.y_axis.x) * inv,
                0.25 * s,
            )
        } else if m00 > m11 && m00 > m22 {
            let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
            let inv = 1.0 / s;
            Self::from_xyzw(
                0.25 * s,
                (m.x_axis.y + m.y_axis.x) * inv,
                (m.z_axis.x + m.x_axis.z) * inv,
                (m.y_axis.z - m.z_axis.y) * inv,
            )
        } else if m11 > m22 {
            let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
            let inv = 1.0 / s;
            Self::from_xyzw(
                (m.x_axis.y + m.y_axis.x) * inv,
                0.25 * s,
                (m.y_axis.z + m.z_axis.y) * inv,
                (m.z_axis.x - m.x_axis.z) * inv,
            )
        } else {
            let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
            let inv = 1.0 / s;
            Self::from_xyzw(
                (m.z_axis.x + m.x_axis.z) * inv,
                (m.y_axis.z + m.z_axis.y) * inv,
                0.25 * s,
                (m.x_axis.y - m.y_axis.x) * inv,
            )
        }
            .normalize()
    }

    #[inline]
    pub fn from_rotation_arc(from: Vec3, to: Vec3) -> Self {
        let f = from.normalize_or_zero();
        let t = to.normalize_or_zero();
        if f.length_squared() == 0.0 || t.length_squared() == 0.0 {
            return Self::IDENTITY;
        }

        let dot = f.dot(t);
        if dot >= 1.0 - 1e-6 {
            return Self::IDENTITY;
        }

        if dot <= -1.0 + 1e-6 {
            // 180°: choose any orthogonal axis.
            let axis = if f.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
            return Self::from_axis_angle(f.cross(axis).normalize_or_zero(), core::f32::consts::PI);
        }

        let axis = f.cross(t);
        Self::from_xyzw(axis.x, axis.y, axis.z, 1.0 + dot).normalize()
    }
}

impl Mul for Quat {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        // Hamilton product.
        Self::from_xyzw(
            self.w * rhs.x + self.x * rhs.w + self.y * rhs.z - self.z * rhs.y,
            self.w * rhs.y - self.x * rhs.z + self.y * rhs.w + self.z * rhs.x,
            self.w * rhs.z + self.x * rhs.y - self.y * rhs.x + self.z * rhs.w,
            self.w * rhs.w - self.x * rhs.x - self.y * rhs.y - self.z * rhs.z,
        )
    }
}

impl MulAssign for Quat {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl Mul<Vec3> for Quat {
    type Output = Vec3;
    #[inline]
    fn mul(self, rhs: Vec3) -> Self::Output {
        // Optimized quaternion-vector rotation.
        let qv = Vec3::new(self.x, self.y, self.z);
        let t = qv.cross(rhs) * 2.0;
        rhs + t * self.w + qv.cross(t)
    }
}
