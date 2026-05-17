#![forbid(unsafe_op_in_unsafe_fn)]

use bytemuck::{Pod, Zeroable};
use newengine_math::{Mat4, Vec2, Vec3, Vec4};

use crate::{CameraChannelState, CameraRig, CameraViewport, Frustum, Projection};

/// CPU-side camera matrices.
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
    pub fn from_view_proj(
        view: Mat4,
        proj: Mat4,
        world_pos: Vec3,
        viewport: CameraViewport,
        jitter: Vec2,
    ) -> Self {
        let view_proj = proj * view;
        let inv_view = view.inverse();
        let inv_proj = proj.inverse();
        let inv_view_proj = view_proj.inverse();

        Self {
            view,
            proj,
            view_proj,
            inv_view,
            inv_proj,
            inv_view_proj,
            world_pos,
            viewport: viewport.uniform(),
            jitter,
        }
    }

    #[inline]
    pub fn to_gpu(&self) -> GpuCameraMatrices {
        GpuCameraMatrices::from_cpu(*self)
    }

    #[inline]
    pub fn to_uniform(&self, near_plane: f32, far_plane: f32) -> CameraUniform {
        CameraUniform::from_cpu(*self, near_plane, far_plane)
    }
}

/// Fully resolved camera frame produced by camera directors/runtime.
///
/// Render backends should not consume this implementation type directly. The engine.camera
/// gateway publishes a protocol snapshot and the host bridge lowers that snapshot into a
/// renderer-neutral view frame.
#[derive(Clone, Copy, Debug)]
pub struct CameraFrame {
    pub channel: CameraChannelState,
    pub rig: CameraRig,
    pub projection: Projection,
    pub viewport: CameraViewport,
    pub jitter_px: Vec2,
    pub matrices: CameraMatrices,
    pub frustum: Frustum,
    pub diagnostics: CameraFrameDiagnostics,
}

impl CameraFrame {
    #[inline]
    pub fn build(
        channel: CameraChannelState,
        rig: CameraRig,
        mut projection: Projection,
        viewport: CameraViewport,
        jitter_px: Vec2,
    ) -> Self {
        let viewport = viewport.sanitized();
        projection.set_viewport(viewport.width, viewport.height);

        let view = rig.view_matrix();
        let proj = apply_jitter(projection.matrix(), jitter_px, viewport);
        let matrices = CameraMatrices::from_view_proj(view, proj, rig.position, viewport, jitter_px);
        let frustum = Frustum::from_view_proj(matrices.view_proj);
        let (near_plane, far_plane) = projection.near_far();

        Self {
            channel,
            rig,
            projection,
            viewport,
            jitter_px,
            matrices,
            frustum,
            diagnostics: CameraFrameDiagnostics {
                near_plane,
                far_plane,
                finite: camera_frame_is_finite(&rig, jitter_px, near_plane, far_plane),
            },
        }
    }

    #[inline]
    pub fn view_proj(&self) -> Mat4 {
        self.matrices.view_proj
    }

    #[inline]
    pub fn uniform(&self) -> CameraUniform {
        let (near_plane, far_plane) = self.projection.near_far();
        self.matrices.to_uniform(near_plane, far_plane)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CameraFrameDiagnostics {
    pub near_plane: f32,
    pub far_plane: f32,
    pub finite: bool,
}

impl Default for CameraFrameDiagnostics {
    #[inline]
    fn default() -> Self {
        Self { near_plane: 0.01, far_plane: 1000.0, finite: true }
    }
}

/// GPU-friendly full camera constants.
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

/// Compact GPU uniform for world passes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub world_pos: [f32; 3],
    pub near_plane: f32,
    pub viewport: [f32; 4], // (w, h, 1/w, 1/h)
    pub jitter: [f32; 2],
    pub far_plane: f32,
    pub _pad0: f32,
}

impl CameraUniform {
    #[inline]
    pub fn from_cpu(c: CameraMatrices, near_plane: f32, far_plane: f32) -> Self {
        Self {
            view_proj: mat4_to_cols(c.view_proj),
            world_pos: [c.world_pos.x, c.world_pos.y, c.world_pos.z],
            near_plane,
            viewport: [c.viewport.x, c.viewport.y, c.viewport.z, c.viewport.w],
            jitter: [c.jitter.x, c.jitter.y],
            far_plane,
            _pad0: 0.0,
        }
    }
}

#[inline]
pub fn apply_jitter(proj: Mat4, jitter_px: Vec2, viewport: CameraViewport) -> Mat4 {
    let viewport = viewport.sanitized();
    let w = viewport.width as f32;
    let h = viewport.height as f32;

    let dx = (2.0 * jitter_px.x) / w;
    let dy = (2.0 * jitter_px.y) / h;

    Mat4::from_translation(Vec3::new(dx, dy, 0.0)) * proj
}

#[inline]
fn camera_frame_is_finite(rig: &CameraRig, jitter_px: Vec2, near_plane: f32, far_plane: f32) -> bool {
    rig.position.is_finite()
        && rig.rotation.is_finite()
        && jitter_px.is_finite()
        && near_plane.is_finite()
        && far_plane.is_finite()
        && near_plane > 0.0
        && far_plane > near_plane
}

#[inline]
fn mat4_to_cols(m: Mat4) -> [[f32; 4]; 4] {
    [
        [m.x_axis.x, m.x_axis.y, m.x_axis.z, m.x_axis.w],
        [m.y_axis.x, m.y_axis.y, m.y_axis.z, m.y_axis.w],
        [m.z_axis.x, m.z_axis.y, m.z_axis.z, m.z_axis.w],
        [m.w_axis.x, m.w_axis.y, m.w_axis.z, m.w_axis.w],
    ]
}
