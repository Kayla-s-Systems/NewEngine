#![forbid(unsafe_op_in_unsafe_fn)]

use glam::{Mat4, Quat, Vec3};

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
}