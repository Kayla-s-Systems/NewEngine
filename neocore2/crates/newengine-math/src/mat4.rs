// Copyright (c) 2026 NewEngine | Take Some(). All rights reserved.
#![forbid(unsafe_op_in_unsafe_fn)]

use core::ops::{Mul, MulAssign};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{Mat3, Quat, Vec3, Vec4};

/// 4x4 float matrix in **column-major** form.
///
/// Naming follows the historical `glam::Mat4` layout (`x_axis..w_axis`).
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Mat4 {
    pub x_axis: Vec4,
    pub y_axis: Vec4,
    pub z_axis: Vec4,
    pub w_axis: Vec4,
}

impl Mat4 {
    pub const IDENTITY: Self = Self {
        x_axis: Vec4::new(1.0, 0.0, 0.0, 0.0),
        y_axis: Vec4::new(0.0, 1.0, 0.0, 0.0),
        z_axis: Vec4::new(0.0, 0.0, 1.0, 0.0),
        w_axis: Vec4::new(0.0, 0.0, 0.0, 1.0),
    };

    #[inline]
    pub const fn from_cols(x_axis: Vec4, y_axis: Vec4, z_axis: Vec4, w_axis: Vec4) -> Self {
        Self {
            x_axis,
            y_axis,
            z_axis,
            w_axis,
        }
    }

    #[inline]
    pub fn transform_point3(self, p: Vec3) -> Vec3 {
        let v = self * Vec4::new(p.x, p.y, p.z, 1.0);
        let w = v.w;

        if w.is_finite() && w.abs() > 1.0e-8 {
            let inv_w = 1.0 / w;
            Vec3::new(v.x * inv_w, v.y * inv_w, v.z * inv_w)
        } else {
            Vec3::new(v.x, v.y, v.z)
        }
    }

    /// Transforms a direction (vector) by this matrix (w=0).
    #[inline]
    pub fn transform_vector3(self, v: Vec3) -> Vec3 {
        let r = self * Vec4::new(v.x, v.y, v.z, 0.0);
        Vec3::new(r.x, r.y, r.z)
    }

    /// Decomposes an affine matrix into (scale, rotation, translation).
    ///
    /// Notes:
    /// - Assumes the matrix represents an affine transform (no projective component).
    /// - Tolerates mild shear by re-orthonormalizing the basis.
    /// - Preserves negative scale via determinant sign.
    #[inline]
    pub fn to_scale_rotation_translation(self) -> (Vec3, Quat, Vec3) {
        // Column-major basis vectors (ignore w).
        let x = Vec3::new(self.x_axis.x, self.x_axis.y, self.x_axis.z);
        let y = Vec3::new(self.y_axis.x, self.y_axis.y, self.y_axis.z);
        let z = Vec3::new(self.z_axis.x, self.z_axis.y, self.z_axis.z);

        let mut sx = x.length();
        let mut sy = y.length();
        let mut sz = z.length();

        // Robustness against NaNs/denormals.
        let eps = 1.0e-8;
        if !sx.is_finite() || sx < eps {
            sx = 0.0;
        }
        if !sy.is_finite() || sy < eps {
            sy = 0.0;
        }
        if !sz.is_finite() || sz < eps {
            sz = 0.0;
        }

        let inv_sx = if sx > 0.0 { 1.0 / sx } else { 0.0 };
        let inv_sy = if sy > 0.0 { 1.0 / sy } else { 0.0 };
        let inv_sz = if sz > 0.0 { 1.0 / sz } else { 0.0 };

        // Preserve the handedness of the authored basis *before* rebuilding the
        // orthonormal Z axis. The previous implementation computed determinant
        // after `rz = rx.cross(ry)`, which is right-handed by construction and
        // therefore silently lost reflected/negative-scale transforms such as
        // NorthStar helper joints with diag(+1,+1,-1).
        let authored_rx = (x * inv_sx).normalize_or_zero();
        let authored_ry = (y * inv_sy).normalize_or_zero();
        let authored_rz = (z * inv_sz).normalize_or_zero();
        let authored_handedness = authored_rx.dot(authored_ry.cross(authored_rz));

        // Normalize basis (remove scale first, then Gram-Schmidt to remove mild shear drift).
        let rx = authored_rx;
        let mut ry = y * inv_sy;
        ry = (ry - rx * ry.dot(rx)).normalize_or_zero();
        let rz = rx.cross(ry);

        // A quaternion always represents a proper rotation (det=+1), so encode a
        // reflected affine basis as a signed scale. Using Z keeps X/Y orientation
        // stable and reconstructs the original matrix exactly for pure S/R/T input.
        if authored_handedness < 0.0 {
            // Keep the quaternion basis proper (det=+1) and encode the
            // reflection exclusively in signed Z scale. For a reflected
            // authored basis `authored_rz ≈ -cross(rx, ry)`, so the positive
            // cross-product rotation multiplied by negative `sz` reconstructs
            // the original Z column exactly.
            sz = -sz;
        }

        let rot_m = Mat3::from_cols(rx, ry, rz);
        let rot = Quat::from_mat3(&rot_m).normalize_or_identity();

        let scale = Vec3::new(sx, sy, sz);
        let trans = Vec3::new(self.w_axis.x, self.w_axis.y, self.w_axis.z);

        (scale, rot, trans)
    }

    #[inline]
    pub const fn to_cols_array_2d(&self) -> [[f32; 4]; 4] {
        [
            [self.x_axis.x, self.x_axis.y, self.x_axis.z, self.x_axis.w],
            [self.y_axis.x, self.y_axis.y, self.y_axis.z, self.y_axis.w],
            [self.z_axis.x, self.z_axis.y, self.z_axis.z, self.z_axis.w],
            [self.w_axis.x, self.w_axis.y, self.w_axis.z, self.w_axis.w],
        ]
    }

    /// Creates a matrix from a 4x4 column-major array (array of columns).
    #[inline]
    pub const fn from_cols_array_2d(m: &[[f32; 4]; 4]) -> Self {
        Self::from_cols(
            Vec4::new(m[0][0], m[0][1], m[0][2], m[0][3]),
            Vec4::new(m[1][0], m[1][1], m[1][2], m[1][3]),
            Vec4::new(m[2][0], m[2][1], m[2][2], m[2][3]),
            Vec4::new(m[3][0], m[3][1], m[3][2], m[3][3]),
        )
    }

    #[inline]
    pub fn from_cols_array(m: &[f32; 16]) -> Self {
        Self::from_cols(
            Vec4::new(m[0], m[1], m[2], m[3]),
            Vec4::new(m[4], m[5], m[6], m[7]),
            Vec4::new(m[8], m[9], m[10], m[11]),
            Vec4::new(m[12], m[13], m[14], m[15]),
        )
    }

    #[inline]
    pub fn to_cols_array(&self) -> [f32; 16] {
        [
            self.x_axis.x,
            self.x_axis.y,
            self.x_axis.z,
            self.x_axis.w,
            self.y_axis.x,
            self.y_axis.y,
            self.y_axis.z,
            self.y_axis.w,
            self.z_axis.x,
            self.z_axis.y,
            self.z_axis.z,
            self.z_axis.w,
            self.w_axis.x,
            self.w_axis.y,
            self.w_axis.z,
            self.w_axis.w,
        ]
    }

    #[inline]
    pub fn from_translation(t: Vec3) -> Self {
        Self::from_cols(
            Vec4::new(1.0, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 1.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(t.x, t.y, t.z, 1.0),
        )
    }

    #[inline]
    pub fn from_quat(q: Quat) -> Self {
        let r = Mat3::from_quat(q);
        Self::from_cols(
            Vec4::new(r.x_axis.x, r.x_axis.y, r.x_axis.z, 0.0),
            Vec4::new(r.y_axis.x, r.y_axis.y, r.y_axis.z, 0.0),
            Vec4::new(r.z_axis.x, r.z_axis.y, r.z_axis.z, 0.0),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        )
    }

    #[inline]
    pub fn from_scale_rotation_translation(scale: Vec3, rot: Quat, trans: Vec3) -> Self {
        let r = Mat3::from_quat(rot);
        let x = r.x_axis * scale.x;
        let y = r.y_axis * scale.y;
        let z = r.z_axis * scale.z;
        Self::from_cols(
            Vec4::new(x.x, x.y, x.z, 0.0),
            Vec4::new(y.x, y.y, y.z, 0.0),
            Vec4::new(z.x, z.y, z.z, 0.0),
            Vec4::new(trans.x, trans.y, trans.z, 1.0),
        )
    }

    #[inline]
    pub fn perspective_rh(fovy_radians: f32, aspect: f32, z_near: f32, z_far: f32) -> Self {
        // Right-handed, depth 0..1 (Vulkan/D3D).
        // Matches `glam::Mat4::perspective_rh`.
        let f = 1.0 / (0.5 * fovy_radians).tan();
        let nf = 1.0 / (z_near - z_far);
        Self::from_cols(
            Vec4::new(f / aspect, 0.0, 0.0, 0.0),
            Vec4::new(0.0, f, 0.0, 0.0),
            Vec4::new(0.0, 0.0, z_far * nf, -1.0),
            Vec4::new(0.0, 0.0, (z_far * z_near) * nf, 0.0),
        )
    }

    #[inline]
    pub fn orthographic_rh(
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        z_near: f32,
        z_far: f32,
    ) -> Self {
        // Right-handed, depth 0..1.
        let rcp_w = 1.0 / (right - left);
        let rcp_h = 1.0 / (top - bottom);
        let rcp_d = 1.0 / (z_near - z_far);
        Self::from_cols(
            Vec4::new(2.0 * rcp_w, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 2.0 * rcp_h, 0.0, 0.0),
            Vec4::new(0.0, 0.0, rcp_d, 0.0),
            Vec4::new(
                -(right + left) * rcp_w,
                -(top + bottom) * rcp_h,
                z_near * rcp_d,
                1.0,
            ),
        )
    }

    #[inline]
    pub fn look_at_rh(eye: Vec3, center: Vec3, up: Vec3) -> Self {
        let f = (center - eye).normalize_or_zero();
        let s = f.cross(up).normalize_or_zero();
        let u = s.cross(f);

        // Column-major.
        Self::from_cols(
            Vec4::new(s.x, u.x, -f.x, 0.0),
            Vec4::new(s.y, u.y, -f.y, 0.0),
            Vec4::new(s.z, u.z, -f.z, 0.0),
            Vec4::new(-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0),
        )
    }

    #[inline]
    pub fn inverse(self) -> Self {
        // Generic 4x4 inversion (Gauss-Jordan / adjugate).
        // Fast enough for editor/runtime usage (not per-vertex).
        let m = self.to_cols_array();

        let mut inv = [0.0_f32; 16];

        inv[0] = m[5] * m[10] * m[15] - m[5] * m[11] * m[14] - m[9] * m[6] * m[15]
            + m[9] * m[7] * m[14]
            + m[13] * m[6] * m[11]
            - m[13] * m[7] * m[10];

        inv[4] = -m[4] * m[10] * m[15] + m[4] * m[11] * m[14] + m[8] * m[6] * m[15]
            - m[8] * m[7] * m[14]
            - m[12] * m[6] * m[11]
            + m[12] * m[7] * m[10];

        inv[8] = m[4] * m[9] * m[15] - m[4] * m[11] * m[13] - m[8] * m[5] * m[15]
            + m[8] * m[7] * m[13]
            + m[12] * m[5] * m[11]
            - m[12] * m[7] * m[9];

        inv[12] = -m[4] * m[9] * m[14] + m[4] * m[10] * m[13] + m[8] * m[5] * m[14]
            - m[8] * m[6] * m[13]
            - m[12] * m[5] * m[10]
            + m[12] * m[6] * m[9];

        inv[1] = -m[1] * m[10] * m[15] + m[1] * m[11] * m[14] + m[9] * m[2] * m[15]
            - m[9] * m[3] * m[14]
            - m[13] * m[2] * m[11]
            + m[13] * m[3] * m[10];

        inv[5] = m[0] * m[10] * m[15] - m[0] * m[11] * m[14] - m[8] * m[2] * m[15]
            + m[8] * m[3] * m[14]
            + m[12] * m[2] * m[11]
            - m[12] * m[3] * m[10];

        inv[9] = -m[0] * m[9] * m[15] + m[0] * m[11] * m[13] + m[8] * m[1] * m[15]
            - m[8] * m[3] * m[13]
            - m[12] * m[1] * m[11]
            + m[12] * m[3] * m[9];

        inv[13] = m[0] * m[9] * m[14] - m[0] * m[10] * m[13] - m[8] * m[1] * m[14]
            + m[8] * m[2] * m[13]
            + m[12] * m[1] * m[10]
            - m[12] * m[2] * m[9];

        inv[2] = m[1] * m[6] * m[15] - m[1] * m[7] * m[14] - m[5] * m[2] * m[15]
            + m[5] * m[3] * m[14]
            + m[13] * m[2] * m[7]
            - m[13] * m[3] * m[6];

        inv[6] = -m[0] * m[6] * m[15] + m[0] * m[7] * m[14] + m[4] * m[2] * m[15]
            - m[4] * m[3] * m[14]
            - m[12] * m[2] * m[7]
            + m[12] * m[3] * m[6];

        inv[10] = m[0] * m[5] * m[15] - m[0] * m[7] * m[13] - m[4] * m[1] * m[15]
            + m[4] * m[3] * m[13]
            + m[12] * m[1] * m[7]
            - m[12] * m[3] * m[5];

        inv[14] = -m[0] * m[5] * m[14] + m[0] * m[6] * m[13] + m[4] * m[1] * m[14]
            - m[4] * m[2] * m[13]
            - m[12] * m[1] * m[6]
            + m[12] * m[2] * m[5];

        inv[3] = -m[1] * m[6] * m[11] + m[1] * m[7] * m[10] + m[5] * m[2] * m[11]
            - m[5] * m[3] * m[10]
            - m[9] * m[2] * m[7]
            + m[9] * m[3] * m[6];

        inv[7] = m[0] * m[6] * m[11] - m[0] * m[7] * m[10] - m[4] * m[2] * m[11]
            + m[4] * m[3] * m[10]
            + m[8] * m[2] * m[7]
            - m[8] * m[3] * m[6];

        inv[11] = -m[0] * m[5] * m[11] + m[0] * m[7] * m[9] + m[4] * m[1] * m[11]
            - m[4] * m[3] * m[9]
            - m[8] * m[1] * m[7]
            + m[8] * m[3] * m[5];

        inv[15] = m[0] * m[5] * m[10] - m[0] * m[6] * m[9] - m[4] * m[1] * m[10]
            + m[4] * m[2] * m[9]
            + m[8] * m[1] * m[6]
            - m[8] * m[2] * m[5];

        let det = m[0] * inv[0] + m[1] * inv[4] + m[2] * inv[8] + m[3] * inv[12];
        if det.abs() < 1e-12 {
            return Self::IDENTITY;
        }
        let inv_det = 1.0 / det;
        for v in &mut inv {
            *v *= inv_det;
        }
        Self::from_cols_array(&inv)
    }
}

impl Mul for Mat4 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        let a = self;
        let b = rhs;
        // Multiply columns: result_col = a * b_col
        let bx = b.x_axis;
        let by = b.y_axis;
        let bz = b.z_axis;
        let bw = b.w_axis;
        Self::from_cols(a * bx, a * by, a * bz, a * bw)
    }
}

impl MulAssign for Mat4 {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl Mul<Vec4> for Mat4 {
    type Output = Vec4;

    #[inline]
    fn mul(self, rhs: Vec4) -> Self::Output {
        // Column-major: v' = x*vx + y*vy + z*vz + w*vw
        self.x_axis * rhs.x + self.y_axis * rhs.y + self.z_axis * rhs.z + self.w_axis * rhs.w
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_srt_round_trip(source: Mat4) {
        let (scale, rotation, translation) = source.to_scale_rotation_translation();
        let rebuilt = Mat4::from_scale_rotation_translation(scale, rotation, translation);
        let error = source
            .to_cols_array()
            .iter()
            .zip(rebuilt.to_cols_array().iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f32, f32::max);
        assert!(error <= 1.0e-6, "reflected SRT round-trip error={error}");
        assert!(
            scale.z < 0.0,
            "reflection must remain encoded in signed scale: {scale:?}"
        );
    }

    #[test]
    fn reflected_srt_round_trips_without_losing_scale_sign() {
        assert_srt_round_trip(Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, 1.0, -1.0),
            Quat::IDENTITY,
            Vec3::new(0.25, -0.5, 1.25),
        ));
    }

    #[test]
    fn rotated_reflected_srt_round_trips_without_improper_quaternion_basis() {
        assert_srt_round_trip(Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, 1.0, -1.0),
            Quat::from_rotation_y(0.37) * Quat::from_rotation_x(-0.21),
            Vec3::new(-0.1, 0.75, 0.33),
        ));
    }
}
