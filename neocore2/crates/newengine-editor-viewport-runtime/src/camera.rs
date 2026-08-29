use newengine_camera::{Orthographic, Projection};
use newengine_math::{Mat4, Vec3};
use newengine_ui_api::UiEditorViewportProjection;

use super::EditorViewportController;

#[derive(Clone, Copy, Debug)]
pub struct EditorCameraProjectionOverride {
    pub view: Mat4,
    pub projection: Mat4,
    pub view_projection: Mat4,
    pub inverse_view: Mat4,
    pub position_ws: Vec3,
    pub forward_ws: Vec3,
    pub right_ws: Vec3,
    pub up_ws: Vec3,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
    pub half_height: f32,
    pub finite: bool,
}

impl EditorViewportController {
    pub fn resolve_camera_projection(
        &mut self,
        world_center: Vec3,
        world_radius: f32,
        selection_center: Option<Vec3>,
        selection_radius: Option<f32>,
        wheel_y: f32,
        viewport_size: [u32; 2],
    ) -> Option<EditorCameraProjectionOverride> {
        if self.state.projection == UiEditorViewportProjection::Perspective {
            return None;
        }
        let center = selection_center.unwrap_or(world_center);
        let radius = selection_radius.unwrap_or(world_radius).max(0.25);
        if self.last_projection != self.state.projection
            || !self.orthographic_half_height.is_finite()
        {
            self.orthographic_half_height = (radius * 1.75).clamp(0.5, 10_000.0);
            self.last_projection = self.state.projection;
        }
        if wheel_y.abs() > f32::EPSILON {
            self.orthographic_half_height =
                (self.orthographic_half_height * (-wheel_y * 0.12).exp()).clamp(0.05, 100_000.0);
        } else if self.orthographic_half_height <= 0.0 {
            self.orthographic_half_height = (radius * 1.75).max(0.5);
        }
        let distance = (radius * 6.0).max(10.0);
        let (eye, up) = match self.state.projection {
            UiEditorViewportProjection::Top => (center + Vec3::Y * distance, -Vec3::Z),
            UiEditorViewportProjection::Front => (center + Vec3::Z * distance, Vec3::Y),
            UiEditorViewportProjection::Side => (center + Vec3::X * distance, Vec3::Y),
            UiEditorViewportProjection::Perspective => return None,
        };
        let width = viewport_size[0].max(1);
        let height = viewport_size[1].max(1);
        let aspect = width as f32 / height as f32;
        let near = 0.01;
        let far = (distance + world_radius.max(radius) * 8.0 + 100.0).max(1_000.0);
        let view = Mat4::look_at_rh(eye, center, up);
        let projection = Projection::Orthographic(Orthographic::new(
            self.orthographic_half_height,
            aspect,
            near,
            far,
        ))
        .matrix();
        let view_projection = projection * view;
        let inverse_view = view.inverse();
        let forward = (center - eye).normalize_or_zero();
        let right = inverse_view.x_axis.truncate().normalize_or_zero();
        let camera_up = inverse_view.y_axis.truncate().normalize_or_zero();
        Some(EditorCameraProjectionOverride {
            view,
            projection,
            view_projection,
            inverse_view,
            position_ws: eye,
            forward_ws: forward,
            right_ws: right,
            up_ws: camera_up,
            viewport_width: width,
            viewport_height: height,
            aspect,
            near,
            far,
            half_height: self.orthographic_half_height,
            finite: view_projection
                .to_cols_array()
                .iter()
                .all(|v| v.is_finite()),
        })
    }
}
