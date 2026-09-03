use newengine_camera_api::CameraProjectionKind;
use newengine_math::Vec3;

use crate::scene_bridge::EngineViewGatewayFrame;

#[derive(Clone, Copy, Debug)]
pub struct CameraProjectionBounds {
    pub world_center: Vec3,
    pub world_radius: f32,
    pub selection_center: Option<Vec3>,
    pub selection_radius: Option<f32>,
}

pub fn apply_camera_projection(
    controller: &mut newengine_editor_viewport_runtime::EditorViewportController,
    frame: &mut EngineViewGatewayFrame,
    bounds: CameraProjectionBounds,
    wheel_y: f32,
    viewport_size: [u32; 2],
) {
    let Some(projection) = controller.resolve_camera_projection(
        bounds.world_center,
        bounds.world_radius,
        bounds.selection_center,
        bounds.selection_radius,
        wheel_y,
        viewport_size,
    ) else {
        return;
    };

    frame.view.view = projection.view;
    frame.view.projection = projection.projection;
    frame.view.view_projection = projection.view_projection;
    frame.view.inverse_view = projection.inverse_view;
    frame.view.position_ws = projection.position_ws;
    frame.view.position_ws_f64 = [
        projection.position_ws.x as f64,
        projection.position_ws.y as f64,
        projection.position_ws.z as f64,
    ];
    frame.view.position_origin_relative_ws = projection.position_ws;
    frame.view.forward_ws = projection.forward_ws;
    frame.view.viewport_width = projection.viewport_width;
    frame.view.viewport_height = projection.viewport_height;
    frame.view.aspect = projection.aspect;

    let snapshot = &mut frame.camera_snapshot;
    snapshot.view_cols = projection.view.to_cols_array_2d();
    snapshot.projection_cols = projection.projection.to_cols_array_2d();
    snapshot.view_projection_cols = projection.view_projection.to_cols_array_2d();
    snapshot.inverse_view_cols = projection.inverse_view.to_cols_array_2d();
    snapshot.inverse_projection_cols = projection.projection.inverse().to_cols_array_2d();
    snapshot.inverse_view_projection_cols = projection.view_projection.inverse().to_cols_array_2d();
    snapshot.position_ws = [
        projection.position_ws.x,
        projection.position_ws.y,
        projection.position_ws.z,
    ];
    snapshot.position_ws_f64 = [
        projection.position_ws.x as f64,
        projection.position_ws.y as f64,
        projection.position_ws.z as f64,
    ];
    snapshot.position_origin_relative_ws = [
        projection.position_ws.x,
        projection.position_ws.y,
        projection.position_ws.z,
    ];
    snapshot.forward_ws = [
        projection.forward_ws.x,
        projection.forward_ws.y,
        projection.forward_ws.z,
    ];
    snapshot.right_ws = [
        projection.right_ws.x,
        projection.right_ws.y,
        projection.right_ws.z,
    ];
    snapshot.up_ws = [projection.up_ws.x, projection.up_ws.y, projection.up_ws.z];
    snapshot.viewport.width = projection.viewport_width;
    snapshot.viewport.height = projection.viewport_height;
    snapshot.viewport.aspect = projection.aspect;
    snapshot.projection.kind = CameraProjectionKind::Orthographic;
    snapshot.projection.aspect = projection.aspect;
    snapshot.projection.half_height = projection.half_height;
    snapshot.projection.near = projection.near;
    snapshot.projection.far = projection.far;
    snapshot.finite = projection.finite;
}
