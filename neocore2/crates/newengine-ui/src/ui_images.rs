#![forbid(unsafe_op_in_unsafe_fn)]

use crate::asset::AssetAccess;

/// Engine-neutral UI image loader stub.
///
/// Concrete UI providers own native texture handles and widget library state.
/// Engine crates intentionally do not import concrete UI toolkits; callers that
/// need textured UI should publish provider-neutral texture ids through
/// `engine.ui` or through `newengine-ui-draw` texture deltas.
#[derive(Debug, Default)]
pub struct UiImageLoader;

impl UiImageLoader {
    #[inline]
    pub fn new() -> Self {
        Self
    }

    #[inline]
    pub fn request(
        &mut self,
        _assets: &dyn AssetAccess,
        _key: impl Into<String>,
        _path: impl Into<String>,
    ) {}

    #[inline]
    pub fn tex_id_u64(&self, _key: &str) -> Option<u64> {
        None
    }
}
