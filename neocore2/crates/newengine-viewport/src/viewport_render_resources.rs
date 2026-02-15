#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{RenderTargetId, TextureId};

/// GPU resources backing a viewport.
///
/// Fully owned by render controller.
/// Editor must not mutate this directly.
#[derive(Clone, Debug, Default)]
pub struct ViewportRenderResources {
    pub render_target: Option<RenderTargetId>,
    pub color_texture: Option<TextureId>,
}

impl ViewportRenderResources {
    #[inline]
    pub fn clear(&mut self) {
        self.render_target = None;
        self.color_texture = None;
    }

    #[inline]
    pub fn is_initialized(&self) -> bool {
        self.render_target.is_some() && self.color_texture.is_some()
    }
}