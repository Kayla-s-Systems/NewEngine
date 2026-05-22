#![forbid(unsafe_op_in_unsafe_fn)]

use crate::asset::AssetAccess;

/// Engine-neutral UI image loader stub.
///
/// UI images are requested as semantic texture refs, normally `.ytd@entry`.
/// When the supplied [`AssetAccess`] implementation is the runtime host client,
/// requests go through `engine.textures.entry_rgba8_v1`; UI providers receive an
/// RGBA8/debug texture packet or provider-neutral texture deltas, never raw
/// `.ytd` bytes. Concrete UI providers still own native texture handles and
/// widget library state.
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
        assets: &dyn AssetAccess,
        _key: impl Into<String>,
        path: impl Into<String>,
    ) {
        let path = path.into();
        let _ = assets.textures_entry_rgba8_v1(&path);
    }

    #[inline]
    pub fn tex_id_u64(&self, _key: &str) -> Option<u64> {
        None
    }
}
