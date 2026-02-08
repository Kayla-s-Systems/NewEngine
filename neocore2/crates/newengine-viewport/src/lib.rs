#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{Extent2D, RenderTargetId, TextureId};
use newengine_ecs::EntityId;

/// Editor-owned viewport state.
#[derive(Clone, Debug)]
pub struct ViewportState {
    /// Camera entity used to render this viewport.
    ///
    /// `None` is allowed: the renderer should treat it as "no view" and skip rendering.
    pub camera: Option<EntityId>,
    pub extent: Extent2D,

    pub render_target: Option<RenderTargetId>,
    pub color_texture: Option<TextureId>,
}

impl ViewportState {
    #[inline]
    pub fn new(camera: Option<EntityId>) -> Self {
        Self {
            camera,
            extent: Extent2D::new(1, 1),
            render_target: None,
            color_texture: None,
        }
    }

    #[inline]
    pub fn set_camera(&mut self, camera: Option<EntityId>) {
        self.camera = camera;
    }

    #[inline]
    pub fn set_pixel_rect(&mut self, width: u32, height: u32) {
        self.extent = Extent2D::new(width.max(1), height.max(1));
    }

    #[inline]
    pub fn set_pixel_extent(&mut self, width: u32, height: u32) {
        self.set_pixel_rect(width, height);
    }
}