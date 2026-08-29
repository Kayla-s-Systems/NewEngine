#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::Extent2D;
use newengine_math::{Mat4, Vec3};
use newengine_render_feature_api::BoundsSnap;
use newengine_scene::Scene;

use super::scene;

/// CPU-side scene render snapshot captured before RenderPrep/submit.
///
/// This is the first structural boundary for moving provider-safe extraction out
/// of `render.controller`. It intentionally contains DTO-like values, not
/// `RenderApi`, backend handles or mutable world references. Heavy consumers can
/// later receive this through `engine.threading` RenderPrep batches and return frame
/// packets for render-thread recording.
#[derive(Clone, Copy, Debug)]
pub(super) struct SceneRenderSnapshot {
    pub frame_index: u64,
    pub bounds: BoundsSnap,
    pub camera_position: Vec3,
    pub camera_forward: Vec3,
    pub viewport_extent: Extent2D,
    pub surface_extent: Extent2D,
    pub ui_present: bool,
    pub plugin_snapshot_present: bool,
}

impl SceneRenderSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn capture(
        frame_index: u64,
        scene: &Scene,
        _viewproj: Mat4,
        camera_position: Vec3,
        camera_forward: Vec3,
        viewport_extent: Extent2D,
        surface_extent: Extent2D,
        ui_present: bool,
        plugin_snapshot_present: bool,
    ) -> Self {
        Self {
            frame_index,
            bounds: scene::scene_bounds(scene).unwrap_or_else(scene::default_bounds),
            camera_position,
            camera_forward,
            viewport_extent,
            surface_extent,
            ui_present,
            plugin_snapshot_present,
        }
    }

    pub(super) fn diagnostic_detail(&self) -> String {
        format!(
            "SceneRenderSnapshot frame={} bounds_radius={:.3} viewport={}x{} surface={}x{} ui_present={} plugin_snapshot={}",
            self.frame_index,
            self.bounds.radius,
            self.viewport_extent.width,
            self.viewport_extent.height,
            self.surface_extent.width,
            self.surface_extent.height,
            self.ui_present,
            self.plugin_snapshot_present,
        )
    }
}
