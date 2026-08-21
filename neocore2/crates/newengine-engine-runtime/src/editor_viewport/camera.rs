use newengine_camera::{Orthographic, Projection};
use newengine_camera_api::CameraProjectionKind;
use newengine_math::{Mat4, Vec3};
use newengine_ui_api::UiEditorViewportProjection;

use super::EditorViewportController;
use crate::scene_bridge::EngineViewGatewayFrame;

impl EditorViewportController {
    pub(crate) fn apply_camera_projection(
        &mut self,
        frame: &mut EngineViewGatewayFrame,
        world_center: Vec3,
        world_radius: f32,
        selection_center: Option<Vec3>,
        selection_radius: Option<f32>,
        wheel_y: f32,
        viewport_size: [u32; 2],
    ) {
        if self.state.projection == UiEditorViewportProjection::Perspective {
            return;
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
            UiEditorViewportProjection::Perspective => return,
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

        frame.view.view = view;
        frame.view.projection = projection;
        frame.view.view_projection = view_projection;
        frame.view.inverse_view = inverse_view;
        frame.view.position_ws = eye;
        frame.view.position_ws_f64 = [eye.x as f64, eye.y as f64, eye.z as f64];
        frame.view.position_origin_relative_ws = eye;
        frame.view.forward_ws = forward;
        frame.view.viewport_width = width;
        frame.view.viewport_height = height;
        frame.view.aspect = aspect;

        let snapshot = &mut frame.camera_snapshot;
        snapshot.view_cols = view.to_cols_array_2d();
        snapshot.projection_cols = projection.to_cols_array_2d();
        snapshot.view_projection_cols = view_projection.to_cols_array_2d();
        snapshot.inverse_view_cols = inverse_view.to_cols_array_2d();
        snapshot.inverse_projection_cols = projection.inverse().to_cols_array_2d();
        snapshot.inverse_view_projection_cols = view_projection.inverse().to_cols_array_2d();
        snapshot.position_ws = [eye.x, eye.y, eye.z];
        snapshot.position_ws_f64 = [eye.x as f64, eye.y as f64, eye.z as f64];
        snapshot.position_origin_relative_ws = [eye.x, eye.y, eye.z];
        snapshot.forward_ws = [forward.x, forward.y, forward.z];
        let right = inverse_view.x_axis.truncate().normalize_or_zero();
        let camera_up = inverse_view.y_axis.truncate().normalize_or_zero();
        snapshot.right_ws = [right.x, right.y, right.z];
        snapshot.up_ws = [camera_up.x, camera_up.y, camera_up.z];
        snapshot.viewport.width = width;
        snapshot.viewport.height = height;
        snapshot.viewport.aspect = aspect;
        snapshot.projection.kind = CameraProjectionKind::Orthographic;
        snapshot.projection.aspect = aspect;
        snapshot.projection.half_height = self.orthographic_half_height;
        snapshot.projection.near = near;
        snapshot.projection.far = far;
        snapshot.finite = view_projection
            .to_cols_array()
            .iter()
            .all(|value| value.is_finite());
    }
}
