#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::Extent2D;

/// Runtime viewport parameters.
///
/// Contains size and dirty flags.
/// No GPU objects here.
#[derive(Clone, Debug)]
pub struct ViewportRuntime {
    extent: Extent2D,
    resized: bool,
}

impl Default for ViewportRuntime {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl ViewportRuntime {
    #[inline]
    pub fn new() -> Self {
        Self {
            extent: Extent2D::new(1, 1),
            resized: true,
        }
    }

    #[inline]
    pub fn extent(&self) -> Extent2D {
        self.extent
    }

    #[inline]
    pub fn is_resized(&self) -> bool {
        self.resized
    }

    #[inline]
    pub fn clear_resize_flag(&mut self) {
        self.resized = false;
    }

    #[inline]
    pub fn set_extent(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);

        let new_extent = Extent2D::new(width, height);

        if self.extent != new_extent {
            self.extent = new_extent;
            self.resized = true;
        }
    }
}
