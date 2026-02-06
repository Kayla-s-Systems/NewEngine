#![forbid(unsafe_op_in_unsafe_fn)]

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2, Vec3, Vec4};

/// CPU-side camera matrices.
///
/// Uses `glam` types for ergonomics and math. Not POD.
#[derive(Clone, Copy, Debug)]
pub struct CameraMatrices {
    pub view: Mat4,
    pub proj: Mat4,
    pub view_proj: Mat4,
    pub inv_view: Mat4,
    pub inv_proj: Mat4,
    pub inv_view_proj: Mat4,
    pub world_pos: Vec3,
    pub viewport: Vec4, // (w, h, 1/w, 1/h)
    pub jitter: Vec2,
}

impl Default for CameraMatrices {
    #[inline]
    fn default() -> Self {
        Self {
            view: Mat4::IDENTITY,
            proj: Mat4::IDENTITY,
            view_proj: Mat4::IDENTITY,
            inv_view: Mat4::IDENTITY,
            inv_proj: Mat4::IDENTITY,
            inv_view_proj: Mat4::IDENTITY,
            world_pos: Vec3::ZERO,
            viewport: Vec4::new(1.0, 1.0, 1.0, 1.0),
            jitter: Vec2::ZERO,
        }
    }
}

impl CameraMatrices {
    #[inline]
    pub fn new(view: Mat4, proj: Mat4, world_pos: Vec3, viewport_wh: Vec2, jitter: Vec2) -> Self {
        let view_proj = proj * view;
        let inv_view = view.inverse();
        let inv_proj = proj.inverse();
        let inv_view_proj = view_proj.inverse();

        let w = viewport_wh.x.max(1.0);
        let h = viewport_wh.y.max(1.0);

        Self {
            view,
            proj,
            view_proj,
            inv_view,
            inv_proj,
            inv_view_proj,
            world_pos,
            viewport: Vec4::new(w, h, 1.0 / w, 1.0 / h),
            jitter,
        }
    }

    /// Convert into a strict POD layout suitable for GPU uploads.
    #[inline]
    pub fn to_gpu(&self) -> GpuCameraMatrices {
        GpuCameraMatrices::from_cpu(*self)
    }
}

/// GPU-friendly camera constants.
///
/// Column-major matrices (GLSL convention), `std140`-friendly padding.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct GpuCameraMatrices {
    pub view: [[f32; 4]; 4],
    pub proj: [[f32; 4]; 4],
    pub view_proj: [[f32; 4]; 4],
    pub inv_view: [[f32; 4]; 4],
    pub inv_proj: [[f32; 4]; 4],
    pub inv_view_proj: [[f32; 4]; 4],

    pub world_pos: [f32; 3],
    pub _pad0: f32,

    pub viewport: [f32; 4], // (w, h, 1/w, 1/h)

    pub jitter: [f32; 2],
    pub _pad1: [f32; 2],
}

impl GpuCameraMatrices {
    #[inline]
    pub fn from_cpu(c: CameraMatrices) -> Self {
        Self {
            view: mat4_to_cols(c.view),
            proj: mat4_to_cols(c.proj),
            view_proj: mat4_to_cols(c.view_proj),
            inv_view: mat4_to_cols(c.inv_view),
            inv_proj: mat4_to_cols(c.inv_proj),
            inv_view_proj: mat4_to_cols(c.inv_view_proj),
            world_pos: [c.world_pos.x, c.world_pos.y, c.world_pos.z],
            _pad0: 0.0,
            viewport: [c.viewport.x, c.viewport.y, c.viewport.z, c.viewport.w],
            jitter: [c.jitter.x, c.jitter.y],
            _pad1: [0.0, 0.0],
        }
    }
}

#[inline]
fn mat4_to_cols(m: Mat4) -> [[f32; 4]; 4] {
    // glam Mat4 is column-major: x_axis/y_axis/z_axis/w_axis are columns (Vec4).
    [
        [m.x_axis.x, m.x_axis.y, m.x_axis.z, m.x_axis.w],
        [m.y_axis.x, m.y_axis.y, m.y_axis.z, m.y_axis.w],
        [m.z_axis.x, m.z_axis.y, m.z_axis.z, m.z_axis.w],
        [m.w_axis.x, m.w_axis.y, m.w_axis.z, m.w_axis.w],
    ]
}