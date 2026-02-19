#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-agnostic gizmo primitives.
//!
//! This crate intentionally contains **no renderer/UI dependencies**.
//! It defines the common, reusable types required to build editor gizmos
//! (move/rotate/scale) while keeping the editor layer thin.

use newengine_math::Vec3;

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

/// Transform space used by the gizmo.
///
/// `Local` uses the selected transform's rotation to orient axes.
/// `World` uses world axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GizmoSpace {
    Local,
    World,
}

impl Default for GizmoSpace {
    #[inline]
    fn default() -> Self {
        // Industry default (e.g. DCC tools / Unreal): operate in World space unless explicitly switched.
        Self::World
    }
}

/// Primary axis handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GizmoAxis {
    X,
    Y,
    Z,
    /// Screen-space handle.
    ///
    /// For rotation this corresponds to "free rotate" around the camera view axis
    /// (industry standard: UE/DCC outer ring).
    Screen,
}

#[cfg(feature = "egui")]
pub mod egui;

impl GizmoAxis {
    /// Unit vector for the axis.
    #[inline]
    pub fn vec3(self) -> Vec3 {
        match self {
            GizmoAxis::X => Vec3::X,
            GizmoAxis::Y => Vec3::Y,
            GizmoAxis::Z => Vec3::Z,
            GizmoAxis::Screen => Vec3::ZERO,
        }
    }
}
