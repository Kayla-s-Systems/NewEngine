#![forbid(unsafe_op_in_unsafe_fn)]

use glam::{Mat4, Vec2};

use crate::{CameraInput, CameraMatrices, CameraRig, FreeFlyController, Frustum, Projection};

/// Full camera state used by the engine/editor.
///
/// Owns spatial rig + projection + controller.
/// Produces matrices, frustum and GPU-uniform each frame.
#[derive(Clone, Debug)]
pub struct CameraState {
    pub rig: CameraRig,
    pub projection: Projection,
    pub controller: FreeFlyController,

    pub jitter: Vec2,
    pub viewport_wh: Vec2,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            rig: CameraRig::default(),
            projection: Projection::Perspective(crate::Perspective::new(
                60.0_f32.to_radians(),
                16.0 / 9.0,
                0.1,
                200.0,
            )),
            controller: FreeFlyController::default(),
            jitter: Vec2::ZERO,
            viewport_wh: Vec2::new(1920.0, 1080.0),
        }
    }
}

impl CameraState {
    #[inline]
    pub fn set_viewport(&mut self, width: u32, height: u32) {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        self.viewport_wh = Vec2::new(w, h);
        self.projection.set_viewport(width, height);
    }

    /// Applies controller input (optional) and returns CPU matrices + frustum.
    #[inline]
    pub fn update(&mut self, input: Option<CameraInput>, dt: f32) -> (CameraMatrices, Frustum) {
        if let Some(i) = input {
            if i.zoom_delta.is_finite() && i.zoom_delta.abs() > 1e-6 {
                self.apply_zoom(i.zoom_delta);
            }
            self.controller.apply(&mut self.rig, i, dt);
        }

        let view = self.rig.view_matrix();
        let proj = self.projection.matrix();

        // IMPORTANT: jitter is applied to projection (TAA-ready). For now we offset NDC.
        let proj = apply_jitter(proj, self.jitter, self.viewport_wh);

        let mats = CameraMatrices::new(view, proj, self.rig.position, self.viewport_wh, self.jitter);
        let frustum = Frustum::from_view_proj(mats.view_proj);
        (mats, frustum)
    }

    #[inline]
    pub fn near_far(&self) -> (f32, f32) {
        self.projection.near_far()
    }

    /// Applies zoom to the current projection.
    ///
    /// Convention: positive `delta` zooms in.
    #[inline]
    pub fn apply_zoom(&mut self, delta: f32) {
        let step = (delta * 0.1).clamp(-2.0, 2.0);
        let factor = (1.0 - step).clamp(0.05, 20.0);

        match &mut self.projection {
            Projection::Perspective(p) => {
                let min_fovy = 20.0_f32.to_radians();
                let max_fovy = 100.0_f32.to_radians();
                let fovy = (p.fovy * factor).clamp(min_fovy, max_fovy);
                p.fovy = fovy;
            }
            Projection::Orthographic(o) => {
                let hh = (o.half_height * factor).clamp(0.05, 10_000.0);
                o.half_height = hh;
            }
        }
    }
}

#[inline]
fn apply_jitter(proj: Mat4, jitter: Vec2, viewport_wh: Vec2) -> Mat4 {
    let w = viewport_wh.x.max(1.0);
    let h = viewport_wh.y.max(1.0);

    let dx = (2.0 * jitter.x) / w;
    let dy = (2.0 * jitter.y) / h;

    proj * Mat4::from_translation(glam::Vec3::new(dx, dy, 0.0))
}