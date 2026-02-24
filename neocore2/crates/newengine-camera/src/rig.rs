#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Mat4, Quat, Vec3};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Camera transform in world space.
///
/// The rig is purely spatial; projection is handled separately by `Projection`.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraRig {
    pub position: Vec3,
    pub rotation: Quat,
}

impl Default for CameraRig {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        }
    }
}

impl CameraRig {
    #[inline]
    pub fn new(position: Vec3, rotation: Quat) -> Self {
        Self { position, rotation }
    }

    #[inline]
    pub fn forward(&self) -> Vec3 {
        self.rotation * Vec3::Z * -1.0
    }

    #[inline]
    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::X
    }

    #[inline]
    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    /// World->View matrix.
    #[inline]
    pub fn view_matrix(&self) -> Mat4 {
        // View = inverse(world transform).
        // World transform: T * R
        // Inverse: R^-1 * T^-1
        Mat4::from_quat(self.rotation.conjugate()) * Mat4::from_translation(-self.position)
    }

    /// View->World matrix.
    #[inline]
    pub fn world_matrix(&self) -> Mat4 {
        Mat4::from_translation(self.position) * Mat4::from_quat(self.rotation)
    }

    /// Adds a local-space translation (relative to the current rotation).
    #[inline]
    pub fn translate_local(&mut self, delta_local: Vec3) {
        self.position += self.rotation * delta_local;
    }

    /// Adds a world-space translation.
    #[inline]
    pub fn translate_world(&mut self, delta_world: Vec3) {
        self.position += delta_world;
    }

    /// Sets the rig transform from a look-at target.
    ///
    /// Convention: camera forward is -Z.
    #[inline]
    pub fn set_look_at(&mut self, position: Vec3, target: Vec3, up: Vec3) {
        self.position = position;
        self.rotation = look_at_rotation(position, target, up);
    }

    /// Creates a rig from a look-at target.
    ///
    /// Convention: camera forward is -Z.
    #[inline]
    pub fn from_look_at(position: Vec3, target: Vec3, up: Vec3) -> Self {
        Self {
            position,
            rotation: look_at_rotation(position, target, up),
        }
    }
}

#[inline]
fn look_at_rotation(position: Vec3, target: Vec3, up: Vec3) -> Quat {
    let f = (target - position).normalize_or_zero();
    if f.length_squared() < 1e-8 {
        return Quat::IDENTITY;
    }

    // We want the camera -Z axis to point towards `f`.
    let z_axis = (-f).normalize();
    let mut x_axis = up.cross(z_axis);
    if x_axis.length_squared() < 1e-8 {
        // Fallback if up is parallel to forward.
        x_axis = Vec3::Y.cross(z_axis);
        if x_axis.length_squared() < 1e-8 {
            x_axis = Vec3::X.cross(z_axis);
        }
    }
    x_axis = x_axis.normalize();
    let y_axis = z_axis.cross(x_axis).normalize();

    // Column-major 3x3 basis in world space.
    let m = newengine_math::Mat3::from_cols(x_axis, y_axis, z_axis);
    Quat::from_mat3(&m)
}
