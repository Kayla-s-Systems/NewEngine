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
    /// Screen-space size of axis end caps (cube handles) in points.
    pub axis_cap_pt: f32,
    /// Screen-space radius of the center handle in points.
    pub center_radius_pt: f32,
    pub highlight_mul: f32,

    pub rotate_radius_pt: f32,
    pub rotate_width_pt: f32,
    /// Extra thickness added for the front half of axis rings.
    pub rotate_front_width_add_pt: f32,
    /// Thickness multiplier for the back (dashed) half of axis rings.
    pub rotate_back_width_mul: f32,
    /// Dash length for the back half (points).
    pub rotate_back_dash_pt: f32,
    /// Gap length for the back half (points).
    pub rotate_back_gap_pt: f32,
    /// Base alpha for the glow pass.
    pub rotate_glow_alpha: u8,
    /// Extra thickness for the glow pass.
    pub rotate_glow_width_add_pt: f32,
    /// Additional alpha boost for hovered/active glow.
    pub rotate_hot_glow_alpha_add: u8,
    /// Additional thickness for hovered/active glow.
    pub rotate_hot_glow_width_add_pt: f32,
    /// Screen-space outer ring radius for free-rotate (points).
    pub screen_ring_radius_pt: f32,
    /// Screen-space outer ring stroke width (points).
    pub screen_ring_width_pt: f32,
    pub rotate_segments: u32,
    /// Visible arc span for each axis ring (degrees).
    ///
    /// A "runtime"/"shipping" gizmo often renders short, thick arcs instead of full rings.
    pub rotate_arc_deg: f32,
    pub rotate_fill_alpha: u8,
    pub rotate_back_alpha: u8,

    /// Hover/active plane fill alpha for the UE-style wedge (0 disables).
    pub rotate_plane_fill_alpha: u8,
    /// Alpha for the grid lines inside the plane wedge.
    pub rotate_plane_grid_alpha: u8,
    /// Number of angular grid divisions inside the wedge.
    pub rotate_plane_grid_angular: u8,
    /// Number of radial grid divisions inside the wedge.
    pub rotate_plane_grid_radial: u8,

    /// Rotation snapping step (degrees). 0 disables snapping.
    pub snap_rotate_deg: f32,
    /// Enable snapping only while Shift is held.
    pub snap_on_shift: bool,
}

impl Default for GizmoStyle {
    #[inline]
    fn default() -> Self {
        Self {
            // "AAA runtime" preset: chunky, readable, stable.
            axis_len_pt: 64.0,
            line_width_pt: 3.0,
            pick_radius_pt: 9.0,
            axis_cap_pt: 12.0,
            center_radius_pt: 6.0,
            highlight_mul: 1.35,

            // UE5-style rotation widget: full rings with depth cue (front solid, back dashed/faded).
            rotate_radius_pt: 72.0,
            rotate_width_pt: 4.0,
            rotate_front_width_add_pt: 1.0,
            rotate_back_width_mul: 0.70,
            rotate_back_dash_pt: 7.0,
            rotate_back_gap_pt: 6.0,
            rotate_glow_alpha: 58,
            rotate_glow_width_add_pt: 6.0,
            rotate_hot_glow_alpha_add: 28,
            rotate_hot_glow_width_add_pt: 3.0,
            // Outer "view" ring (free rotate around view axis).
            screen_ring_radius_pt: 92.0,
            screen_ring_width_pt: 3.0,
            rotate_segments: 128,
            // Kept for compatibility; UE5 uses full rings.
            rotate_arc_deg: 360.0,
            rotate_fill_alpha: 0,
            rotate_back_alpha: 75,

            // Disable UE4-style plane wedges (UE5 default rotate gizmo doesn't fill planes).
            rotate_plane_fill_alpha: 0,
            rotate_plane_grid_alpha: 0,
            rotate_plane_grid_angular: 0,
            rotate_plane_grid_radial: 0,

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
    pub(crate) last_angle: f32,
    pub(crate) accum_angle: f32,
}
