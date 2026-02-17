//! Egui overlay gizmo implementation.
//!
//! This module is fully renderer-agnostic and draws via `egui::Painter`.
//! It also owns interaction logic (picking + dragging) so the editor stays thin.

mod camera;
mod controller;
mod draw_axis;
mod draw_rotate;
mod math;
mod pick;
mod types;

pub use camera::GizmoCamera;
pub use controller::EguiGizmo;
pub use types::{GizmoOutput, GizmoStyle, GizmoTransform};
