#![forbid(unsafe_op_in_unsafe_fn)]

use glam::Vec3;
use newengine_camera::{orbit_set_angles, CameraRig, OrbitController, Perspective, Projection};
use newengine_core::render::Extent2D;

use crate::plugin_manager::PluginManagerBridge;
use crate::scene_bridge::SceneBridge;
use crate::viewport_bridge::ViewportBridge;

use super::gpu::{GridGpu, LitPipeline, PrimitiveGpu};

pub struct EditorRenderController {
    pub(super) clear_color: [f32; 4],
    pub(super) last_w: u32,
    pub(super) last_h: u32,

    pub(super) last_vp_w: u32,
    pub(super) last_vp_h: u32,
    pub(super) last_aspect: f32,

    pub(super) orbit: OrbitController,
    pub(super) rig: CameraRig,
    pub(super) projection: Projection,

    pub(super) viewport_bridge: std::sync::Arc<ViewportBridge>,
    pub(super) plugins_bridge: std::sync::Arc<PluginManagerBridge>,
    pub(super) scene_bridge: std::sync::Arc<SceneBridge>,

    pub(super) viewport_rt: Option<newengine_core::render::RenderTargetId>,
    pub(super) viewport_rt_extent: Extent2D,

    pub(super) grid: Option<GridGpu>,
    pub(super) lit: Option<LitPipeline>,
    pub(super) prim_cube: Option<PrimitiveGpu>,
    pub(super) prim_plane: Option<PrimitiveGpu>,

    pub(super) framed_once: bool,
    pub(super) last_bounds_center: Vec3,
    pub(super) last_bounds_radius: f32,
}

impl EditorRenderController {
    #[inline]
    pub fn new(
        clear_color: [f32; 4],
        viewport_bridge: std::sync::Arc<ViewportBridge>,
        plugins_bridge: std::sync::Arc<PluginManagerBridge>,
        scene_bridge: std::sync::Arc<SceneBridge>,
    ) -> Self {
        // Engine baseline coordinate system:
        // - right-handed
        // - +Y up
        // - -Z forward
        // CameraRig::forward() points along -Z.
        let mut orbit = OrbitController::default();
        orbit_set_angles(&mut orbit, 0.7853982, 0.55);
        orbit.distance = 4.1;

        let rig = CameraRig::default();
        let projection = Projection::Perspective(Perspective::new(
            60.0f32.to_radians(),
            1.0,
            0.01,
            1000.0,
        ));

        Self {
            clear_color,
            last_w: 0,
            last_h: 0,

            last_vp_w: 0,
            last_vp_h: 0,
            last_aspect: 1.0,

            orbit,
            rig,
            projection,

            viewport_bridge,
            plugins_bridge,
            scene_bridge,

            viewport_rt: None,
            viewport_rt_extent: Extent2D::new(0, 0),

            grid: None,
            lit: None,
            prim_cube: None,
            prim_plane: None,

            framed_once: false,
            last_bounds_center: Vec3::ZERO,
            last_bounds_radius: 1.0,
        }
    }

    #[inline]
    pub(super) fn default_bounds(&self) -> (Vec3, f32) {
        (Vec3::ZERO, 1.0)
    }
}
