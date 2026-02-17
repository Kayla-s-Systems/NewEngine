#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_gizmo::egui::GizmoCamera;

pub(super) struct FrameCamera<'a> {
    pub(super) frame: &'a crate::viewport_bridge::ViewportCameraFrame,
}

impl<'a> GizmoCamera for FrameCamera<'a> {
    #[inline]
    fn viewproj(&self) -> newengine_math::Mat4 {
        self.frame.viewproj
    }

    #[inline]
    fn inv_viewproj(&self) -> newengine_math::Mat4 {
        self.frame.inv_viewproj
    }

    #[inline]
    fn viewport_px(&self) -> (u32, u32) {
        (self.frame.vp_w, self.frame.vp_h)
    }
}
