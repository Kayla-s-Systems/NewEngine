#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-agnostic gizmo primitives.
//!
//! This crate intentionally contains **no renderer/UI dependencies**.
//! It defines the common, reusable types required to build editor gizmos
//! (move/rotate/scale) while keeping the editor layer thin.

use glam::Vec3;

/// Transform tool mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GizmoMode {
    Translate,
    Rotate,
    Scale,
}

impl Default for GizmoMode {
    #[inline]
    fn default() -> Self {
        Self::Translate
    }
}

/// Primary axis handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GizmoAxis {
    X,
    Y,
    Z,
}

impl GizmoAxis {
    /// Unit vector for the axis.
    #[inline]
    pub fn vec3(self) -> Vec3 {
        match self {
            GizmoAxis::X => Vec3::X,
            GizmoAxis::Y => Vec3::Y,
            GizmoAxis::Z => Vec3::Z,
        }
    }
}
