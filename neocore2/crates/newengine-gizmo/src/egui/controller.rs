use super::camera::GizmoCamera;
use super::draw_axis::{axis_color, draw_axis, draw_axis_scale};
use super::draw_rotate::draw_rotate_gizmo;
use super::math::{axis_end, plane_basis, rotation_angle_on_plane, screen_to_world_at_ndc_z, world_to_screen};
use super::pick::{pick_non_rotate_axis, pick_rotate_axis};
use super::types::{DragState, GizmoOutput, GizmoStyle, GizmoTransform};
use crate::{GizmoAxis, GizmoMode};
use egui::{Painter, Pos2, Rect};
use newengine_math::Quat;

/// Egui-based gizmo controller and overlay renderer.
///
/// This is intentionally renderer-agnostic and uses only `egui::Painter`.
pub struct EguiGizmo {
    mode: GizmoMode,
    style: GizmoStyle,
    drag: Option<DragState>,
}

impl Default for EguiGizmo {
    #[inline]
    fn default() -> Self {
        Self {
            mode: GizmoMode::default(),
            style: GizmoStyle::default(),
            drag: None,
        }
    }
}

impl EguiGizmo {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn mode(&self) -> GizmoMode {
        self.mode
    }

    #[inline]
    pub fn set_mode(&mut self, mode: GizmoMode) {
        if self.mode != mode {
            self.mode = mode;
            self.drag = None;
        }
    }

    #[inline]
    pub fn style(&self) -> GizmoStyle {
        self.style
    }

    #[inline]
    pub fn set_style(&mut self, style: GizmoStyle) {
        self.style = style;
    }

    #[inline]
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Returns true if the gizmo wants to capture mouse input this frame.
    ///
    /// This is designed to be queried before camera navigation / selection logic.
    pub fn wants_capture_now(&self, ctx: &egui::Context, rect: Rect, camera: &impl GizmoCamera, tr: GizmoTransform) -> bool {
        if self.drag.is_some() {
            return true;
        }

        let Some(m) = ctx.input(|i| i.pointer.interact_pos()) else {
            return false;
        };
        if !rect.contains(m) {
            return false;
        }
        if !ctx.input(|i| i.pointer.primary_down()) {
            return false;
        }

        self.pick_axis(camera, rect, tr, m).is_some()
    }

    /// Runs gizmo interaction update and draws overlay.
    pub fn run(
        &mut self,
        painter: &Painter,
        ctx: &egui::Context,
        rect: Rect,
        camera: &impl GizmoCamera,
        tr: GizmoTransform,
    ) -> GizmoOutput {
        let mut out = GizmoOutput::default();

        let Some((center, ndc_z)) = world_to_screen(camera, rect, tr.pos) else {
            self.drag = None;
            return out;
        };

        let mouse = ctx.input(|i| i.pointer.interact_pos());
        let hovered = mouse.filter(|m| rect.contains(*m)).and_then(|m| self.pick_axis(camera, rect, tr, m));

        out.hovered_axis = hovered;

        // Drag start.
        let just_pressed = ctx.input(|i| i.pointer.primary_pressed());
        if self.drag.is_none() && just_pressed {
            if let (Some(axis), Some(m)) = (hovered, mouse) {
                let axis_world = (tr.rot * axis.vec3()).normalize_or_zero();
                let (plane_u, plane_v) = plane_basis(axis_world);
                let start_angle = rotation_angle_on_plane(camera, rect, tr.pos, axis_world, plane_u, plane_v, m);

                self.drag = Some(DragState {
                    mode: self.mode,
                    axis,
                    start_mouse: m,
                    start: tr,
                    ndc_z,
                    plane_u,
                    plane_v,
                    start_angle,
                });
            }
        }

        // Drag update.
        if let Some(drag) = self.drag {
            out.active_axis = Some(drag.axis);
            out.capture = true;

            let lmb_down = ctx.input(|i| i.pointer.primary_down());
            if !lmb_down {
                self.drag = None;
            } else if let Some(m) = mouse {
                let axis_world = (drag.start.rot * drag.axis.vec3()).normalize_or_zero();

                let new_tr = match drag.mode {
                    GizmoMode::Translate => {
                        let ws0 = screen_to_world_at_ndc_z(camera, rect, drag.start_mouse, drag.ndc_z);
                        let ws1 = screen_to_world_at_ndc_z(camera, rect, m, drag.ndc_z);
                        let delta = (ws1 - ws0).dot(axis_world);
                        GizmoTransform {
                            pos: drag.start.pos + axis_world * delta,
                            rot: drag.start.rot,
                            scale: drag.start.scale,
                        }
                    }
                    GizmoMode::Scale => {
                        let ws0 = screen_to_world_at_ndc_z(camera, rect, drag.start_mouse, drag.ndc_z);
                        let ws1 = screen_to_world_at_ndc_z(camera, rect, m, drag.ndc_z);
                        let delta = (ws1 - ws0).dot(axis_world);

                        let mut s = drag.start.scale;
                        match drag.axis {
                            GizmoAxis::X => s.x = (s.x + delta).max(0.001),
                            GizmoAxis::Y => s.y = (s.y + delta).max(0.001),
                            GizmoAxis::Z => s.z = (s.z + delta).max(0.001),
                        }

                        GizmoTransform {
                            pos: drag.start.pos,
                            rot: drag.start.rot,
                            scale: s,
                        }
                    }
                    GizmoMode::Rotate => {
                        let a1 = rotation_angle_on_plane(camera, rect, drag.start.pos, axis_world, drag.plane_u, drag.plane_v, m);
                        let mut da = a1 - drag.start_angle;
                        while da > core::f32::consts::PI {
                            da -= 2.0 * core::f32::consts::PI;
                        }
                        while da < -core::f32::consts::PI {
                            da += 2.0 * core::f32::consts::PI;
                        }
                        let q = Quat::from_axis_angle(axis_world, da);
                        GizmoTransform {
                            pos: drag.start.pos,
                            rot: q * drag.start.rot,
                            scale: drag.start.scale,
                        }
                    }
                };

                out.transform = Some(new_tr);
            }
        } else {
            out.capture = self.wants_capture_now(ctx, rect, camera, tr);
        }

        // Draw.
        match self.mode {
            GizmoMode::Rotate => {
                draw_rotate_gizmo(
                    painter,
                    ctx,
                    rect,
                    camera,
                    tr,
                    hovered,
                    out.active_axis,
                    self.drag,
                    self.style,
                    center,
                );
            }
            _ => {
                let x_end = axis_end(camera, rect, tr.pos, tr.rot, GizmoAxis::X, center, self.style.axis_len_pt);
                let y_end = axis_end(camera, rect, tr.pos, tr.rot, GizmoAxis::Y, center, self.style.axis_len_pt);
                let z_end = axis_end(camera, rect, tr.pos, tr.rot, GizmoAxis::Z, center, self.style.axis_len_pt);

                match self.mode {
                    GizmoMode::Scale => {
                        draw_axis_scale(painter, center, x_end, axis_color(GizmoAxis::X, hovered, out.active_axis, self.style.highlight_mul), self.style);
                        draw_axis_scale(painter, center, y_end, axis_color(GizmoAxis::Y, hovered, out.active_axis, self.style.highlight_mul), self.style);
                        draw_axis_scale(painter, center, z_end, axis_color(GizmoAxis::Z, hovered, out.active_axis, self.style.highlight_mul), self.style);
                    }
                    _ => {
                        draw_axis(painter, center, x_end, axis_color(GizmoAxis::X, hovered, out.active_axis, self.style.highlight_mul), self.style);
                        draw_axis(painter, center, y_end, axis_color(GizmoAxis::Y, hovered, out.active_axis, self.style.highlight_mul), self.style);
                        draw_axis(painter, center, z_end, axis_color(GizmoAxis::Z, hovered, out.active_axis, self.style.highlight_mul), self.style);
                    }
                }
            }
        }

        out
    }

    fn pick_axis(&self, camera: &impl GizmoCamera, rect: Rect, tr: GizmoTransform, mouse: Pos2) -> Option<GizmoAxis> {
        match self.mode {
            GizmoMode::Rotate => pick_rotate_axis(camera, rect, tr, mouse, self.style),
            GizmoMode::Scale => pick_non_rotate_axis(camera, rect, tr, mouse, self.style, true),
            GizmoMode::Translate => pick_non_rotate_axis(camera, rect, tr, mouse, self.style, false),
        }
    }
}
