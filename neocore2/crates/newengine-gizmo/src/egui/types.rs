use crate::{GizmoAxis, GizmoMode};
use egui::Pos2;
use newengine_math::{Quat, Vec3};

/// World-space transform manipulated by the gizmo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GizmoTransform {
    pub pos: Vec3,
    pub rot: Quat,
    pub scale: Vec3,
}

impl GizmoTransform {
    #[inline]
    pub const fn new(pos: Vec3, rot: Quat, scale: Vec3) -> Self {
        Self { pos, rot, scale }
    }
}

/// Visual and interaction tuning knobs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GizmoStyle {
    pub axis_len_pt: f32,
    pub line_width_pt: f32,
    pub pick_radius_pt: f32,
    pub arrow_size_pt: f32,
    pub highlight_mul: f32,

    pub rotate_radius_pt: f32,
    pub rotate_width_pt: f32,
    pub rotate_segments: u32,
    pub rotate_fill_alpha: u8,
    pub rotate_back_alpha: u8,

    /// Rotation snapping step (degrees). 0 disables snapping.
    pub snap_rotate_deg: f32,
    /// Enable snapping only while Shift is held.
    pub snap_on_shift: bool,
}

impl Default for GizmoStyle {
    #[inline]
    fn default() -> Self {
        Self {
            axis_len_pt: 72.0,
            line_width_pt: 2.0,
            pick_radius_pt: 7.0,
            arrow_size_pt: 10.0,
            highlight_mul: 1.35,

            rotate_radius_pt: 78.0,
            rotate_width_pt: 4.0,
            rotate_segments: 96,
            rotate_fill_alpha: 70,
            rotate_back_alpha: 35,

            snap_rotate_deg: 15.0,
            snap_on_shift: true,
        }
    }
}

/// Output of a gizmo update pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GizmoOutput {
    /// True when the gizmo should capture mouse input (prevents camera navigation / selection).
    pub capture: bool,
    /// Currently hovered axis.
    pub hovered_axis: Option<GizmoAxis>,
    /// Currently active (dragged) axis.
    pub active_axis: Option<GizmoAxis>,
    /// Updated transform when changed.
    pub transform: Option<GizmoTransform>,
}

impl Default for GizmoOutput {
    #[inline]
    fn default() -> Self {
        Self {
            capture: false,
            hovered_axis: None,
            active_axis: None,
            transform: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DragState {
    pub(crate) mode: GizmoMode,
    pub(crate) axis: GizmoAxis,
    pub(crate) start_mouse: Pos2,
    pub(crate) start: GizmoTransform,
    pub(crate) ndc_z: f32,

    // Rotate-only cached.
    pub(crate) plane_u: Vec3,
    pub(crate) plane_v: Vec3,
    pub(crate) start_angle: f32,
}
