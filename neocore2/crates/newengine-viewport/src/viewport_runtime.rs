#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::Extent2D;

/// Runtime viewport parameters.
///
/// Contains size and dirty flags. No GPU objects live here.
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

    /// Returns the resize state and clears it in one operation.
    #[inline]
    pub fn take_resize_flag(&mut self) -> bool {
        std::mem::take(&mut self.resized)
    }

    #[inline]
    pub fn set_extent(&mut self, width: u32, height: u32) {
        let new_extent = Extent2D::new(width.max(1), height.max(1));

        if self.extent != new_extent {
            self.extent = new_extent;
            self.resized = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_flag_is_consumed_atomically() {
        let mut runtime = ViewportRuntime::new();
        assert!(runtime.take_resize_flag());
        assert!(!runtime.take_resize_flag());

        runtime.set_extent(1920, 1080);
        assert!(runtime.take_resize_flag());
    }

    #[test]
    fn extent_is_clamped_and_unchanged_size_stays_clean() {
        let mut runtime = ViewportRuntime::new();
        runtime.clear_resize_flag();
        runtime.set_extent(0, 0);
        assert_eq!(runtime.extent(), Extent2D::new(1, 1));
        assert!(!runtime.is_resized());
    }
}
