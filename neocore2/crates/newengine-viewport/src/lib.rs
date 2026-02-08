#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{Extent2D, RenderTargetId, TextureId};
use newengine_ecs::EntityId;

/// Editor-owned viewport state.
#[derive(Clone, Debug)]
pub struct ViewportState {
    pub camera: EntityId,
    pub extent: Extent2D,

    pub render_target: Option<RenderTargetId>,
    pub color_texture: Option<TextureId>,
}

impl ViewportState {
    #[inline]
    pub fn new(camera: EntityId) -> Self {
        Self {
            camera,
            extent: Extent2D::new(1, 1),
            render_target: None,
            color_texture: None,
        }
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
