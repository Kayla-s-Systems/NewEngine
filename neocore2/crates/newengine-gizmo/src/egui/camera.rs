use newengine_math::Mat4;

/// Camera interface required by the overlay gizmo implementation.
///
/// The gizmo operates in screen space, but needs view/projection matrices
/// to project and unproject points.
pub trait GizmoCamera {
    /// View-projection matrix.
    fn viewproj(&self) -> Mat4;
    /// Inverse view-projection matrix.
    fn inv_viewproj(&self) -> Mat4;
    /// Viewport size in pixels.
    fn viewport_px(&self) -> (u32, u32);
}
